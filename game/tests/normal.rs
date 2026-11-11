use game::constants::DIRECT_NORMALS_3D;

#[test]
fn test_direct_normals() {
    assert_eq!(DIRECT_NORMALS_3D.len(), 6);
    assert!(DIRECT_NORMALS_3D.contains(&(0, 1, 0)));
    assert!(DIRECT_NORMALS_3D.contains(&(0, 0, -1)));
}
