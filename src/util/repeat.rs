use crate::{Read, OutBuf};

/// Reader that infinitely repeats a single byte.
///
/// It will never fail and never return 0.
pub struct Repeat {
    byte: u8,
}

impl Read for Repeat {
    type ReadError = core::convert::Infallible;
    type BufInit = buffer::init::Uninit;

    fn read(&mut self, mut buf: OutBuf<'_, Self::BufInit>) -> Result<usize, Self::ReadError> {
        let len = buf.remaining();
        while !buf.is_full() {
            buf.write_byte(self.byte);
        }
        Ok(len)
    }

    #[inline]
    fn retrieve_available_bytes(&self) -> Option<usize> {
        Some(usize::MAX)
    }
}
