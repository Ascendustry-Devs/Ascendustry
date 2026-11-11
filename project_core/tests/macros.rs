use project_core::{printtime, time};

#[test]
fn test_timing_macros() {
    let result = printtime!("test", { 2 + 2 });
    assert_eq!(result, 4);

    let (result, duration): (i32, std::time::Duration) = time!({ 3 * 7 });
    assert_eq!(result, 21);
    assert!(duration.as_nanos() > 0);
}
