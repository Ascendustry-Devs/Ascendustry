#[macro_export]
macro_rules! printtime {
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
macro_rules! time {
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
        if !$crate::audit::tui::log_to_tui(&msg) { println!("{}", msg); }
    }};
}

#[macro_export]
macro_rules! log_warn_server {
    ($($args:tt)*) => {{
        let msg = format!("[WSRV]$> {}", format_args!($($args)*));
        if !$crate::audit::tui::log_to_tui(&msg) { eprintln!("{}", msg); }
    }};
}

#[macro_export]
macro_rules! log_err_server {
    ($($args:tt)*) => {{
        let msg = format!("[ESRV]$> {}", format_args!($($args)*));
        if !$crate::audit::tui::log_to_tui(&msg) { eprintln!("{}", msg); }
    }};
}

#[macro_export]
macro_rules! log_client {
    ($($args:tt)*) => {{
        let msg = format!("[iCLI]$> {}", format_args!($($args)*));
        if !$crate::audit::tui::log_to_tui(&msg) { println!("{}", msg); }
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
        if !$crate::audit::tui::log_to_tui(&msg) { eprintln!("{}", msg); }
    }};
}
