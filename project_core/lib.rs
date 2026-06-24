pub mod buffer_pool;
pub mod geometry;
pub mod parallel;
pub mod utils;

#[cfg(test)]
mod tests;

use std::sync::OnceLock;

type LogTui = dyn Fn(&str) + Send + Sync;

static LOG_TUI: OnceLock<Box<LogTui>> = OnceLock::new();

pub fn set_log_tui(tui: Box<LogTui>) {
    LOG_TUI.set(tui).ok();
}

/// Retourne `true` si un tui était enregistré (mode TUI).
/// Si vrai, l'appelant doit sauter l'impression stdout/stderr.
pub fn log_to_tui(msg: &str) -> bool {
    LOG_TUI.get().is_some_and(|tui| {
        tui(msg);
        true
    })
}

#[macro_export]
macro_rules! time {
    ($label:expr, $block:block) => {{
        let start = std::time::Instant::now();
        let result = $block;
        let duration = start.elapsed();
        let millis = duration.as_millis();
        let micros = duration.as_micros();
        let nanos = duration.as_nanos();
        println!("{}: {}ms/{}µs/{}ns", $label, millis, micros, nanos);
        result
    }};
}

#[macro_export]
macro_rules! time_noprint {
    ($block:block) => {{
        let start = std::time::Instant::now();
        let result = $block;
        let duration = start.elapsed();
        (result, duration)
    }};
}

#[macro_export]
macro_rules! log {
    ($($args:tt)*) => {{
        let msg = format!("i> {}", format_args!($($args)*));
        if !$crate::log_to_tui(&msg) { println!("{}", msg); }
    }};
}

#[macro_export]
macro_rules! log_warn {
    ($($args:tt)*) => {{
        let msg = format!("W> {}", format_args!($($args)*));
        if !$crate::log_to_tui(&msg) { println!("{}", msg); }
    }};
}

#[macro_export]
macro_rules! log_err {
    ($($args:tt)*) => {{
        let msg = format!("E> {}", format_args!($($args)*));
        if !$crate::log_to_tui(&msg) { eprintln!("{}", msg); }
    }};
}

#[macro_export]
macro_rules! log_server {
    ($($args:tt)*) => {{
        let msg = format!("[iSRV]$> {}", format_args!($($args)*));
        if !$crate::log_to_tui(&msg) { println!("{}", msg); }
    }};
}

#[macro_export]
macro_rules! log_warn_server {
    ($($args:tt)*) => {{
        let msg = format!("[WSRV]$> {}", format_args!($($args)*));
        if !$crate::log_to_tui(&msg) { eprintln!("{}", msg); }
    }};
}

#[macro_export]
macro_rules! log_err_server {
    ($($args:tt)*) => {{
        let msg = format!("[ESRV]$> {}", format_args!($($args)*));
        if !$crate::log_to_tui(&msg) { eprintln!("{}", msg); }
    }};
}

#[macro_export]
macro_rules! log_client {
    ($($args:tt)*) => {{
        let msg = format!("[iCLI]$> {}", format_args!($($args)*));
        if !$crate::log_to_tui(&msg) { println!("{}", msg); }
    }};
}

#[macro_export]
macro_rules! log_warn_client {
    ($($args:tt)*) => {{
        let msg = format!("[WCLI]$> {}", format_args!($($args)*));
        if !$crate::log_to_tui(&msg) { println!("{}", msg); }
    }};
}

#[macro_export]
macro_rules! log_err_client {
    ($($args:tt)*) => {{
        let msg = format!("[ECLI]$> {}", format_args!($($args)*));
        if !$crate::log_to_tui(&msg) { eprintln!("{}", msg); }
    }};
}
