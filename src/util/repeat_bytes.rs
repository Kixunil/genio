use void::Void;
use Read;

/// Reader repeating a sequence of bytes infinitely.
pub struct RepeatBytes<B> {
    bytes: B,
    offset: usize,
}

impl<B: AsRef<[u8]>> Read for RepeatBytes<B> {
    type ReadError = Void;

    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::ReadError> {
        use ::core::cmp::min;
        let len = buf.len();
        let bytes = self.bytes.as_ref();

        let amt = min(buf.len(), bytes.len() - self.offset);
        buf[..amt].copy_from_slice(&bytes[self.offset..(self.offset)]);
        self.offset += amt;
        if self.offset == bytes.len() {
            self.offset = 0
        }

        let buf = {
            let tmp = &mut buf[amt..];
            tmp
        };

        for chunk in buf.chunks_mut(bytes.len()) {
            if chunk.len() != bytes.len() {
                chunk.copy_from_slice(&bytes[..bytes.len()]);
                self.offset = chunk.len();
            } else {
                chunk.copy_from_slice(bytes);
            }
        }
        Ok(len)
    }
}
