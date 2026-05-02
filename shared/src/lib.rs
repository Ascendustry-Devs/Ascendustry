pub mod network;
pub mod parallel;
pub mod world;

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
        println!("> {}", format_args!($($args)*));
    };
}

#[macro_export]
macro_rules! log_err {
    ($($args:tt)*) => {
        eprintln!("> {}", format_args!($($args)*));
    };
}

#[macro_export]
macro_rules! log_server {
    ($($args:tt)*) => {
        println!("[INFSRV]$> {}", format_args!($($args)*));
    };
}

#[macro_export]
macro_rules! log_err_server {
    ($($args:tt)*) => {
        eprintln!("[ERRSRV]$> {}", format_args!($($args)*));
    };
}

#[macro_export]
macro_rules! log_client {
    ($($args:tt)*) => {
        println!("[INFCLI]$> {}", format_args!($($args)*));
    };
}

#[macro_export]
macro_rules! log_err_client {
    ($($args:tt)*) => {
        eprintln!("[ERRCLI]$> {}", format_args!($($args)*));
    };
}
