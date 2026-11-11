use game::{types::Position, world::raycast::voxel_raycast};
use project_core::math::geometry::vec3::Vec3;

#[test]
fn test_voxel_raycast_no_hit() {
    let origin = Position::new(0.0, 0.0, 0.0);
    let direction = Vec3::new(0.0, 1.0, 0.0);
    let result = voxel_raycast(&origin, &direction, 10.0, |_| false);
    assert!(result.is_none());
}

#[test]
fn test_voxel_raycast_hit() {
    let origin = Position::new(0.5, 0.5, 0.5);
    let direction = Vec3::new(1.0, 0.0, 0.0);
    let result = voxel_raycast(&origin, &direction, 10.0, |position| position.x() > 2);
    assert!(result.is_some());
    let hit = result.unwrap();
    assert!(hit.block_pos.x() >= 3);
}
