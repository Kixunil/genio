use crate::Read;
use core::mem::MaybeUninit;

/// Represents reader as iterator over bytes.
///
/// Note: this iterator is unbuffered, so it's advised to use `BufReader` to improve performance.
pub struct Bytes<R> {
    reader: R,
}

impl<R: Read> Bytes<R> {
    /// Creates the iterator.
    pub fn new(reader: R) -> Self {
        Bytes { reader: reader }
    }
}

impl<R: Read> Iterator for Bytes<R> {
    type Item = Result<u8, R::ReadError>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut buf = buffer::new_maybe_init::<[MaybeUninit<u8>; 1], R::BufInit>();
        let result = self.reader.read(buf.as_out());
        match (result, buf.written().first()) {
            (Err(e), _) => Some(Err(e)),
            (Ok(_), byte) => byte.copied().map(Ok),
        }
    }
}
