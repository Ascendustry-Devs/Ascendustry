use std::sync::{
    atomic::{AtomicUsize, Ordering},
    mpsc, Arc, Mutex,
};
use std::thread::{Builder, JoinHandle};

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

pub struct WorkerPool<P: Parallelizable> {
    request_tx: mpsc::Sender<WorkItem<P::Input, P::Output>>,
    result_rx: mpsc::Receiver<WorkResult<P::Output>>,
    workers: Vec<JoinHandle<()>>,
    context: P::Context,
    max_pending: Option<usize>,
    pending_count: Arc<AtomicUsize>,
}

impl<P: Parallelizable> WorkerPool<P> {
    pub fn new(num_workers: usize, context: P::Context) -> Self {
        Self::with_max_pending(num_workers, context, None)
    }

    pub fn with_max_pending(num_workers: usize, context: P::Context, max_pending: Option<usize>) -> Self {
        let (request_tx, request_rx) = mpsc::channel::<WorkItem<P::Input, P::Output>>();
        let (result_tx, result_rx) = mpsc::channel::<WorkResult<P::Output>>();
        let request_rx = Arc::new(Mutex::new(request_rx));
        let pending_count = Arc::new(AtomicUsize::new(0));

        let mut workers = Vec::new();

        for worker_id in 0..num_workers {
            let rx = request_rx.clone();
            let tx = result_tx.clone();
            let ctx = context.clone();
            let pending = pending_count.clone();

            let worker = Builder::new()
                .name(format!("WorkerPool-{}", worker_id))
                .stack_size(8 * 1024 * 1024)
                .spawn(move || loop {
                    let item = {
                        let rx = rx.lock().unwrap();
                        match rx.recv() {
                            Ok(req) => req,
                            Err(_) => break,
                        }
                    };
                    let output = P::process(item.input, &ctx);

                    pending.fetch_sub(1, Ordering::Relaxed);
                    let result = WorkResult { output, id: item.id };
                    let _ = tx.send(result);
                })
                .expect("Failed to spawn worker thread");

            workers.push(worker);
        }

        Self {
            request_tx,
            result_rx,
            workers,
            context,
            max_pending,
            pending_count,
        }
    }

    pub fn submit(&self, input: P::Input) -> Result<usize, QueueFull> {
        if self.is_queue_full() {
            return Err(QueueFull);
        }
        let id = ID_COUNTER.fetch_add(1, Ordering::Relaxed);
        self.pending_count.fetch_add(1, Ordering::Relaxed);
        let item = WorkItem {
            input,
            id,
            _phantom: None,
        };
        let _ = self.request_tx.send(item);
        Ok(id)
    }

    pub fn try_recv(&self) -> Option<WorkResult<P::Output>> {
        match self.result_rx.try_recv() {
            Ok(result) => Some(result),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => None,
        }
    }

    pub fn is_queue_full(&self) -> bool {
        if let Some(max) = self.max_pending {
            return self.pending_count.load(Ordering::Relaxed) >= max;
        }
        return false;
    }

    pub fn context(&self) -> &P::Context {
        &self.context
    }
}

impl<P: Parallelizable> Drop for WorkerPool<P> {
    fn drop(&mut self) {
        let (tx, _) = mpsc::channel::<WorkItem<P::Input, P::Output>>();
        let _ = std::mem::replace(&mut self.request_tx, tx);

        for worker in std::mem::take(&mut self.workers) {
            let _ = worker.join();
        }
    }
}
