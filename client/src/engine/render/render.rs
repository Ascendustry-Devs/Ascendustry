use std::{time::Instant, u32::MAX};

use cgmath::{SquareMatrix, Vector3, Vector4};
use wgpu::{
    wgt::{CommandEncoderDescriptor, DrawIndirectArgs},
    BindGroup, Buffer, CommandEncoder, RenderPipeline, TextureView,
};

use crate::{
    common::geometry::vertex::{generate_cube, Vertex},
    engine::render::{camera::RenderCamera, manager::RenderManager, text::TextRenderer, texture::TextureArrayManager},
};
use shared::world::data::chunk::{CHUNK_SIZE, CHUNK_SIZE_F};

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

    pub world_wireframe_render_pipeline: RenderPipeline,
    pub world_render_pipeline: RenderPipeline,
    pub diffuse_bind_group: BindGroup,
    pub diffuse_texture_array: TextureArrayManager,

    pub camera_buffer: Buffer,
    pub camera_bind_group: BindGroup,

    pub gizmo_render_pipeline: RenderPipeline,
    pub gizmo_buffer: Buffer,

    pub wireframe: bool,
    pub show_chunk_borders: bool,

    pub chunk_borders_vertices: Vec<Vertex>,
    pub chunk_borders_buffer: Buffer,

    pub gpu_context: GpuContext,
    pub render_manager: RenderManager,

    pub render_options: RenderOptions,

    pub depth_texture: wgpu::Texture,
    pub depth_view: TextureView,

    pub frame_encoder: Option<CommandEncoder>,

    pub player_mesh: Buffer,

    pub remote_players: Vec<(f32, f32, f32)>,
}

pub struct GpuContext {
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
}

impl Renderer {
    pub fn new(
        is_surface_configured: bool,

        world_wireframe_render_pipeline: RenderPipeline,
        world_render_pipeline: RenderPipeline,
        diffuse_bind_group: BindGroup,
        diffuse_texture_array: TextureArrayManager,

        camera_buffer: Buffer,
        camera_bind_group: BindGroup,

        gizmo_render_pipeline: RenderPipeline,
        gizmo_buffer: Buffer,

        dimensions: (u32, u32),

        chunk_borders_vertices: Vec<Vertex>,
        chunk_borders_buffer: Buffer,

        gpu_context: GpuContext,
        render_manager: RenderManager,

        depth_texture: wgpu::Texture,
        depth_view: TextureView,

        player_mesh: Buffer,
    ) -> Self {
        let frame_encoder = gpu_context.device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Frame encoder"),
        });
        Self {
            is_surface_configured,

            world_wireframe_render_pipeline,
            world_render_pipeline,
            diffuse_bind_group,
            diffuse_texture_array,

            camera_buffer,
            camera_bind_group,

            gizmo_render_pipeline,
            gizmo_buffer,

            wireframe: WIREFRAME,
            show_chunk_borders: SHOW_CHUNK_BORDERS,

            chunk_borders_vertices,
            chunk_borders_buffer,

            gpu_context,
            render_manager,

            render_options: RenderOptions::new((dimensions.0 as f32) / (dimensions.1 as f32), 0.1, 1000.0),

            depth_texture,
            depth_view,

            frame_encoder: Some(frame_encoder),

            player_mesh,

            remote_players: Vec::new(),
        }
    }

    pub fn render<'a>(&'a mut self, camera: &RenderCamera, text_renderer: Option<&'a mut TextRenderer>) {
        if !self.is_surface_configured {
            return;
        }

        let surface = &self.gpu_context.surface;
        let device = &self.gpu_context.device;
        let queue = &self.gpu_context.queue;

        self.render_manager.update_indirect_buffer(device, queue);

        if let Some(encoder) = self.frame_encoder.take() {
            queue.submit(std::iter::once(encoder.finish()));
            self.render_manager.mesh_manager.process_pending_destructions();
        }

        queue.write_buffer(&self.camera_buffer, 0, bytemuck::cast_slice(&camera.get_view_proj_raw()));

        // TODO: CODE SERVANT À DESSINER LE DEBUG CHUNK - VIRER CETTE MERDE AU PLUS VITE (la rendre meilleure et extraire)

        // vp = projection * view
        let inv_vp = camera.get_view_proj().invert().expect("VP matrix is not invertible");

        // clip space du centre de l'écran
        // z = 0 pour le near plane (wgpu utilise NDC z ∈ 0..1)
        // w = 1 pour homogène
        let clip_pos = Vector4::new(0.0, 0.0, 0.0, 1.0);

        // world position
        let world_pos_h = inv_vp * clip_pos;
        let world_pos = Vector3::new(
            world_pos_h.x / world_pos_h.w,
            world_pos_h.y / world_pos_h.w,
            world_pos_h.z / world_pos_h.w,
        );

        let player_chunk_pos = [
            (world_pos.x / CHUNK_SIZE_F).floor() as i32 * CHUNK_SIZE,
            (world_pos.y / CHUNK_SIZE_F).floor() as i32 * CHUNK_SIZE,
            (world_pos.z / CHUNK_SIZE_F).floor() as i32 * CHUNK_SIZE,
        ];

        let debug_vertices: Vec<Vertex> = self
            .chunk_borders_vertices
            .iter()
            .map(|v| {
                Vertex::new_with_color(
                    v.position[0] + player_chunk_pos[0] as f32,
                    v.position[1] + player_chunk_pos[1] as f32,
                    v.position[2] + player_chunk_pos[2] as f32,
                    v.color,
                    MAX,
                    3.0,
                    0.0,
                    0.0,
                )
            })
            .collect();

        queue.write_buffer(&self.chunk_borders_buffer, 0, bytemuck::cast_slice(&debug_vertices));

        // FIN CODE DEBUG CHUNK

        let player_mesh_cube: Vec<Vertex> = generate_cube(world_pos.x, world_pos.y, world_pos.z);
        queue.write_buffer(&self.player_mesh, 0, bytemuck::cast_slice(&player_mesh_cube));

        let output = surface.get_current_texture().unwrap();

        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 1.0,
                            g: 1.0,
                            b: 1.0,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
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

            if self.wireframe {
                render_pass.set_pipeline(&self.world_wireframe_render_pipeline);
            } else {
                render_pass.set_pipeline(&self.world_render_pipeline);
            }

            render_pass.set_bind_group(0, &self.diffuse_bind_group, &[]);
            render_pass.set_bind_group(1, &self.camera_bind_group, &[]);

            let meshes = self.render_manager.get_meshes_to_render();

            let mut _rendered_mesh_count = meshes.len();

            let _start = Instant::now();

            // World & Player meshes (other than local player)
            if _rendered_mesh_count > 0 {
                render_pass.set_vertex_buffer(0, self.render_manager.mesh_manager.get_buffer().slice(..));

                const CAN_MULTIDRAW: bool = true; // TODO: detect if the device supports multi-draw indirect

                if CAN_MULTIDRAW {
                    render_pass.multi_draw_indirect(&self.render_manager.indirect_buffer.buffer(), 0, meshes.len() as u32);
                } else {
                    const CMD_SIZE: u64 = std::mem::size_of::<DrawIndirectArgs>() as u64;
                    for i in 0.._rendered_mesh_count as u32 {
                        render_pass.draw_indirect(&self.render_manager.indirect_buffer.buffer(), i as u64 * CMD_SIZE);
                    }
                }
            }

            // Debug
            if self.wireframe || self.show_chunk_borders {
                render_pass.set_pipeline(&self.gizmo_render_pipeline);
                if self.wireframe {
                    render_pass.set_vertex_buffer(0, self.gizmo_buffer.slice(..));
                    render_pass.draw(0..6, 0..1);
                }
                if self.show_chunk_borders {
                    render_pass.set_vertex_buffer(0, self.chunk_borders_buffer.slice(..));
                    render_pass.draw(0..self.chunk_borders_vertices.len() as u32, 0..1);
                }
            }
        }

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

        queue.submit(std::iter::once(encoder.finish()));
        output.present();

        self.frame_encoder = Some(device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Frame encoder"),
        }));

        self.render_manager.clear_render_queue();
    }
}
