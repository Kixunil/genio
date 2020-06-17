use void::Void;
use Read;

/// This reader is empty - always returns 0 from read method.
pub struct Empty;

impl Read for Empty {
    type ReadError = Void;

    fn read(&mut self, _buf: &mut [u8]) -> Result<usize, Self::ReadError> {
        Ok(0)
    }
}
