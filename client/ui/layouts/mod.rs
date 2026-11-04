use std::{any::Any, fmt::Debug};

pub mod hotbar;
pub mod inventory;
pub mod slot;

pub trait UiLayout: Any + Debug {}
