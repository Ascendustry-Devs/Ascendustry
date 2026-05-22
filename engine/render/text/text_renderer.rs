use satiscore::utils::updatable::Updatable;
use wgpu::Device;
use wgpu_text::{glyph_brush::ab_glyph::FontRef, BrushBuilder, TextBrush};

pub const FPS_UPDATE_DELAY: f32 = 0.25;

pub struct TextRenderer {
    brush: TextBrush<FontRef<'static>>,
    dimensions: Updatable<(u32, u32)>,
    current_text: String,
    pub timer: f32,
}

impl TextRenderer {
    pub fn new(device: &Device, _queue: &wgpu::Queue, surface_format: wgpu::TextureFormat) -> Self {
        let font_data = include_bytes!("../../../assets/fonts/font.ttf");

        let brush: TextBrush<FontRef<'static>> =
            BrushBuilder::using_font_bytes(font_data)
                .unwrap()
                .build(device, 1024, 1024, surface_format);

        Self {
            brush,
            dimensions: Updatable::new((1024, 1024)),
            current_text: String::new(),
            timer: 0.0,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.dimensions.update((width, height));
    }

    pub fn update_text(&mut self, fps_avg: u32, fps_last: u32, dt: f32) {
        self.current_text = format!("FPS\navg {}\nlast {}\ndt {:.3}ms", fps_avg, fps_last, dt * 1000.0);
    }

    pub fn render<'a>(&'a mut self, device: &wgpu::Device, queue: &wgpu::Queue, render_pass: &mut wgpu::RenderPass<'a>) {
        use wgpu_text::glyph_brush::{Section, Text};

        if let Some(dimensions) = self.dimensions.change() {
            self.brush.resize_view(dimensions.0 as f32, dimensions.1 as f32, queue);
            self.dimensions.update(*self.dimensions.current());
        }

        let text = Text::new(&self.current_text).with_scale(30.0).with_color([1.0, 0.0, 0.0, 1.0]);

        let section = Section::default().with_text(vec![text]).with_screen_position((10.0, 10.0));

        self.brush.queue(device, queue, vec![section]).unwrap();
        self.brush.draw(render_pass);
    }

    pub fn dispose(&mut self) {
        // TODO: dispose lorsqu'il y aura des choses à disposer
    }
}
