use crate::{Read, OutBuf};

/// Reader repeating a sequence of bytes infinitely.
pub struct RepeatBytes<B> {
    bytes: B,
    offset: usize,
}

impl<B: AsRef<[u8]>> Read for RepeatBytes<B> {
    type ReadError = core::convert::Infallible;
    type BufInit = buffer::init::Uninit;

    fn read(&mut self, mut buf: OutBuf<'_, Self::BufInit>) -> Result<usize, Self::ReadError> {
        let len = buf.remaining();
        let bytes = self.bytes.as_ref();

        while !buf.is_full() {
            let copied = buf.write_slice_min(&bytes[self.offset..]);
            self.offset += copied;
            if self.offset == bytes.len() {
                self.offset = 0
            }
        }

        Ok(len)
    }

    #[inline]
    fn retrieve_available_bytes(&self) -> Option<usize> {
        Some(usize::MAX)
    }
}
