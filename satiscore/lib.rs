pub mod buffer_pool;
pub mod constants;
pub mod geometry;
pub mod parallel;
pub mod utils;
pub mod world;

#[cfg(test)]
mod tests;

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
    ($($args:tt)*) => {
        println!("i> {}", format_args!($($args)*));
    };
}

#[macro_export]
macro_rules! log_warn {
    ($($args:tt)*) => {
        println!("W> {}", format_args!($($args)*));
    };
}

#[macro_export]
macro_rules! log_err {
    ($($args:tt)*) => {
        eprintln!("E> {}", format_args!($($args)*));
    };
}

#[macro_export]
macro_rules! log_server {
    ($($args:tt)*) => {
        println!("[iSRV]$> {}", format_args!($($args)*));
    };
}

#[macro_export]
macro_rules! log_warn_server {
    ($($args:tt)*) => {
        eprintln!("[WSRV]$> {}", format_args!($($args)*));
    };
}

#[macro_export]
macro_rules! log_err_server {
    ($($args:tt)*) => {
        eprintln!("[ESRV]$> {}", format_args!($($args)*));
    };
}

#[macro_export]
macro_rules! log_client {
    ($($args:tt)*) => {
        println!("[iCLI]$> {}", format_args!($($args)*));
    };
}

#[macro_export]
macro_rules! log_warn_client {
    ($($args:tt)*) => {
        println!("[WCLI]$> {}", format_args!($($args)*));
    };
}

#[macro_export]
macro_rules! log_err_client {
    ($($args:tt)*) => {
        eprintln!("[ECLI]$> {}", format_args!($($args)*));
    };
}
