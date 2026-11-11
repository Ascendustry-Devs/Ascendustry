use project_core::parallel::{buffer_pool::BufferPool, worker_pool::WorkerPool, Parallelizable};

#[test]
fn test_buffer_pool() {
    let pool = BufferPool::<u8>::new(16);
    let buf = pool.get_buffer();
    assert!(buf.is_empty());
    pool.release_buffer(buf);
    let buf2 = pool.get_buffer();
    assert!(buf2.is_empty());
}

#[test]
fn test_worker_pool_basic() {
    struct Double;
    impl Parallelizable for Double {
        type Input = i32;
        type Output = i32;
        type Context = ();
        fn process(input: Self::Input, _ctx: &Self::Context) -> Self::Output {
            input * 2
        }
    }

    let mut pool = WorkerPool::<Double>::new(2, ());
    pool.submit(21).unwrap();

    std::thread::sleep(std::time::Duration::from_millis(100));
    let result = pool.try_recv();
    assert!(result.is_some());
    assert_eq!(result.unwrap().output, 42);
}
