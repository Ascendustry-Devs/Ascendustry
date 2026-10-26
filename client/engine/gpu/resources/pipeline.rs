use wgpu::RenderPipeline;

#[allow(unused)]
pub struct Pipelines {
    opaque: RenderPipeline,
    alpha_cutout: RenderPipeline,
    translucent: RenderPipeline,
    billboard: RenderPipeline,
    ui: RenderPipeline,
}

#[allow(unused)]
impl Pipelines {
    pub const fn new(
        opaque: RenderPipeline,
        alpha_cutout: RenderPipeline,
        translucent: RenderPipeline,
        billboard: RenderPipeline,
        ui: RenderPipeline,
    ) -> Self {
        Self {
            opaque,
            alpha_cutout,
            translucent,
            billboard,
            ui,
        }
    }

    pub const fn opaque(&self) -> &RenderPipeline {
        &self.opaque
    }

    pub const fn alpha_cutout(&self) -> &RenderPipeline {
        &self.alpha_cutout
    }

    pub const fn translucent(&self) -> &RenderPipeline {
        &self.translucent
    }

    pub const fn billboard(&self) -> &RenderPipeline {
        &self.billboard
    }

    pub const fn ui(&self) -> &RenderPipeline {
        &self.ui
    }
}
