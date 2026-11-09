use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};
use std::thread::{Builder, JoinHandle};

use tokio::sync::mpsc::{self, error::TrySendError};

use crate::parallel::{Parallelizable, QueueFull, WorkItem, WorkResult, ID_COUNTER};

/// Capacité du canal de requêtes (en nombre d'items, pas en octets).
const REQUEST_CAPACITY: usize = 1024;
/// Capacité du canal de résultats.
const RESULT_CAPACITY: usize = 1024;
/// Taille de pile de chaque thread worker (en octets).
const WORKER_STACK_SIZE: usize = 8 * 1024 * 1024;

pub struct WorkerPool<P: Parallelizable> {
    // Option pour pouvoir le dropper proprement avant le join dans Drop.
    request_tx: Option<mpsc::Sender<WorkItem<P::Input, P::Output>>>,
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
        let (request_tx, request_rx) = mpsc::channel::<WorkItem<P::Input, P::Output>>(REQUEST_CAPACITY);
        let (result_tx, result_rx) = mpsc::channel::<WorkResult<P::Output>>(RESULT_CAPACITY);

        let request_rx = Arc::new(Mutex::new(request_rx));
        let pending_count = Arc::new(AtomicUsize::new(0));

        let mut workers = Vec::with_capacity(num_workers);

        for worker_id in 0..num_workers {
            let rx = Arc::clone(&request_rx);
            let tx = result_tx.clone();
            let ctx = context.clone();
            let pending = Arc::clone(&pending_count);

            let worker = Builder::new()
                .name(format!("WorkerPool-{worker_id}"))
                .stack_size(WORKER_STACK_SIZE)
                .spawn(move || {
                    loop {
                        let item = {
                            let mut rx = rx.lock().unwrap();
                            rx.blocking_recv()
                        };

                        let Some(item) = item else {
                            // Tous les senders sont droppés : fin normale.
                            break;
                        };

                        let output = P::process(item.input, &ctx);
                        pending.fetch_sub(1, Ordering::Relaxed);

                        let result = WorkResult { output, id: item.id };

                        // Envoi bloquant côté worker (c'est voulu : backpressure),
                        // mais on sort si le receiver a été droppé / fermé.
                        if tx.blocking_send(result).is_err() {
                            break;
                        }
                    }
                })
                .expect("Failed to spawn worker thread");

            workers.push(worker);
        }

        drop(result_tx);

        Self {
            request_tx: Some(request_tx),
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

        let tx = self.request_tx.as_ref().expect("WorkerPool déjà droppé");

        let id = ID_COUNTER.fetch_add(1, Ordering::Relaxed);
        let item = WorkItem {
            input,
            id,
            _phantom: None,
        };

        self.pending_count.fetch_add(1, Ordering::Relaxed);

        match tx.try_send(item) {
            Ok(()) => Ok(id),
            Err(TrySendError::Full(_)) | Err(TrySendError::Closed(_)) => {
                self.pending_count.fetch_sub(1, Ordering::Relaxed);
                Err(QueueFull)
            }
        }
    }

    pub fn try_recv(&mut self) -> Option<WorkResult<P::Output>> {
        match self.result_rx.try_recv() {
            Ok(result) => Some(result),
            Err(mpsc::error::TryRecvError::Empty) => None,
            Err(mpsc::error::TryRecvError::Disconnected) => None,
        }
    }

    pub fn is_queue_full(&self) -> bool {
        self.max_pending
            .is_some_and(|max| self.pending_count.load(Ordering::Relaxed) >= max)
    }

    pub const fn context(&self) -> &P::Context {
        &self.context
    }
}

impl<P: Parallelizable> Drop for WorkerPool<P> {
    fn drop(&mut self) {
        self.result_rx.close();
        self.request_tx.take();
        for worker in std::mem::take(&mut self.workers) {
            let _ = worker.join();
        }
    }
}
