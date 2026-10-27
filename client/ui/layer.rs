use std::fmt::Debug;

use engine::render::ui::widgets::{DrawCommand, Widget, WidgetTransform};

use crate::ui::{context::UiContext, layouts::UiLayout};

pub trait UiLayer: Debug {
    fn layout(&mut self, available_space: &(u32, u32)) -> &Vec<WidgetTransform>;

    fn update(&mut self, available_space: &(u32, u32), ctx: &UiContext) -> &Vec<WidgetTransform>;

    fn draw(&mut self, layout: &Vec<WidgetTransform>, ctx: &UiContext) -> &Vec<DrawCommand>;
}
