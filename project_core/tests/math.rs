use project_core::math::geometry::{corner::SquareCorner, direction::Direction, plane::Plane, vec3::Vec3};

#[test]
fn test_direction_enum() {
    assert_eq!(Direction::Left.to_usize(), 0);
    assert_eq!(Direction::Bottom.to_usize(), 1);
    assert_eq!(Direction::Back.to_usize(), 2);
    assert_eq!(Direction::Right.to_usize(), 3);
    assert_eq!(Direction::Top.to_usize(), 4);
    assert_eq!(Direction::Front.to_usize(), 5);

    assert!(Direction::Left.is_negative());
    assert!(Direction::Right.is_positive());
}

#[test]
fn test_square_corner() {
    assert_eq!(SquareCorner::TopLeft.to_usize(), 0);
    assert_eq!(SquareCorner::BottomRight.to_usize(), 3);
}

#[test]
fn test_plane_normalize() {
    let p = Plane {
        normal: Vec3::new(2.0, 0.0, 0.0),
        d: 4.0,
    };
    let normalized = p.normalize();
    assert!((normalized.normal.x() - 1.0).abs() < 1e-6);
    assert!((normalized.d - 2.0).abs() < 1e-6);
}

#[test]
fn test_plane_distance() {
    let p = Plane {
        normal: Vec3::new(0.0, 1.0, 0.0),
        d: 0.0,
    };
    let dist = p.distance(&Vec3::new(0.0, 5.0, 0.0));
    assert!((dist - 5.0).abs() < 1e-6);
}
