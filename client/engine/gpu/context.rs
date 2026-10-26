use std::sync::{Arc, RwLock};

use wgpu::{
    wgt::{CommandEncoderDescriptor, DeviceDescriptor},
    CommandEncoder, ExperimentalFeatures, Features, Instance, InstanceDescriptor, Limits, PowerPreference, PresentMode,
    RequestAdapterOptions, Surface, SurfaceConfiguration, TextureUsages, Trace,
};
use winit::{event_loop::OwnedDisplayHandle, window::Window};

use crate::gpu::tools::GpuTools;

pub struct GpuContext {
    pub surface: Surface<'static>,
    pub tools: Arc<GpuTools>,
    pub frame_encoder: Arc<RwLock<CommandEncoder>>,
    pub config: SurfaceConfiguration,
    pub limits: Limits,
    pub features: Features,
}

impl GpuContext {
    pub fn new(window: Arc<Window>, display_handle: OwnedDisplayHandle) -> anyhow::Result<Self> {
        let size = window.inner_size();
        let instance = {
            let descriptor = InstanceDescriptor::new_with_display_handle(Box::new(display_handle));
            Instance::new(descriptor)
        };

        let surface = instance.create_surface(window).unwrap();

        let adapter_request = instance.request_adapter(&RequestAdapterOptions {
            power_preference: PowerPreference::default(),
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        });
        let adapter = pollster::block_on(adapter_request)?;

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

        let device_request = adapter.request_device(&DeviceDescriptor {
            label: None,
            required_features: features,
            experimental_features: ExperimentalFeatures::disabled(),
            required_limits: Limits::default(),
            memory_hints: Default::default(),
            trace: Trace::Off,
        });
        let (device, queue) = pollster::block_on(device_request)?;

        let frame_encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Frame encoder"),
        });
        let frame_encoder = Arc::new(RwLock::new(frame_encoder));

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
            frame_encoder,
            config,
            limits,
            features,
        })
    }

    pub fn get_tools(&self) -> Arc<GpuTools> {
        GpuTools::from_arc(&self.tools)
    }

    pub fn get_encoder(&self) -> Arc<RwLock<CommandEncoder>> {
        Arc::clone(&self.frame_encoder)
    }
}
