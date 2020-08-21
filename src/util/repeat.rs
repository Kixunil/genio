use crate::Read;
use void::Void;

/// Reader that infinitely repeats single byte.
///
/// It will never fail and never return 0.
pub struct Repeat {
    byte: u8,
}

impl Read for Repeat {
    type ReadError = Void;

    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::ReadError> {
        // TODO: use memset?
        let len = buf.len();
        for b in buf {
            *b = self.byte;
        }
        Ok(len)
    }
}
