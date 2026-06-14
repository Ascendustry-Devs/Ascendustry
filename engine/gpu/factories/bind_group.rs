use std::{num::NonZero, sync::Arc};

use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor, BindGroupLayoutEntry,
    BindingResource, BindingType, Buffer, BufferBindingType, SamplerBindingType, ShaderStages, TextureSampleType,
    TextureViewDimension,
};

use crate::{
    gpu::{
        textures::{array::Texture2DArray, atlas::Texture2DAtlas, manager::TextureManager},
        tools::GpuTools,
    },
    render::modes::RenderMode,
};

pub struct BindGroupLayoutFactory {
    gpu_tools: Arc<GpuTools>,
}

impl BindGroupLayoutFactory {
    pub fn new(gpu_tools: Arc<GpuTools>) -> Self {
        Self { gpu_tools }
    }

    pub fn make(&self, label: Option<&str>, entries: &[BindGroupLayoutEntry]) -> BindGroupLayout {
        self.gpu_tools
            .device()
            .create_bind_group_layout(&BindGroupLayoutDescriptor { label, entries })
    }

    pub fn make_texture_array(&self, label: Option<&str>) -> BindGroupLayout {
        let entries = [
            BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Texture {
                    view_dimension: TextureViewDimension::D2Array,
                    multisampled: false,
                    sample_type: TextureSampleType::Float { filterable: true },
                },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 1,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Sampler(SamplerBindingType::Filtering),
                count: None,
            },
        ];
        self.make(label, &entries)
    }

    pub fn make_texture_array_entry(&self, binding: u32) -> [BindGroupLayoutEntry; 2] {
        [
            BindGroupLayoutEntry {
                binding: binding,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Texture {
                    view_dimension: TextureViewDimension::D2Array,
                    multisampled: false,
                    sample_type: TextureSampleType::Float { filterable: true },
                },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: binding + 1,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Sampler(SamplerBindingType::Filtering),
                count: None,
            },
        ]
    }

    pub fn make_texture_atlas_entry(&self, binding: u32) -> [BindGroupLayoutEntry; 2] {
        [
            BindGroupLayoutEntry {
                binding: binding,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Texture {
                    view_dimension: TextureViewDimension::D2,
                    multisampled: false,
                    sample_type: TextureSampleType::Float { filterable: true },
                },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: binding + 1,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Sampler(SamplerBindingType::Filtering),
                count: None,
            },
        ]
    }

    pub fn make_textures(&self, label: Option<&str>) -> BindGroupLayout {
        let opaque = self.make_texture_array_entry(0);
        let alpha_cutout = self.make_texture_array_entry(2);
        let translucent = self.make_texture_array_entry(4);
        let billboard = self.make_texture_array_entry(6);
        let ui = self.make_texture_atlas_entry(8);

        let entries = [
            opaque[0],
            opaque[1],
            alpha_cutout[0],
            alpha_cutout[1],
            translucent[0],
            translucent[1],
            billboard[0],
            billboard[1],
            ui[0],
            ui[1],
        ];

        self.make(label, &entries)
    }

    pub fn make_camera(&self) -> BindGroupLayout {
        let entries = [BindGroupLayoutEntry {
            binding: 0,
            visibility: ShaderStages::VERTEX,
            ty: BindingType::Buffer {
                ty: BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }];
        self.make(Some("Camera"), &entries)
    }

    pub fn make_ui_uniform(&self) -> BindGroupLayout {
        let entries = [BindGroupLayoutEntry {
            binding: 0,
            visibility: ShaderStages::VERTEX,
            ty: BindingType::Buffer {
                ty: BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: NonZero::new(size_of::<[[f32; 4]; 4]>() as u64),
            },
            count: None,
        }];
        self.make(Some("UI Render Pipeline Layout"), &entries)
    }
}

pub struct BindGroupFactory {
    gpu_tools: Arc<GpuTools>,
}

impl BindGroupFactory {
    pub fn new(gpu_tools: Arc<GpuTools>) -> Self {
        Self { gpu_tools }
    }

    pub fn make(&self, label: Option<&str>, layout: &BindGroupLayout, entries: &[BindGroupEntry]) -> BindGroup {
        self.gpu_tools
            .device()
            .create_bind_group(&BindGroupDescriptor { label, layout, entries })
    }

    pub fn make_texture_array_entry<'a>(&'a self, binding: u32, array: &'a Texture2DArray) -> [BindGroupEntry<'a>; 2] {
        [
            BindGroupEntry {
                binding: binding,
                resource: BindingResource::TextureView(array.view()),
            },
            BindGroupEntry {
                binding: binding + 1,
                resource: BindingResource::Sampler(array.sampler()),
            },
        ]
    }

    pub fn make_texture_atlas_entry<'a>(&'a self, binding: u32, atlas: &'a Texture2DAtlas) -> [BindGroupEntry<'a>; 2] {
        [
            BindGroupEntry {
                binding: binding,
                resource: BindingResource::TextureView(atlas.view()),
            },
            BindGroupEntry {
                binding: binding + 1,
                resource: BindingResource::Sampler(atlas.sampler()),
            },
        ]
    }

    pub fn make_textures_entries(
        &self,
        layout: &BindGroupLayout,
        texture_manager: &TextureManager,
        label: Option<&str>,
    ) -> BindGroup {
        let opaque_array = texture_manager.get_array(&RenderMode::Opaque);
        let alpha_cutout_array = texture_manager.get_array(&RenderMode::AlphaCutout);
        let translucent_array = texture_manager.get_array(&RenderMode::Translucent);
        let billboard_array = texture_manager.get_array(&RenderMode::Billboard);
        let ui_atlas = texture_manager.get_ui_atlas();

        let entries: Vec<BindGroupEntry> = [
            self.make_texture_array_entry(0, opaque_array),
            self.make_texture_array_entry(2, alpha_cutout_array),
            self.make_texture_array_entry(4, translucent_array),
            self.make_texture_array_entry(6, billboard_array),
            self.make_texture_atlas_entry(8, ui_atlas),
        ]
        .into_iter()
        .flatten()
        .collect();

        self.make(label, layout, &entries)
    }

    pub fn make_camera(&self, layout: &BindGroupLayout, camera_buffer: &Buffer) -> BindGroup {
        let entries = [BindGroupEntry {
            binding: 0,
            resource: camera_buffer.as_entire_binding(),
        }];
        self.make(Some("Camera"), layout, &entries)
    }
}
