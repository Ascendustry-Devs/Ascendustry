use std::{any::Any, fmt::Debug};

pub mod hotbar;

pub trait UiLayout: Any + Debug {}
