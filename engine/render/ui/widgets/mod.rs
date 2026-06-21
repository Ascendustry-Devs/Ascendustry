use project_core::utils::data_manager::Id;

use crate::render::ui::widgets::{list::List, panel::Panel, textured_panel::TexturedPanel};

pub mod list;
pub mod panel;
pub mod textured_panel;
// pub mod window;

#[derive(Clone)]
pub struct WidgetTransform {
    x: u32,
    y: u32,
    w: u32,
    h: u32,
}

impl WidgetTransform {
    pub const fn new(x: u32, y: u32, w: u32, h: u32) -> Self {
        Self { x, y, w, h }
    }

    pub const fn from_lrtb(space: (u32, u32), l: u32, r: u32, t: u32, b: u32) -> Self {
        let x = l;
        let y = t;
        let w = space.0 - l - r;
        let h = space.1 - t - b;
        Self::new(x, y, w, h)
    }

    pub const fn x(&self) -> u32 {
        self.x
    }

    pub const fn y(&self) -> u32 {
        self.y
    }

    pub const fn w(&self) -> u32 {
        self.w
    }

    pub const fn h(&self) -> u32 {
        self.h
    }

    pub const fn extract(&self) -> (u32, u32, u32, u32) {
        (self.x(), self.y(), self.w(), self.h())
    }
}

impl Default for WidgetTransform {
    fn default() -> Self {
        Self { x: 0, y: 0, w: 1, h: 1 }
    }
}

pub trait Widget {
    fn transform(&self) -> &WidgetTransform;
    fn draw(&self, commands: &mut Vec<DrawCommand>);

    fn x(&self) -> u32 {
        self.transform().x()
    }

    fn y(&self) -> u32 {
        self.transform().y()
    }

    fn pos(&self) -> (u32, u32) {
        let transform = self.transform();
        (transform.x(), transform.y())
    }

    fn w(&self) -> u32 {
        self.transform().w()
    }

    fn h(&self) -> u32 {
        self.transform().h()
    }

    fn dim(&self) -> (u32, u32) {
        let transform = self.transform();
        (transform.w(), transform.h())
    }
}

pub enum WidgetType {
    List(List),
    Panel(Panel),
    TexturedPanel(TexturedPanel),
}

impl WidgetType {
    fn draw(&self, commands: &mut Vec<DrawCommand>) {
        match self {
            WidgetType::List(p) => {
                for child in p.children() {
                    child.draw(commands);
                }
            }
            WidgetType::Panel(p) => p.draw(commands),
            WidgetType::TexturedPanel(p) => p.draw(commands),
        }
    }
}

pub enum DrawCommand {
    Panel { transform: WidgetTransform, color: u32 },
    TexturedPanel { transform: WidgetTransform, texture: Id },
}
