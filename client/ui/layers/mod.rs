pub mod hotbar;

use std::{any::Any, fmt::Debug};

use engine::render::ui::widgets::DrawCommand;

use crate::ui::{context::UiContext, layouts::UiLayout};

pub trait UiLayer: Debug + Clone {
    type Layout: UiLayout;

    fn update(&mut self, _ctx: &UiContext) {}
    fn layout(&self, available_space: &(u32, u32)) -> Self::Layout;
    fn draw(&self, layout: &Self::Layout) -> Vec<DrawCommand>;
}

pub trait DynUiLayer: Debug {
    fn update(&mut self, ctx: &UiContext);
    fn layout(&self, available_space: &(u32, u32)) -> Box<dyn UiLayout>;
    fn draw(&self, layout: &dyn UiLayout) -> Vec<DrawCommand>;
}

impl<T: UiLayer> DynUiLayer for T {
    fn update(&mut self, ctx: &UiContext) {
        self.update(ctx);
    }

    fn layout(&self, available_space: &(u32, u32)) -> Box<dyn UiLayout> {
        Box::new(UiLayer::layout(self, available_space))
    }

    fn draw(&self, layout: &dyn UiLayout) -> Vec<DrawCommand> {
        let layout = (layout as &dyn Any)
            .downcast_ref::<T::Layout>()
            .expect("Layout type mismatch");

        UiLayer::draw(self, layout)
    }
}
