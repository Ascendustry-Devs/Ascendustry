#[cfg(test)]
mod tests {
    use crate::buffer_pool::BufferPool;
    use crate::geometry::corner::SquareCorner;
    use crate::geometry::direction::Direction;
    use crate::geometry::plane::Plane;
    use crate::parallel::Parallelizable;
    use crate::parallel::WorkerPool;
    use crate::utils::unique_queue::UniqueQueue;
    use crate::utils::updatable::Updatable;

    #[test]
    fn test_direction_enum() {
        assert_eq!(Direction::Left as u8, 0);
        assert_eq!(Direction::Bottom as u8, 1);
        assert_eq!(Direction::Back as u8, 2);
        assert_eq!(Direction::Right as u8, 3);
        assert_eq!(Direction::Top as u8, 4);
        assert_eq!(Direction::Front as u8, 5);

        assert!(Direction::Left.is_negative());
        assert!(Direction::Right.is_positive());
        assert_eq!(Direction::Top.to_usize(), 4);
    }

    #[test]
    fn test_square_corner() {
        assert_eq!(SquareCorner::TopLeft.to_usize(), 0);
        assert_eq!(SquareCorner::BottomRight.to_usize(), 3);
    }

    #[test]
    fn test_plane_normalize() {
        use cgmath::Vector3;

        let p = Plane {
            normal: Vector3::new(2.0, 0.0, 0.0),
            d: 4.0,
        };
        let normalized = p.normalize();
        assert!((normalized.normal.x - 1.0).abs() < 1e-6);
        assert!((normalized.d - 2.0).abs() < 1e-6);
    }

    #[test]
    fn test_plane_distance() {
        use cgmath::Vector3;

        let p = Plane {
            normal: Vector3::new(0.0, 1.0, 0.0),
            d: 0.0,
        };
        let dist = p.distance(Vector3::new(0.0, 5.0, 0.0));
        assert!((dist - 5.0).abs() < 1e-6);
    }

    #[test]
    fn test_updatable() {
        let mut u = Updatable::new(42);
        assert!(!u.has_changed());
        assert_eq!(*u.current(), 42);

        u.update(100);
        assert!(u.has_changed());
        assert_eq!(*u.current(), 100);
        assert_eq!(*u.last(), 42);

        let change = u.change();
        assert!(change.is_some());
        assert_eq!(*change.unwrap(), 100);
    }

    #[test]
    fn test_updatable_copy() {
        let mut u = Updatable::new([1.0, 2.0, 3.0]);
        u.update_by_copy([4.0, 5.0, 6.0]);
        assert!(u.has_changed());
    }

    #[test]
    fn test_unique_queue() {
        let mut q: UniqueQueue<i32> = UniqueQueue::new();
        assert!(q.is_empty());

        assert!(q.push_back(1));
        assert!(q.push_back(2));
        assert!(!q.push_back(1));
        assert_eq!(q.len(), 2);

        assert_eq!(q.pop_front(), Some(1));
        assert_eq!(q.pop_front(), Some(2));
        assert_eq!(q.pop_front(), None);
    }

    #[test]
    fn test_unique_queue_retain() {
        let mut q: UniqueQueue<i32> = UniqueQueue::new();
        q.push_back(1);
        q.push_back(2);
        q.push_back(3);
        q.retain(|x| *x != 2);
        assert_eq!(q.len(), 2);
        assert!(q.contains(&1));
        assert!(q.contains(&3));
    }

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

        let pool = WorkerPool::<Double>::new(2, ());
        pool.submit(21).unwrap();

        std::thread::sleep(std::time::Duration::from_millis(100));
        let result = pool.try_recv();
        assert!(result.is_some());
        assert_eq!(result.unwrap().output, 42);
    }

    #[test]
    fn test_timing_macros() {
        let result = crate::time!("test", { 2 + 2 });
        assert_eq!(result, 4);

        let (result, duration): (i32, std::time::Duration) = crate::time_noprint!({ 3 * 7 });
        assert_eq!(result, 21);
        assert!(duration.as_nanos() > 0);
    }
}
