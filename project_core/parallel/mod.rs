use std::sync::atomic::AtomicUsize;

pub mod buffer_pool;
pub mod worker_pool;

#[derive(Debug)]
pub struct QueueFull;

static ID_COUNTER: AtomicUsize = AtomicUsize::new(0);

pub struct WorkItem<I, O> {
    pub input: I,
    pub id: usize,
    _phantom: Option<O>,
}

pub struct WorkResult<O> {
    pub output: O,
    pub id: usize,
}

pub trait Parallelizable: Send + 'static {
    type Input: Send + 'static;
    type Output: Send + 'static;
    type Context: Clone + Send + 'static;

    fn process(input: Self::Input, ctx: &Self::Context) -> Self::Output;
}
