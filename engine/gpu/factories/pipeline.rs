use std::sync::Arc;

use wgpu::{
    BindGroupLayout, DepthStencilState, FragmentState, PipelineLayout, PipelineLayoutDescriptor, PrimitiveState, RenderPipeline,
    VertexState,
};

use crate::gpu::tools::GpuTools;

pub struct PipelineLayoutFactory {
    gpu_tools: Arc<GpuTools>,
}

impl PipelineLayoutFactory {
    pub fn new(gpu_tools: Arc<GpuTools>) -> Self {
        Self { gpu_tools }
    }

    pub fn make(&self, label: Option<&str>, bind_group_layouts: &[Option<&BindGroupLayout>]) -> PipelineLayout {
        let descriptor = PipelineLayoutDescriptor {
            label: label,
            bind_group_layouts: bind_group_layouts,
            immediate_size: 0,
        };
        self.gpu_tools.device().create_pipeline_layout(&descriptor)
    }
}

pub struct PipelineFactory {
    gpu_tools: Arc<GpuTools>,
}

impl PipelineFactory {
    pub fn new(gpu_tools: Arc<GpuTools>) -> Self {
        Self { gpu_tools }
    }

    pub fn make(
        &self,
        label: &str,
        layout: &PipelineLayout,
        vertex: VertexState,
        fragment: FragmentState,
        primitive: PrimitiveState,
        depth_stencil: Option<DepthStencilState>,
    ) -> RenderPipeline {
        let descriptor = wgpu::RenderPipelineDescriptor {
            label: Some(label),
            layout: Some(layout),
            vertex: vertex,
            fragment: Some(fragment),
            primitive: primitive,
            depth_stencil: depth_stencil,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        };
        self.gpu_tools.device().create_render_pipeline(&descriptor)
    }
}
