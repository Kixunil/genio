use crate::{Read, OutBuf};

/// This reader is empty - always returns 0 from read method.
pub struct Empty;

impl Read for Empty {
    type ReadError = core::convert::Infallible;
    type BufInit = buffer::init::Uninit;

    fn read(&mut self, _buf: OutBuf<'_, Self::BufInit>) -> Result<usize, Self::ReadError> {
        Ok(0)
    }

    #[inline]
    fn available_bytes(&self, at_least: usize) -> bool {
        at_least == 0
    }

    #[inline]
    fn retrieve_available_bytes(&self) -> Option<usize> {
        Some(0)
    }
}
