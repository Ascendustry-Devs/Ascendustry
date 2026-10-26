use std::fmt::Display;

#[repr(u8)]
#[derive(Debug, PartialEq, Eq)]
pub enum AllocError {
    InvalidId = 0,
    NotEnoughSpace = 1,
}

impl Display for AllocError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match *self {
            Self::InvalidId => "InvalidId",
            Self::NotEnoughSpace => "NotEnoughSpace",
        })
    }
}
