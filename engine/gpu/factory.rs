use crate::gpu::{
    context::GpuContext,
    resources::{
        bind_group::{BindGroupFactory, BindGroupLayoutFactory},
        pipeline::{PipelineFactory, PipelineLayoutFactory},
    },
};

pub struct GpuFactory {
    bind_group_layout: BindGroupLayoutFactory,
    bind_group: BindGroupFactory,
    pipeline_layout: PipelineLayoutFactory,
    pipeline: PipelineFactory,
}

impl GpuFactory {
    pub fn new(ctx: &GpuContext) -> Self {
        let bind_group_layout = BindGroupLayoutFactory::new(ctx.get_tools());
        let bind_group = BindGroupFactory::new(ctx.get_tools());
        let pipeline_layout = PipelineLayoutFactory::new(ctx.get_tools());
        let pipeline = PipelineFactory::new(ctx.get_tools());

        Self {
            bind_group_layout,
            bind_group,
            pipeline_layout,
            pipeline,
        }
    }

    pub fn bind_group_layout(&self) -> &BindGroupLayoutFactory {
        &self.bind_group_layout
    }

    pub fn bind_group(&self) -> &BindGroupFactory {
        &self.bind_group
    }

    pub fn pipeline_layout(&self) -> &PipelineLayoutFactory {
        &self.pipeline_layout
    }

    pub fn pipeline(&self) -> &PipelineFactory {
        &self.pipeline
    }
}
