use crate::{
    audio::GameAudioManager,
    core::{
        application::AppState,
        frame::{EngineFrameData, GameFrameData},
    },
    gpu::{
        context::GpuContext,
        factories::wrapper::GpuFactory,
        resources::{pipeline::Pipelines, wrapper::GpuResources},
        textures::manager::TextureManager,
    },
    render::{
        camera::RenderCamera,
        debug::DebugRenderResources,
        manager::RenderManager,
        options::RenderOptions,
        render::Renderer,
        text::{text_renderer::FPS_UPDATE_DELAY, TextRenderer},
        ui::render::UiRenderer,
    },
};
use game::world::data::chunk::CHUNK_SIZE_F;

use crate::geometry::vertex::Vertex;
use std::time::Instant;
use std::{num::NonZero, sync::Arc};
use wgpu::{
    util::DeviceExt, wgt::BufferDescriptor, BindGroupEntry, BindGroupLayout, BindingResource, BufferBinding, BufferUsages,
    CurrentSurfaceTexture, PipelineLayout,
};
use winit::{event_loop::ActiveEventLoop, window::Window};

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
        let gpu_context = GpuContext::new(Arc::clone(&window), event_loop.owned_display_handle()).unwrap();
        let device = gpu_context.tools.device();
        let config = &gpu_context.config;
        let mut texture_manager = TextureManager::new(
            gpu_context.get_tools(),
            gpu_context.limits.max_texture_dimension_2d,
            gpu_context.limits.max_texture_array_layers,
        );
        let atlas = image::open("assets/ui/texture_atlas.png").unwrap();
        let _id = texture_manager
            .register_atlas(atlas.as_bytes(), 0, 0, atlas.width(), atlas.height())
            .unwrap();
        let render_camera = RenderCamera::new();
        let camera_buffer = gpu_context.tools.device().create_buffer(&BufferDescriptor {
            label: Some("Camera Buffer"),
            size: size_of::<[[f32; 4]; 4]>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let gpu_factory = GpuFactory::new(&gpu_context);
        let (
            texture_bind_group_layout,
            camera_bind_group_layout,
            ui_uniform_bind_group_layout,
            opaque_render_pipeline_layout,
            ui_render_pipeline_layout,
        ) = make_bind_group_layouts(&gpu_factory);
        let textures_bind_group = gpu_factory.bind_group().make_textures_entries(
            &texture_bind_group_layout,
            &texture_manager,
            Some("Texture Bind Group"),
        );
        let camera_bind_group = gpu_factory
            .bind_group()
            .make_camera(&camera_bind_group_layout, &camera_buffer);
        let ui_renderer = UiRenderer::new(gpu_context.get_tools());
        let ui_uniform_bind_group = gpu_factory.bind_group().make(
            Some("UI Uniform"),
            &ui_uniform_bind_group_layout,
            &[BindGroupEntry {
                binding: 0,
                resource: BindingResource::Buffer(BufferBinding {
                    buffer: ui_renderer.proj_buffer(),
                    offset: 0,
                    size: NonZero::new(size_of::<[[f32; 4]; 4]>() as u64),
                }),
            }],
        );
        let (opaque_render_pipeline, opaque_wireframe_render_pipeline) =
            gpu_factory
                .pipeline()
                .make_opaque(&opaque_render_pipeline_layout, config, &gpu_context.features);
        let gizmo_render_pipeline = gpu_factory.pipeline().make_gizmo(&opaque_render_pipeline_layout, config);
        let ui_render_pipeline = gpu_factory.pipeline().make_ui(&ui_render_pipeline_layout, config);
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
        let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Depth Texture"),
            size: wgpu::Extent3d {
                width: size.width,
                height: size.height,
                depth_or_array_layers: 1,
            },
            view_formats: &[wgpu::TextureFormat::Depth32Float],
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        });
        let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let pipelines = Pipelines::new(
            opaque_render_pipeline.clone(),
            opaque_render_pipeline.clone(),
            opaque_render_pipeline.clone(),
            opaque_render_pipeline.clone(),
            ui_render_pipeline,
        );
        let debug_pipelines = Pipelines::new(
            opaque_wireframe_render_pipeline.clone(),
            opaque_wireframe_render_pipeline.clone(),
            opaque_wireframe_render_pipeline.clone(),
            opaque_wireframe_render_pipeline.clone(),
            opaque_wireframe_render_pipeline.clone(),
        );

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
            textures_bind_group,
            ui_uniform_bind_group,
            camera_buffer,
            depth_texture,
            depth_view,
        );

        let debug = DebugRenderResources::new(debug_pipelines, gizmo_render_pipeline, gizmo_buffer, chunk_borders_buffer);

        ui_renderer.update_proj(size.width, size.height);

        let renderer = Renderer::new(
            false,
            render_options,
            render_manager,
            texture_manager,
            ui_renderer,
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
            self.renderer.ui_renderer.update_proj(width, height);
            self.renderer.gpu_resources.depth_texture =
                self.renderer
                    .gpu_context
                    .tools
                    .device()
                    .create_texture(&wgpu::TextureDescriptor {
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
        self.renderer.render_manager.world_buffer.write().unwrap().flush();

        let output = self.renderer.gpu_context.surface.get_current_texture();
        match output {
            CurrentSurfaceTexture::Success(surface_texture) => {
                self.renderer.render(
                    &self.window,
                    surface_texture,
                    &self.game_frame_data.camera,
                    Some(&mut self.text_renderer),
                );
            }
            CurrentSurfaceTexture::Suboptimal(old) => {
                drop(old);
                let (width, height) = self.window.inner_size().into();
                self.resize(width, height);
                // reconfigure
                return;
            }
            CurrentSurfaceTexture::Outdated => {
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

fn make_bind_group_layouts(
    gpu_factory: &GpuFactory,
) -> (
    BindGroupLayout,
    BindGroupLayout,
    BindGroupLayout,
    PipelineLayout,
    PipelineLayout,
) {
    let texture_bind_group_layout = gpu_factory
        .bind_group_layout()
        .make_textures(Some("Texture Bind Group Layout"));
    let camera_bind_group_layout = gpu_factory.bind_group_layout().make_camera();
    let ui_uniform_bind_group_layout = gpu_factory.bind_group_layout().make_ui_uniform();
    let opaque_render_pipeline_layout = gpu_factory
        .pipeline_layout()
        .make(None, &[Some(&texture_bind_group_layout), Some(&camera_bind_group_layout)]);
    let ui_render_pipeline_layout = gpu_factory
        .pipeline_layout()
        .make(None, &[Some(&texture_bind_group_layout), Some(&ui_uniform_bind_group_layout)]);
    (
        texture_bind_group_layout,
        camera_bind_group_layout,
        ui_uniform_bind_group_layout,
        opaque_render_pipeline_layout,
        ui_render_pipeline_layout,
    )
}
