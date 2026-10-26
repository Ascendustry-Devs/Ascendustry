use std::mem::forget;

use crate::render::ui::geometry::ui_vertex::UiVertex;

pub struct UiCompiler;

impl UiCompiler {
    pub fn compile(vertices: Vec<UiVertex>) -> Vec<u8> {
        let len_bytes = vertices.len() * size_of::<UiVertex>();
        let cap_bytes = vertices.capacity() * size_of::<UiVertex>();
        let ptr = vertices.as_ptr();
        let ptr = ptr as *mut u8;

        forget(vertices);

        // Promis c'est safe Clément, promis
        unsafe { Vec::from_raw_parts(ptr, len_bytes, cap_bytes) }
    }
}
