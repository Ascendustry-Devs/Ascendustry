use project_core::data_structure_types::unique_queue::UniqueQueue;

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
