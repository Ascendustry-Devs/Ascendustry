#[derive(Debug, Default, Clone, PartialEq)]
pub struct Checksum {
    inner: [u8; 2],
}

impl From<[u8; 2]> for Checksum {
    fn from(checksum: [u8; 2]) -> Checksum {
        Checksum { inner: checksum }
    }
}
