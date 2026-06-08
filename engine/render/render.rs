use std::{iter, mem};

use game::world::data::chunk::CHUNK_SIZE_F;

use crate::{geometry::vertex::Vertex, render::ui::render::UiRenderer};
use wgpu::{
    wgt::{CommandEncoderDescriptor, DrawIndirectArgs},
    Features, RenderPass, SurfaceTexture,
};

use crate::{
    gpu::{context::GpuContext, resources::wrapper::GpuResources, textures::manager::TextureManager},
    render::{
        camera::RenderCamera, debug::DebugRenderResources, manager::RenderManager, options::RenderOptions, text::TextRenderer,
    },
};

pub struct Renderer {
    pub is_surface_configured: bool,

    pub render_options: RenderOptions,

    pub render_manager: RenderManager,
    pub texture_manager: TextureManager,
    pub ui_renderer: UiRenderer,

    pub gpu_context: GpuContext,
    pub gpu_resources: GpuResources,

    pub debug: DebugRenderResources,
}

impl Renderer {
    pub fn new(
        is_surface_configured: bool,

        render_options: RenderOptions,

        render_manager: RenderManager,
        texture_manager: TextureManager,
        ui_renderer: UiRenderer,

        gpu_context: GpuContext,
        gpu_resources: GpuResources,

        debug: DebugRenderResources,
    ) -> Self {
        Self {
            is_surface_configured,

            render_options,

            render_manager,
            texture_manager,
            ui_renderer,

            gpu_context,
            gpu_resources,

            debug,
        }
    }

    fn world_pass(&self, render_pass: &mut RenderPass) {
        // World & Player meshes (other than local player)
        let mesh_count = self.render_manager.ids_to_render.len() as u32;
        let alloc = &self.render_manager.world_buffer.read().unwrap();
        if mesh_count > 0 {
            render_pass.set_vertex_buffer(0, alloc.get_buffer().slice(..));

            let can_multidraw = self.gpu_context.features.contains(Features::MULTI_DRAW_INDIRECT_COUNT);

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

    pub fn render<'a>(
        &'a mut self,
        output: SurfaceTexture,
        camera: &RenderCamera,
        text_renderer: Option<&'a mut TextRenderer>,
    ) {
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
        self.render_manager.update_indirect_buffer();

        let encoder = self.gpu_context.get_encoder();
        let mut encoder = encoder.write().unwrap();

        if let Some(view_proj) = camera.view_proj().change() {
            queue.write_buffer(&self.gpu_resources.camera_buffer, 0, bytemuck::cast_slice(view_proj));
        }

        if let Some(cw) = camera.cw().change() {
            let chunk_borders_vertices =
                CHUNK_BORDERS.map(|v| v.copy_with_pos(v.position[0] + cw[0], v.position[1] + cw[1], v.position[2] + cw[2]));

            queue.write_buffer(
                &self.debug.chunk_borders_buffer,
                0,
                bytemuck::cast_slice(&chunk_borders_vertices),
            );
        };

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
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("UI Render Pass"),
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

            self.ui_renderer.render(
                &mut render_pass,
                self.gpu_resources.pipelines.ui(),
                &self.gpu_resources.texture_bind_group,
                &self.gpu_resources.ui_uniform_bind_group,
            );

            if let Some(text_renderer) = text_renderer {
                text_renderer.render(device, queue, &mut render_pass);
            }
        }

        let encoder = {
            let new_encoder = device.create_command_encoder(&CommandEncoderDescriptor {
                label: Some("Frame encoder"),
            });

            mem::replace(&mut (*encoder), new_encoder)
        };

        queue.submit(iter::once(encoder.finish()));
        output.present();

        self.render_manager
            .world_buffer
            .write()
            .unwrap()
            .process_pending_destructions();
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
