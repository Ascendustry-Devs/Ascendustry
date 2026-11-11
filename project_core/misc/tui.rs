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
