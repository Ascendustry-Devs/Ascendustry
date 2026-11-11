use project_core::general_types::{bit::Bit, updatable::Updatable};

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
fn test_bit_get() {
    let expected = 0b_100;

    let value = 0b_01_100_010;
    let part_size = 3;
    let i = 6;

    assert_eq!(expected, Bit::get_part_of(value, i, part_size))
}

#[test]
fn test_bit_set() {
    let expected = 0b_011_111_10;

    let value = 0b_011_000_10;
    let new_part = 0b_111;
    let part_size = 3;
    let i = 5;

    assert_eq!(expected, Bit::set_part_of(value, i, part_size, new_part))
}
