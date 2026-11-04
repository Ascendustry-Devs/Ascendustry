use crate::ui::context::UiContext;

pub mod hotbar;
pub mod inventory;
pub mod slot;

pub trait UiView {}

pub trait UiAdapter {
    type Input;
    type Output: UiView;

    fn adapt(input: Self::Input, ctx: &UiContext) -> Self::Output;
}
