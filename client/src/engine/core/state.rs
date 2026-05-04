use std::sync::Arc;
use std::u32::MAX;

use crate::common::geometry::vertex::Vertex;
use crate::engine::audio::GameAudioManager;
use crate::engine::core::frame::{EngineFrameData, GameFrameData};
use crate::engine::render::camera::RenderCamera;
use crate::engine::render::manager::RenderManager;
use crate::engine::render::render::{GpuContext, RenderOptions, Renderer};
use crate::engine::render::text::text_renderer::FPS_UPDATE_DELAY;
use crate::engine::render::text::TextRenderer;
use crate::engine::render::texture::TextureManager;
use shared::world::data::chunk::CHUNK_SIZE_F;
use std::time::Instant;
use wgpu::util::DeviceExt;
use wgpu::wgt::BufferDescriptor;
use winit::window::Window;

pub struct State {
    pub window: Arc<Window>,
    pub engine_frame_data: EngineFrameData,
    pub game_frame_data: GameFrameData,
    pub renderer: Renderer,
    text_renderer: TextRenderer,
    pub audio_manager: Option<GameAudioManager>,
}

impl State {
    pub async fn new<S: crate::engine::core::application::AppState>(window: Arc<Window>, _app_state: &S) -> anyhow::Result<Self> {
        let audio_manager = GameAudioManager::new(&window);
        let size = window.inner_size();

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });

        let surface = instance.create_surface(window.clone()).unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::POLYGON_MODE_LINE
                    | wgpu::Features::CONSERVATIVE_RASTERIZATION
                    | wgpu::Features::MULTI_DRAW_INDIRECT_COUNT,
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                required_limits: wgpu::Limits::default(),
                memory_hints: Default::default(),
                trace: wgpu::Trace::Off,
            })
            .await?;

        let limits = device.limits();

        let surface_caps = surface.get_capabilities(&adapter);

        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::AutoNoVsync,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        let texture_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
            label: Some("texture_bind_group_layout"),
        });

        macro_rules! load_textures {
            ($($name:ident: $path:literal),*) => {
                vec![$(
                    image::load_from_memory(include_bytes!($path))
                        .expect(concat!("Failed to load texture: ", $path))
                        .to_rgba8()
                ),*]
            };
        }

        let textures_data = load_textures!(
            grass: "../../../../assets/images/grass.png",
            dirt: "../../../../assets/images/dirt.png",
            stone: "../../../../assets/images/stone.png"
        );

        let (width, height) = textures_data[0].dimensions();
        for (i, data) in textures_data.iter().enumerate() {
            assert_eq!(
                data.dimensions(),
                (width, height),
                "All textures must have same dimensions (texture {})",
                i
            );
        }

        let textures: Vec<&[u8]> = textures_data.iter().map(|d| d.as_ref()).collect();

        let mut texture_manager = TextureManager::new(
            limits.max_texture_dimension_2d,
            limits.max_texture_array_layers
        );

        for (_, texture) in textures.iter().enumerate() {
            texture_manager.register(&device, &queue, texture, 32, 32);
        }

        let diffuse_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture_manager.arrays.first().unwrap().view()),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&texture_manager.arrays.first().unwrap().sampler()),
                },
            ],
            label: Some("diffuse_bind_group"),
        });

        let camera_uniform = RenderCamera::new();

        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[camera_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let camera_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
            label: Some("camera_bind_group_layout"),
        });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
            label: Some("camera_bind_group"),
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../../../../assets/shaders/shader.wgsl").into()),
        });

        let render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts: &[&texture_bind_group_layout, &camera_bind_group_layout],
            immediate_size: 0,
        });

        let wireframe_render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Wireframe Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Vertex::buffer_layout()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Line,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState {
                    constant: 0,
                    slope_scale: 0.0,
                    clamp: 0.0,
                },
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Normal Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Vertex::buffer_layout()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: true, // warning: conservative may NOT be supported. TODO: handle unsupported conservative rasterization
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState {
                    constant: 0,
                    slope_scale: 0.0,
                    clamp: 0.0,
                },
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        });

        let gizmo_render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Gizmo Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Vertex::buffer_layout()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::LineList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        });

        let gizmo = [
            Vertex::new_with_rgba(0.0, 0.0, 0.0, 255, 0, 0, 255, MAX, 3.0, 0.0, 0.0),
            Vertex::new_with_rgba(1.0, 0.0, 0.0, 255, 0, 0, 255, MAX, 3.0, 0.0, 0.0),
            Vertex::new_with_rgba(0.0, 0.0, 0.0, 0, 255, 0, 255, MAX, 3.0, 0.0, 0.0),
            Vertex::new_with_rgba(0.0, 1.0, 0.0, 0, 255, 0, 255, MAX, 3.0, 0.0, 0.0),
            Vertex::new_with_rgba(0.0, 0.0, 0.0, 0, 0, 255, 255, MAX, 3.0, 0.0, 0.0),
            Vertex::new_with_rgba(0.0, 0.0, 1.0, 0, 0, 255, 255, MAX, 3.0, 0.0, 0.0),
        ];

        let gizmo_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Gizmo Buffer"),
            contents: bytemuck::cast_slice(&gizmo),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let chunk_borders = vec![
            Vertex::new_with_rgba(0.0, 0.0, 0.0, 0, 255, 0, 255, MAX, 3.0, 0.0, 0.0),
            Vertex::new_with_rgba(0.0, CHUNK_SIZE_F, 0.0, 0, 255, 0, 255, MAX, 3.0, 0.0, 0.0),
            Vertex::new_with_rgba(0.0, 0.0, CHUNK_SIZE_F, 0, 255, 0, 255, MAX, 3.0, 0.0, 0.0),
            Vertex::new_with_rgba(0.0, CHUNK_SIZE_F, CHUNK_SIZE_F, 0, 255, 0, 255, MAX, 3.0, 0.0, 0.0),
            Vertex::new_with_rgba(CHUNK_SIZE_F, 0.0, 0.0, 0, 255, 0, 255, MAX, 3.0, 0.0, 0.0),
            Vertex::new_with_rgba(CHUNK_SIZE_F, CHUNK_SIZE_F, 0.0, 0, 255, 0, 255, MAX, 3.0, 0.0, 0.0),
            Vertex::new_with_rgba(CHUNK_SIZE_F, 0.0, CHUNK_SIZE_F, 0, 255, 0, 255, MAX, 3.0, 0.0, 0.0),
            Vertex::new_with_rgba(CHUNK_SIZE_F, CHUNK_SIZE_F, CHUNK_SIZE_F, 0, 255, 0, 255, MAX, 3.0, 0.0, 0.0),
            Vertex::new_with_rgba(0.0, 0.0, 0.0, 255, 0, 0, 255, MAX, 3.0, 0.0, 0.0),
            Vertex::new_with_rgba(CHUNK_SIZE_F, 0.0, 0.0, 255, 0, 0, 255, MAX, 3.0, 0.0, 0.0),
            Vertex::new_with_rgba(0.0, 0.0, CHUNK_SIZE_F, 255, 0, 0, 255, MAX, 3.0, 0.0, 0.0),
            Vertex::new_with_rgba(CHUNK_SIZE_F, 0.0, CHUNK_SIZE_F, 255, 0, 0, 255, MAX, 3.0, 0.0, 0.0),
            Vertex::new_with_rgba(0.0, CHUNK_SIZE_F, 0.0, 255, 0, 0, 255, MAX, 3.0, 0.0, 0.0),
            Vertex::new_with_rgba(CHUNK_SIZE_F, CHUNK_SIZE_F, 0.0, 255, 0, 0, 255, MAX, 3.0, 0.0, 0.0),
            Vertex::new_with_rgba(0.0, CHUNK_SIZE_F, CHUNK_SIZE_F, 255, 0, 0, 255, MAX, 3.0, 0.0, 0.0),
            Vertex::new_with_rgba(CHUNK_SIZE_F, CHUNK_SIZE_F, CHUNK_SIZE_F, 255, 0, 0, 255, MAX, 3.0, 0.0, 0.0),
            Vertex::new_with_rgba(0.0, 0.0, 0.0, 0, 0, 255, 255, MAX, 3.0, 0.0, 0.0),
            Vertex::new_with_rgba(0.0, 0.0, CHUNK_SIZE_F, 0, 0, 255, 255, MAX, 3.0, 0.0, 0.0),
            Vertex::new_with_rgba(CHUNK_SIZE_F, 0.0, 0.0, 0, 0, 255, 255, MAX, 3.0, 0.0, 0.0),
            Vertex::new_with_rgba(CHUNK_SIZE_F, 0.0, CHUNK_SIZE_F, 0, 0, 255, 255, MAX, 3.0, 0.0, 0.0),
            Vertex::new_with_rgba(0.0, CHUNK_SIZE_F, 0.0, 0, 0, 255, 255, MAX, 3.0, 0.0, 0.0),
            Vertex::new_with_rgba(0.0, CHUNK_SIZE_F, CHUNK_SIZE_F, 0, 0, 255, 255, MAX, 3.0, 0.0, 0.0),
            Vertex::new_with_rgba(CHUNK_SIZE_F, CHUNK_SIZE_F, 0.0, 0, 0, 255, 255, MAX, 3.0, 0.0, 0.0),
            Vertex::new_with_rgba(CHUNK_SIZE_F, CHUNK_SIZE_F, CHUNK_SIZE_F, 0, 0, 255, 255, MAX, 3.0, 0.0, 0.0),
        ];

        let chunk_borders_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Chunk Borders Buffer"),
            contents: bytemuck::cast_slice(&chunk_borders),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });

        window
            .set_cursor_grab(winit::window::CursorGrabMode::Confined)
            .expect("Capture souris");
        window.set_cursor_visible(false);

        let engine_frame_data = EngineFrameData::new();
        let mut game_frame_data = GameFrameData::blank();

        game_frame_data.camera = camera_uniform;

        let depth_size = wgpu::Extent3d {
            width: size.width,
            height: size.height,
            depth_or_array_layers: 1,
        };

        let depth_texture_desc = wgpu::TextureDescriptor {
            label: Some("Depth Texture"),
            size: depth_size,
            view_formats: &[wgpu::TextureFormat::Depth32Float],
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        };

        let depth_texture = device.create_texture(&depth_texture_desc);

        let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let gpu_context = GpuContext {
            surface,
            device,
            queue,
            config,
        };

        let render_manager = RenderManager::new(&gpu_context.device);

        let player_mesh = gpu_context.device.create_buffer(&BufferDescriptor {
            label: Some("Player Mesh Buffer"),
            mapped_at_creation: false,
            size: size_of::<Vertex>() as u64 * 36, // 36 vertices for a cube
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });

        let renderer = Renderer::new(
            false,
            wireframe_render_pipeline,
            render_pipeline,
            diffuse_bind_group,
            texture_manager,
            camera_buffer,
            camera_bind_group,
            gizmo_render_pipeline,
            gizmo_buffer,
            (size.width, size.height),
            chunk_borders,
            chunk_borders_buffer,
            gpu_context,
            render_manager,
            depth_texture,
            depth_view,
            player_mesh,
        );

        let text_renderer = TextRenderer::new(
            &renderer.gpu_context.device,
            &renderer.gpu_context.queue,
            renderer.gpu_context.config.format,
        );

        Ok(Self {
            window,
            engine_frame_data,
            game_frame_data,
            renderer,
            text_renderer,
            audio_manager: Some(audio_manager.expect("Failed to load audio manager in state")),
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.renderer.render_options = RenderOptions {
                aspect: (width as f32) / (height as f32),
                znear: self.renderer.render_options.znear,
                zfar: self.renderer.render_options.zfar,
            };
            self.renderer.gpu_context.config.width = width;
            self.renderer.gpu_context.config.height = height;
            self.renderer
                .gpu_context
                .surface
                .configure(&self.renderer.gpu_context.device, &self.renderer.gpu_context.config);
            self.renderer.is_surface_configured = true;
            self.text_renderer.resize(width, height);
            self.renderer.depth_texture = self.renderer.gpu_context.device.create_texture(&wgpu::TextureDescriptor {
                size: wgpu::Extent3d {
                    width: width,
                    height: height,
                    depth_or_array_layers: 1,
                },
                ..wgpu::TextureDescriptor {
                    label: Some("Depth Texture"),
                    view_formats: &[wgpu::TextureFormat::Depth32Float],
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Depth32Float,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                    size: Default::default(),
                }
            });
            self.renderer.depth_view = self.renderer.depth_texture.create_view(&wgpu::TextureViewDescriptor::default());
        }
    }

    pub fn frame_update(&mut self) {
        let now = Instant::now();
        let dt = now - self.engine_frame_data.last_frame;

        self.engine_frame_data.last_frame = now;
        self.engine_frame_data.dt = dt.as_secs_f32();

        self.engine_frame_data.frame_count += 1;
        self.engine_frame_data.fps_timer += dt.as_secs_f32();

        if self.engine_frame_data.fps_timer >= 1.0 {
            self.engine_frame_data.fps = self.engine_frame_data.frame_count;
            self.engine_frame_data.frame_count = 0;
            self.engine_frame_data.fps_timer = self.engine_frame_data.fps_timer - 1.0;
        }
    }

    pub fn update(&mut self) {
        self.frame_update();
        self.text_renderer.timer += self.engine_frame_data.dt;
        if self.text_renderer.timer >= FPS_UPDATE_DELAY {
            self.text_renderer.update_text(
                self.engine_frame_data.fps,
                (1.0 / self.engine_frame_data.dt) as u32,
                self.engine_frame_data.dt,
            );
            self.text_renderer.timer -= FPS_UPDATE_DELAY;
        }
        if let Some(ref mut audio) = self.audio_manager {
            audio.update();
        }
    }

    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        self.renderer.render_manager.mesh_manager.flush(&self.renderer.gpu_context.queue);
        self.renderer.render(&self.game_frame_data.camera, Some(&mut self.text_renderer));

        Ok(())
    }
}
