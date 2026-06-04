use crate::{
    audio::GameAudioManager,
    core::{
        application::AppState,
        frame::{EngineFrameData, GameFrameData},
    },
    gpu::{
        context::GpuContext,
        factories::wrapper::GpuFactory,
        layouts::BufferLayouts,
        resources::{pipeline::Pipelines, wrapper::GpuResources},
        textures::manager::TextureManager,
    },
    render::{
        camera::RenderCamera,
        debug::DebugRenderResources,
        manager::RenderManager,
        modes::RenderMode,
        options::RenderOptions,
        render::Renderer,
        text::{text_renderer::FPS_UPDATE_DELAY, TextRenderer},
    },
};
use bytemuck::cast_slice;
use game::world::data::chunk::CHUNK_SIZE_F;

use crate::geometry::vertex::Vertex;
use std::sync::Arc;
use std::time::Instant;
use wgpu::{
    util::{BufferInitDescriptor, DeviceExt},
    BindGroup, BlendState, Buffer, BufferUsages, ColorTargetState, ColorWrites, CompareFunction, CurrentSurfaceTexture, DepthBiasState,
    DepthStencilState, Face, Features, FragmentState, FrontFace, PolygonMode, PrimitiveState, PrimitiveTopology, RenderPipeline,
    ShaderSource, Texture, TextureFormat, TextureView, VertexState,
};
use winit::{dpi::PhysicalSize, event_loop::ActiveEventLoop, window::Window};

pub struct State {
    pub window: Arc<Window>,
    pub engine_frame_data: EngineFrameData,
    pub game_frame_data: GameFrameData,
    pub renderer: Renderer,
    text_renderer: TextRenderer,
    pub audio_manager: Option<GameAudioManager>,
}

impl State {
    pub async fn new<S: AppState>(window: Arc<Window>, event_loop: &ActiveEventLoop, _app_state: &S) -> anyhow::Result<Self> {
        let audio_manager = GameAudioManager::new(&window).ok();

        let size = window.inner_size();

        let (
            gpu_context,
            texture_manager,
            texture_bind_group,
            render_camera,
            camera_buffer,
            camera_bind_group,
            gizmo_render_pipeline,
            gizmo_buffer,
            chunk_borders_buffer,
            depth_texture,
            depth_view,
            pipelines,
            debug_pipelines,
        ) = fun_name(size, &window, event_loop);

        let device = gpu_context.tools.device();
        let queue = gpu_context.tools.queue();
        let config = &gpu_context.config;

        // window.set_cursor_grab(winit::window::CursorGrabMode::Confined).unwrap_or(());
        window.set_cursor_visible(false);

        let engine_frame_data = EngineFrameData::new();
        let mut game_frame_data = GameFrameData::blank();

        game_frame_data.camera = render_camera;

        let render_manager = RenderManager::new(gpu_context.get_tools(), gpu_context.get_encoder());
        let text_renderer = TextRenderer::new(device, queue, config.format);
        let render_options = RenderOptions::new((size.width as f32) / (size.height as f32), 0.1, 1000.0);

        let gpu_tools = GpuResources::new(
            pipelines,
            camera_bind_group,
            texture_bind_group,
            camera_buffer,
            depth_texture,
            depth_view,
        );

        let debug = DebugRenderResources::new(debug_pipelines, gizmo_render_pipeline, gizmo_buffer, chunk_borders_buffer);

        let renderer = Renderer::new(
            false,
            render_manager,
            render_options,
            texture_manager,
            gpu_context,
            gpu_tools,
            debug,
        );

        Ok(Self {
            window,
            engine_frame_data,
            game_frame_data,
            renderer,
            text_renderer,
            audio_manager,
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
                .configure(&self.renderer.gpu_context.tools.device(), &self.renderer.gpu_context.config);
            self.renderer.is_surface_configured = true;
            self.text_renderer.resize(width, height);
            self.renderer.gpu_resources.depth_texture = self.renderer.gpu_context.tools.device().create_texture(&wgpu::TextureDescriptor {
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
            self.renderer.gpu_resources.depth_view = self
                .renderer
                .gpu_resources
                .depth_texture
                .create_view(&wgpu::TextureViewDescriptor::default());
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
        if let Some(audio) = self.audio_manager.as_mut() {
            audio.update();
        }
    }

    pub fn render(&mut self) {
        self.renderer.render_manager.mesh_manager.write().unwrap().flush();

        let output = self.renderer.gpu_context.surface.get_current_texture();
        match output {
            CurrentSurfaceTexture::Success(surface_texture) => {
                self.renderer
                    .render(surface_texture, &self.game_frame_data.camera, Some(&mut self.text_renderer));
            }
            CurrentSurfaceTexture::Suboptimal(_) | CurrentSurfaceTexture::Outdated => {
                let (width, height) = self.window.inner_size().into();
                self.resize(width, height);
                // reconfigure
                return;
            }
            CurrentSurfaceTexture::Timeout | CurrentSurfaceTexture::Occluded | CurrentSurfaceTexture::Validation => {
                return;
            }
            CurrentSurfaceTexture::Lost => {
                return; /* chiant sa mère */
            }
        }
    }

    pub fn dispose(&mut self) {
        // if let Some(audio) = self.audio_manager.as_mut() {
        //     // TODO: faire fonctionner -> audio.dispose();
        // }
        self.text_renderer.dispose();
        self.renderer.dispose();
    }
}

fn fun_name(
    size: PhysicalSize<u32>,
    window: &Arc<Window>,
    event_loop: &ActiveEventLoop,
) -> (
    GpuContext,
    TextureManager,
    BindGroup,
    RenderCamera,
    Buffer,
    BindGroup,
    RenderPipeline,
    Buffer,
    Buffer,
    Texture,
    TextureView,
    Pipelines,
    Pipelines,
) {
    const GIZMO: [Vertex; 6] = [
        Vertex::new_with_rgba(0.0, 0.0, 0.0, 255, 0, 0, 255, 0, 3.0, 0.0, 0.0),
        Vertex::new_with_rgba(1.0, 0.0, 0.0, 255, 0, 0, 255, 0, 3.0, 0.0, 0.0),
        Vertex::new_with_rgba(0.0, 0.0, 0.0, 0, 255, 0, 255, 0, 3.0, 0.0, 0.0),
        Vertex::new_with_rgba(0.0, 1.0, 0.0, 0, 255, 0, 255, 0, 3.0, 0.0, 0.0),
        Vertex::new_with_rgba(0.0, 0.0, 0.0, 0, 0, 255, 255, 0, 3.0, 0.0, 0.0),
        Vertex::new_with_rgba(0.0, 0.0, 1.0, 0, 0, 255, 255, 0, 3.0, 0.0, 0.0),
    ];

    const CHUNK_BORDERS: [Vertex; 24] = [
        Vertex::new_with_rgba(0.0, 0.0, 0.0, 0, 255, 0, 255, 0, 3.0, 0.0, 1.0),
        Vertex::new_with_rgba(0.0, CHUNK_SIZE_F, 0.0, 0, 255, 0, 255, 0, 3.0, 0.0, 1.0),
        Vertex::new_with_rgba(0.0, 0.0, CHUNK_SIZE_F, 0, 255, 0, 255, 0, 3.0, 0.0, 1.0),
        Vertex::new_with_rgba(0.0, CHUNK_SIZE_F, CHUNK_SIZE_F, 0, 255, 0, 255, 0, 3.0, 0.0, 1.0),
        Vertex::new_with_rgba(CHUNK_SIZE_F, 0.0, 0.0, 0, 255, 0, 255, 0, 3.0, 0.0, 1.0),
        Vertex::new_with_rgba(CHUNK_SIZE_F, CHUNK_SIZE_F, 0.0, 0, 255, 0, 255, 0, 3.0, 0.0, 1.0),
        Vertex::new_with_rgba(CHUNK_SIZE_F, 0.0, CHUNK_SIZE_F, 0, 255, 0, 255, 0, 3.0, 0.0, 1.0),
        Vertex::new_with_rgba(CHUNK_SIZE_F, CHUNK_SIZE_F, CHUNK_SIZE_F, 0, 255, 0, 255, 0, 3.0, 0.0, 1.0),
        Vertex::new_with_rgba(0.0, 0.0, 0.0, 255, 0, 0, 255, 0, 3.0, 0.0, 1.0),
        Vertex::new_with_rgba(CHUNK_SIZE_F, 0.0, 0.0, 255, 0, 0, 255, 0, 3.0, 0.0, 1.0),
        Vertex::new_with_rgba(0.0, 0.0, CHUNK_SIZE_F, 255, 0, 0, 255, 0, 3.0, 0.0, 1.0),
        Vertex::new_with_rgba(CHUNK_SIZE_F, 0.0, CHUNK_SIZE_F, 255, 0, 0, 255, 0, 3.0, 0.0, 1.0),
        Vertex::new_with_rgba(0.0, CHUNK_SIZE_F, 0.0, 255, 0, 0, 255, 0, 3.0, 0.0, 1.0),
        Vertex::new_with_rgba(CHUNK_SIZE_F, CHUNK_SIZE_F, 0.0, 255, 0, 0, 255, 0, 3.0, 0.0, 1.0),
        Vertex::new_with_rgba(0.0, CHUNK_SIZE_F, CHUNK_SIZE_F, 255, 0, 0, 255, 0, 3.0, 0.0, 1.0),
        Vertex::new_with_rgba(CHUNK_SIZE_F, CHUNK_SIZE_F, CHUNK_SIZE_F, 255, 0, 0, 255, 0, 3.0, 0.0, 1.0),
        Vertex::new_with_rgba(0.0, 0.0, 0.0, 0, 0, 255, 255, 0, 3.0, 0.0, 1.0),
        Vertex::new_with_rgba(0.0, 0.0, CHUNK_SIZE_F, 0, 0, 255, 255, 0, 3.0, 0.0, 1.0),
        Vertex::new_with_rgba(CHUNK_SIZE_F, 0.0, 0.0, 0, 0, 255, 255, 0, 3.0, 0.0, 1.0),
        Vertex::new_with_rgba(CHUNK_SIZE_F, 0.0, CHUNK_SIZE_F, 0, 0, 255, 255, 0, 3.0, 0.0, 1.0),
        Vertex::new_with_rgba(0.0, CHUNK_SIZE_F, 0.0, 0, 0, 255, 255, 0, 3.0, 0.0, 1.0),
        Vertex::new_with_rgba(0.0, CHUNK_SIZE_F, CHUNK_SIZE_F, 0, 0, 255, 255, 0, 3.0, 0.0, 1.0),
        Vertex::new_with_rgba(CHUNK_SIZE_F, CHUNK_SIZE_F, 0.0, 0, 0, 255, 255, 0, 3.0, 0.0, 1.0),
        Vertex::new_with_rgba(CHUNK_SIZE_F, CHUNK_SIZE_F, CHUNK_SIZE_F, 0, 0, 255, 255, 0, 3.0, 0.0, 1.0),
    ];

    let gpu_context = GpuContext::new(Arc::clone(window), event_loop.owned_display_handle()).unwrap();

    let device = gpu_context.tools.device();
    let config = &gpu_context.config;

    let texture_manager = TextureManager::new(
        gpu_context.get_tools(),
        gpu_context.limits.max_texture_dimension_2d,
        gpu_context.limits.max_texture_array_layers,
    );

    let gpu_factory = GpuFactory::new(&gpu_context);

    let texture_bind_group_layout = gpu_factory.bind_group_layout().make_textures(Some("Texture Bind Group Layout"));

    let opaque_array = texture_manager.get_array(&RenderMode::Opaque);
    let alpha_cutout_array = texture_manager.get_array(&RenderMode::AlphaCutout);
    let translucent_array = texture_manager.get_array(&RenderMode::Translucent);
    let billboard_array = texture_manager.get_array(&RenderMode::Billboard);
    let ui_atlas = texture_manager.get_ui_atlas();

    let textures_arrays_bind_group_entries = [opaque_array, alpha_cutout_array, translucent_array, billboard_array];

    let textures_bind_group = gpu_factory.bind_group().make_textures_entries(
        &texture_bind_group_layout,
        &textures_arrays_bind_group_entries,
        ui_atlas,
        Some("Texture Bind Group"),
    );

    let render_camera = RenderCamera::new();

    let camera_buffer = gpu_context.tools.device().create_buffer_init(&BufferInitDescriptor {
        label: Some("Camera Buffer"),
        contents: cast_slice(render_camera.view_proj().current()),
        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
    });

    let camera_bind_group_layout = gpu_factory.bind_group_layout().make_camera();
    let camera_bind_group = gpu_factory.bind_group().make_camera(&camera_bind_group_layout, &camera_buffer);

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Shader"),
        source: ShaderSource::Wgsl(include_str!("../../assets/shaders/shader.wgsl").into()),
    });

    let render_pipeline_layout = gpu_factory
        .pipeline_layout()
        .make(None, &[Some(&texture_bind_group_layout), Some(&camera_bind_group_layout)]);

    let vertex = VertexState {
        module: &shader,
        entry_point: Some("vs_main"),
        buffers: &[BufferLayouts::vertex()],
        compilation_options: Default::default(),
    };

    let fragment = FragmentState {
        module: &shader,
        entry_point: Some("fs_main"),
        targets: &[Some(ColorTargetState {
            format: config.format,
            blend: Some(BlendState::REPLACE),
            write_mask: ColorWrites::ALL,
        })],
        compilation_options: wgpu::PipelineCompilationOptions::default(),
    };

    let wireframe_primitive = PrimitiveState {
        topology: PrimitiveTopology::TriangleList,
        strip_index_format: None,
        front_face: FrontFace::Ccw,
        cull_mode: Some(Face::Back),
        polygon_mode: PolygonMode::Line,
        unclipped_depth: false,
        conservative: false,
    };

    let normal_primitive = PrimitiveState {
        topology: PrimitiveTopology::TriangleList,
        strip_index_format: None,
        front_face: FrontFace::Ccw,
        cull_mode: Some(Face::Back),
        polygon_mode: PolygonMode::Fill,
        unclipped_depth: false,
        conservative: gpu_context.features.contains(Features::CONSERVATIVE_RASTERIZATION),
    };

    let gizmo_primitive = PrimitiveState {
        topology: PrimitiveTopology::LineList,
        strip_index_format: None,
        front_face: FrontFace::Ccw,
        cull_mode: Some(Face::Back),
        polygon_mode: PolygonMode::Fill,
        unclipped_depth: false,
        conservative: false,
    };

    let depth_stencil = DepthStencilState {
        format: TextureFormat::Depth32Float,
        depth_write_enabled: Some(true),
        depth_compare: Some(CompareFunction::Less),
        bias: DepthBiasState {
            constant: 0,
            slope_scale: 0.0,
            clamp: 0.0,
        },
        stencil: Default::default(),
    };

    let wireframe_render_pipeline = gpu_factory.pipeline().make(
        "Wireframe",
        &render_pipeline_layout,
        vertex.clone(),
        fragment.clone(),
        wireframe_primitive,
        Some(depth_stencil.clone()),
    );

    let render_pipeline = gpu_factory.pipeline().make(
        "Normal",
        &render_pipeline_layout,
        vertex.clone(),
        fragment.clone(),
        normal_primitive,
        Some(depth_stencil.clone()),
    );

    let gizmo_render_pipeline = gpu_factory.pipeline().make(
        "Gizmo",
        &render_pipeline_layout,
        vertex.clone(),
        fragment.clone(),
        gizmo_primitive,
        Some(depth_stencil.clone()),
    );

    let gizmo_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Gizmo Buffer"),
        contents: bytemuck::cast_slice(&GIZMO),
        usage: wgpu::BufferUsages::VERTEX,
    });

    let chunk_borders_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Chunk Borders Buffer"),
        contents: bytemuck::cast_slice(&CHUNK_BORDERS),
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
    });

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

    // TODO: faire les autres pipelines
    let pipelines = Pipelines::new(
        render_pipeline.clone(),
        render_pipeline.clone(),
        render_pipeline.clone(),
        render_pipeline.clone(),
        render_pipeline.clone(),
    );

    // TODO (wireframe)
    let debug_pipelines = Pipelines::new(
        wireframe_render_pipeline.clone(),
        wireframe_render_pipeline.clone(),
        wireframe_render_pipeline.clone(),
        wireframe_render_pipeline.clone(),
        wireframe_render_pipeline.clone(),
    );
    (
        gpu_context,
        texture_manager,
        textures_bind_group,
        render_camera,
        camera_buffer,
        camera_bind_group,
        gizmo_render_pipeline,
        gizmo_buffer,
        chunk_borders_buffer,
        depth_texture,
        depth_view,
        pipelines,
        debug_pipelines,
    )
}
