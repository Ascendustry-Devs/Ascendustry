pub mod entry;
pub mod error;
pub(super) mod gap_manager;
pub mod gpu_allocator;
pub(super) mod write_operation;

pub(super) const LOG_ALLOCATOR: bool = false;

// const BYTES_PER_FRAME_CAP: usize = 1024 * 1024 * 8;
// const MAX_MILLIS_PER_FRAME_CAP: u128 = 8;
// const MAX_WRITE_OPERATIONS_PER_FRAME: usize = 5;
const ARENA_MIN_SIZE: usize = 1024 * 1024 * 4; // 4Mb
const BUFFER_BASE_SIZE: usize = 1024 * 1024 * 4; // 4Mb
const BUFFER_EXPAND_COEF: f32 = 1.25; // allocates 1.25x more than needed to prevent quick reallocations.

#[macro_export]
macro_rules! log_allocator {
    () => {
        if crate::gpu::allocator::LOG_ALLOCATOR {
            println!();
        }
    };

    ($($arg:tt)*) => {
        if crate::gpu::allocator::LOG_ALLOCATOR {
            println!($($arg)*);
        }
    };
}
