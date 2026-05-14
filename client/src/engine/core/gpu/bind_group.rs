use std::sync::Arc;

use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingResource,
    BindingType, Buffer, SamplerBindingType, ShaderStages, TextureSampleType, TextureViewDimension,
};

use crate::engine::render::{render::GpuTools, textures::array::Texture2DArray};

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

    pub fn make_camera(&self) -> BindGroupLayout {
        let entries = [BindGroupLayoutEntry {
            binding: 0,
            visibility: ShaderStages::VERTEX,
            ty: BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }];
        self.make(Some("Camera"), &entries)
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

    pub fn make_texture_array(&self, layout: &BindGroupLayout, array: &Texture2DArray, label: Option<&str>) -> BindGroup {
        let entries = [
            BindGroupEntry {
                binding: 0,
                resource: BindingResource::TextureView(array.view()),
            },
            BindGroupEntry {
                binding: 1,
                resource: BindingResource::Sampler(array.sampler()),
            },
        ];
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
