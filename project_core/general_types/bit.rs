use std::ops::{BitAnd, BitOr, Not, Shl, Shr, Sub};

// Trait scellé : seuls les entiers primitifs peuvent être utilisés.
mod sealed {
    pub trait Sealed {}
}

/// Entier manipulable bit à bit.
///
/// `BITS` évite `size_of::<T>() * 8` et les conversions sujettes à troncature.
pub trait BitField: sealed::Sealed + Copy {
    const BITS: u32;
    const ZERO: Self;
    const ONE: Self;
    const ALL_ONES: Self;
}

macro_rules! impl_bit_field {
    ($($t:ty),* $(,)?) => {$(
        impl sealed::Sealed for $t {}

        impl BitField for $t {
            const BITS: u32 = <$t>::BITS;
            const ZERO: Self = 0;
            const ONE: Self = 1;
            const ALL_ONES: Self = !0;
        }
    )*};
}

impl_bit_field!(u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize);

pub enum Bit {}

impl Bit {
    /// Masque des `part_size` bits de poids faible.
    ///
    /// `part_size == T::BITS` est traité à part car `1 << BITS` est un
    /// dépassement de décalage.
    #[inline(always)]
    fn low_mask<T>(part_size: u32) -> T
    where
        T: BitField + Shl<u32, Output = T> + Sub<Output = T>,
    {
        debug_assert!(part_size > 0 && part_size <= T::BITS);

        if part_size == T::BITS {
            T::ALL_ONES
        } else {
            (T::ONE << part_size) - T::ONE
        }
    }

    /// Récupère les `part_size` bits de `value` dans `[end - part_size, end)`.
    #[inline(always)]
    pub fn get_part_of<T>(value: T, end: u32, part_size: u32) -> T
    where
        T: BitField + Shr<u32, Output = T> + Shl<u32, Output = T> + BitAnd<Output = T> + Sub<Output = T>,
    {
        assert!(
            part_size > 0 && part_size <= T::BITS,
            "part_size ({part_size}) doit être dans 1..={}",
            T::BITS
        );
        assert!(
            end >= part_size && end <= T::BITS,
            "end ({end}) doit être dans part_size..={}",
            T::BITS
        );

        (value >> (end - part_size)) & Self::low_mask(part_size)
    }

    /// Remplace les `part_size` bits de `value` dans `[end - part_size, end)`
    /// par les `part_size` bits de poids faible de `new_part`.
    ///
    /// Les bits de `new_part` au-delà de `part_size` sont ignorés.
    #[inline(always)]
    pub fn set_part_of<T>(value: T, end: u32, part_size: u32, new_part: T) -> T
    where
        T: BitField + Shl<u32, Output = T> + BitAnd<Output = T> + BitOr<Output = T> + Not<Output = T> + Sub<Output = T>,
    {
        assert!(
            part_size > 0 && part_size <= T::BITS,
            "part_size ({part_size}) doit être dans 1..={}",
            T::BITS
        );
        assert!(
            end >= part_size && end <= T::BITS,
            "end ({end}) doit être dans part_size..={}",
            T::BITS
        );

        let low = Self::low_mask::<T>(part_size);
        let shift = end - part_size;
        let mask = low << shift;

        (value & !mask) | ((new_part & low) << shift)
    }
}
