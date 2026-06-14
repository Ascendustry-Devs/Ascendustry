use std::sync::Arc;

use wgpu::{
    BindGroupLayout, BlendState, ColorTargetState, ColorWrites, CompareFunction, DepthBiasState, DepthStencilState, Face,
    Features, FragmentState, FrontFace, MultisampleState, PipelineLayout, PipelineLayoutDescriptor, PolygonMode,
    PrimitiveState, PrimitiveTopology, RenderPipeline, RenderPipelineDescriptor, ShaderModuleDescriptor, ShaderSource,
    SurfaceConfiguration, TextureFormat, VertexState,
};

use crate::gpu::{layouts::BufferLayouts, tools::GpuTools};

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
        let descriptor = RenderPipelineDescriptor {
            label: Some(label),
            layout: Some(layout),
            vertex: vertex,
            fragment: Some(fragment),
            primitive: primitive,
            depth_stencil: depth_stencil,
            multisample: MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        };
        self.gpu_tools.device().create_render_pipeline(&descriptor)
    }

    pub fn make_opaque(
        &self,
        layout: &PipelineLayout,
        config: &SurfaceConfiguration,
        _features: &Features,
    ) -> (RenderPipeline, RenderPipeline) {
        let device = self.gpu_tools.device();

        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Opaque Shader"),
            source: ShaderSource::Wgsl(include_str!("../../../assets/shaders/shader.wgsl").into()),
        });

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
            compilation_options: Default::default(),
        };

        let primitive = PrimitiveState {
            topology: PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: FrontFace::Ccw,
            cull_mode: Some(Face::Back),
            polygon_mode: PolygonMode::Fill,
            unclipped_depth: false,
            conservative: false,
            // conservative: features.contains(Features::CONSERVATIVE_RASTERIZATION),
        };

        let wireframe = PrimitiveState {
            topology: PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: FrontFace::Ccw,
            cull_mode: Some(Face::Back),
            polygon_mode: PolygonMode::Line,
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

        (
            self.make(
                "Opaque",
                layout,
                vertex.clone(),
                fragment.clone(),
                primitive,
                Some(depth_stencil.clone()),
            ),
            self.make("Opaque Wireframe", layout, vertex, fragment, wireframe, Some(depth_stencil)),
        )
    }

    pub fn make_gizmo(&self, layout: &PipelineLayout, config: &SurfaceConfiguration) -> RenderPipeline {
        let device = self.gpu_tools.device();

        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Opaque Shader"),
            source: ShaderSource::Wgsl(include_str!("../../../assets/shaders/shader.wgsl").into()),
        });

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
            compilation_options: Default::default(),
        };

        let primitive = PrimitiveState {
            topology: PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: FrontFace::Ccw,
            cull_mode: Some(Face::Back),
            polygon_mode: PolygonMode::Line,
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

        self.make("Gizmo", layout, vertex, fragment, primitive, Some(depth_stencil))
    }

    pub fn make_ui(&self, layout: &PipelineLayout, config: &SurfaceConfiguration) -> RenderPipeline {
        let device = self.gpu_tools.device();

        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("UI Shader"),
            source: ShaderSource::Wgsl(include_str!("../../../assets/shaders/ui.wgsl").into()),
        });

        let vertex = VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[BufferLayouts::ui_vertex()],
            compilation_options: Default::default(),
        };

        let fragment = FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(ColorTargetState {
                format: config.format,
                blend: Some(BlendState::ALPHA_BLENDING),
                write_mask: ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        };

        let primitive = PrimitiveState {
            topology: PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: FrontFace::Ccw,
            cull_mode: None,
            polygon_mode: PolygonMode::Fill,
            unclipped_depth: false,
            conservative: false,
            // conservative: features.contains(Features::CONSERVATIVE_RASTERIZATION),
        };

        self.make("UI", layout, vertex, fragment, primitive, None)
    }
}
