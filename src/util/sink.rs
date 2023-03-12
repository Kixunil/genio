use crate::Write;

/// Silently drops everything that is written to it.
pub struct Sink;

impl Write for Sink {
    type WriteError = core::convert::Infallible;
    type FlushError = core::convert::Infallible;

    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::WriteError> {
        Ok(buf.len())
    }

    fn flush(&mut self) -> Result<(), Self::FlushError> {
        Ok(())
    }

    fn size_hint(&mut self, _min_bytes: usize, _max_bytes: Option<usize>) {}
}
