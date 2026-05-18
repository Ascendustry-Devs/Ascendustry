use std::{iter, mem, sync::Arc};

use shared::world::data::chunk::CHUNK_SIZE_F;
use wgpu::{
    wgt::{CommandEncoderDescriptor, DeviceDescriptor, DrawIndirectArgs},
    Backends, BindGroup, Buffer, CommandEncoder, Device, ExperimentalFeatures, Features, Instance, InstanceDescriptor, Limits,
    PowerPreference, PresentMode, Queue, RenderPass, RenderPipeline, RequestAdapterOptions, Surface, SurfaceConfiguration, Texture,
    TextureUsages, TextureView, Trace,
};
use winit::window::Window;

use crate::{
    common::geometry::vertex::Vertex,
    engine::{
        core::gpu::pipeline::Pipelines,
        render::{camera::RenderCamera, manager::RenderManager, text::TextRenderer, texture::TextureManager},
    },
};

const WIREFRAME: bool = false;
const SHOW_CHUNK_BORDERS: bool = false;

pub struct RenderOptions {
    pub aspect: f32,
    pub znear: f32,
    pub zfar: f32,
}

impl RenderOptions {
    pub fn new(aspect: f32, znear: f32, zfar: f32) -> Self {
        Self { aspect, znear, zfar }
    }
}

pub struct Renderer {
    pub is_surface_configured: bool,

    pub render_manager: RenderManager,
    pub render_options: RenderOptions,
    pub texture_manager: TextureManager,

    pub gpu_context: GpuContext,
    pub gpu_resources: GpuResources,

    pub debug: DebugRenderResources,
}

pub struct GpuTools {
    device: Device,
    queue: Queue,
}

pub struct GpuResources {
    pub pipelines: Pipelines,

    pub camera_bind_group: BindGroup,
    pub texture_bind_group: BindGroup,

    pub camera_buffer: Buffer,

    pub depth_texture: Texture,
    pub depth_view: TextureView,

    pub frame_encoder: Option<CommandEncoder>,
}

impl GpuResources {
    pub fn new(
        pipelines: Pipelines,

        camera_bind_group: BindGroup,
        texture_bind_group: BindGroup,

        camera_buffer: Buffer,

        depth_texture: Texture,
        depth_view: TextureView,

        frame_encoder: Option<CommandEncoder>,
    ) -> Self {
        Self {
            pipelines,
            camera_bind_group,
            texture_bind_group,
            camera_buffer,
            depth_texture,
            depth_view,
            frame_encoder,
        }
    }
}

impl GpuTools {
    pub fn new(device: Device, queue: Queue) -> Self {
        Self { device, queue }
    }

    pub fn device(&self) -> &Device {
        &self.device
    }

    pub fn queue(&self) -> &Queue {
        &self.queue
    }
}

pub struct GpuContext {
    pub surface: Surface<'static>,
    pub tools: Arc<GpuTools>,
    pub config: SurfaceConfiguration,
    pub limits: Limits,
    pub features: Features,
}

impl GpuContext {
    pub fn new(window: Arc<Window>) -> anyhow::Result<Self> {
        let size = window.inner_size();
        let instance = Instance::new(&InstanceDescriptor {
            backends: Backends::PRIMARY,
            ..Default::default()
        });

        let surface = instance.create_surface(window).unwrap();

        let adapter = pollster::block_on(instance.request_adapter(&RequestAdapterOptions {
            power_preference: PowerPreference::default(),
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))?;

        let features = {
            let mut requested = vec![
                Features::CONSERVATIVE_RASTERIZATION,
                Features::POLYGON_MODE_LINE,
                Features::MULTI_DRAW_INDIRECT_COUNT,
            ];

            requested.retain(|value| adapter.features().contains(*value));
            let result = requested.iter().fold(Features::empty(), |acc, value| acc.union(*value));
            result
        };

        let (device, queue) = pollster::block_on(adapter.request_device(&DeviceDescriptor {
            label: None,
            required_features: features,
            experimental_features: ExperimentalFeatures::disabled(),
            required_limits: Limits::default(),
            memory_hints: Default::default(),
            trace: Trace::Off,
        }))?;

        let tools = Arc::new(GpuTools::new(device, queue));

        let limits = tools.device().limits();
        let features = tools.device().features().intersection(adapter.features());

        let surface_caps = surface.get_capabilities(&adapter);

        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);
        let config = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: PresentMode::AutoNoVsync,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        Ok(Self {
            surface,
            tools,
            config,
            limits,
            features,
        })
    }

    pub fn get_tools(&mut self) -> Arc<GpuTools> {
        Arc::clone(&self.tools)
    }
}

pub struct DebugRenderResources {
    pub wireframe: bool,
    pub pipelines: Pipelines,
    pub gizmo_render_pipeline: RenderPipeline,
    pub gizmo_buffer: Buffer,
    pub show_chunk_borders: bool,
    pub chunk_borders_buffer: Buffer,
}

impl DebugRenderResources {
    pub fn new(pipelines: Pipelines, gizmo_render_pipeline: RenderPipeline, gizmo_buffer: Buffer, chunk_borders_buffer: Buffer) -> Self {
        Self {
            wireframe: false,
            pipelines,
            gizmo_render_pipeline,
            gizmo_buffer,
            show_chunk_borders: false,
            chunk_borders_buffer,
        }
    }

    pub fn pass(&mut self, render_pass: &mut RenderPass) {
        if self.wireframe || self.show_chunk_borders {
            render_pass.set_pipeline(&self.gizmo_render_pipeline);
            if self.wireframe {
                render_pass.set_vertex_buffer(0, self.gizmo_buffer.slice(..));
                render_pass.draw(0..6, 0..1);
            }
            if self.show_chunk_borders {
                render_pass.set_vertex_buffer(0, self.chunk_borders_buffer.slice(..));
                render_pass.draw(0..24, 0..1);
            }
        }
    }
}

impl Renderer {
    pub fn new(
        is_surface_configured: bool,

        render_manager: RenderManager,
        render_options: RenderOptions,
        texture_manager: TextureManager,

        gpu_context: GpuContext,
        gpu_resources: GpuResources,

        debug: DebugRenderResources,
    ) -> Self {
        Self {
            is_surface_configured,

            render_manager,
            render_options,
            texture_manager,

            gpu_context,
            gpu_resources,

            debug,
        }
    }

    fn world_pass(&self, render_pass: &mut RenderPass) {
        // World & Player meshes (other than local player)
        let mesh_count = self.render_manager.get_meshes_to_render().len() as u32;
        if mesh_count > 0 {
            render_pass.set_vertex_buffer(0, self.render_manager.mesh_manager.get_buffer().slice(..));

            let can_multidraw = self.gpu_context.features.contains(Features::MULTI_DRAW_INDIRECT_COUNT); // TODO: detect if the device supports multi-draw indirect

            if can_multidraw {
                render_pass.multi_draw_indirect(&self.render_manager.indirect_buffer.buffer(), 0, mesh_count);
            } else {
                const CMD_SIZE: u64 = mem::size_of::<DrawIndirectArgs>() as u64;
                for i in 0..mesh_count {
                    render_pass.draw_indirect(&self.render_manager.indirect_buffer.buffer(), i as u64 * CMD_SIZE);
                }
            }
        }
    }

    pub fn render<'a>(&'a mut self, camera: &RenderCamera, text_renderer: Option<&'a mut TextRenderer>) {
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

        if !self.is_surface_configured {
            return;
        }

        let device = self.gpu_context.tools.device();
        let queue = self.gpu_context.tools.queue();
        self.render_manager.update_indirect_buffer(device, queue);

        let mut encoder =
            self.gpu_resources
                .frame_encoder
                .take()
                .unwrap_or(device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Render Encoder"),
                }));

        if let Some(view_proj) = camera.view_proj().change() {
            queue.write_buffer(&self.gpu_resources.camera_buffer, 0, bytemuck::cast_slice(view_proj));
        }

        if let Some(cw) = camera.cw().change() {
            let chunk_borders_vertices: Vec<Vertex> = CHUNK_BORDERS
                .iter()
                .map(|v| v.copy_with_pos(v.position[0] + cw[0], v.position[1] + cw[1], v.position[2] + cw[2]))
                .collect();

            queue.write_buffer(&self.debug.chunk_borders_buffer, 0, bytemuck::cast_slice(&chunk_borders_vertices));
        };

        let output = self.gpu_context.surface.get_current_texture().unwrap();
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

        // World pass
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.125,
                            g: 0.5,
                            b: 0.75,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.gpu_resources.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });

            let pipelines = match self.debug.wireframe {
                true => &self.debug.pipelines,
                false => &self.gpu_resources.pipelines,
            };

            render_pass.set_pipeline(pipelines.opaque());

            render_pass.set_bind_group(0, &self.gpu_resources.texture_bind_group, &[]);
            render_pass.set_bind_group(1, &self.gpu_resources.camera_bind_group, &[]);

            self.world_pass(&mut render_pass);
            self.debug.pass(&mut render_pass);
        }

        // UI Pass
        if let Some(text_renderer) = text_renderer {
            let mut text_render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Text Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });
            text_renderer.render(device, queue, &mut text_render_pass);
        }

        queue.submit(iter::once(encoder.finish()));
        output.present();

        self.gpu_resources.frame_encoder = Some(device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Frame encoder"),
        }));

        self.render_manager.mesh_manager.process_pending_destructions();
        self.render_manager.clear_render_queue();
    }

    pub fn dispose(&mut self) {
        self.is_surface_configured = false;

        self.gpu_resources.camera_buffer.destroy();
        self.debug.chunk_borders_buffer.destroy();
        self.gpu_resources.depth_texture.destroy();
        // TODO: faire fonctionner -> self.diffuse_texture_array.dispose();
        self.debug.gizmo_buffer.destroy();
        // TODO: faire fonctionner -> self.gpu_context.dispose();
        // TODO: faire fonctionner -> self.render_manager.dispose();
    }
}
