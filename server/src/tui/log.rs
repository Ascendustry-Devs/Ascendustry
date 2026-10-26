use crate::tui::bridge::TuiState;
use std::sync::{Arc, Mutex};

pub fn init_logging(state: Arc<Mutex<TuiState>>) {
    project_core::audit::tui::set_log_tui(Box::new(move |msg: &str| {
        if let Ok(mut s) = state.lock() {
            s.logs.push(msg.to_string());
            if s.logs.len() > 500 {
                s.logs.remove(0);
            }
        }
    }));
}
