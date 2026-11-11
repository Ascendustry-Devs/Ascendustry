use crate::general_types::bit::Bit;

const COORD_BITS: u32 = 2;
const X_END: u32 = 6;
const Y_END: u32 = 4;
const Z_END: u32 = 2;

pub struct BinaryNormal3 {
    inner: u8,
}

impl BinaryNormal3 {
    pub fn x<T>(&self) -> T
    where
        T: From<i32>,
    {
        ((Bit::get_part_of(self.inner, X_END, COORD_BITS)) as i32 - 1).into()
    }

    pub fn y<T>(&self) -> T
    where
        T: From<i32>,
    {
        ((Bit::get_part_of(self.inner, Y_END, COORD_BITS)) as i32 - 1).into()
    }

    pub fn z<T>(&self) -> T
    where
        T: From<i32>,
    {
        ((Bit::get_part_of(self.inner, Z_END, COORD_BITS)) as i32 - 1).into()
    }
}

impl From<BinaryNormal3> for (i32, i32, i32) {
    fn from(value: BinaryNormal3) -> Self {
        (value.x::<i32>(), value.y::<i32>(), value.z::<i32>())
    }
}

impl From<(i32, i32, i32)> for BinaryNormal3 {
    fn from(value: (i32, i32, i32)) -> Self {
        let (x, y, z) = value;
        let mut inner = 0u8;
        inner = Bit::set_part_of(inner, X_END, COORD_BITS, (x + 1) as u8);
        inner = Bit::set_part_of(inner, Y_END, COORD_BITS, (y + 1) as u8);
        inner = Bit::set_part_of(inner, Z_END, COORD_BITS, (z + 1) as u8);

        Self { inner }
    }
}
