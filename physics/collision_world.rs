pub trait CollisionWorld {
    fn is_empty(&self) -> bool;
    fn is_block_solid(&self, x: i32, y: i32, z: i32) -> bool;
}
