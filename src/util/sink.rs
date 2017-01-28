use Write;
use void::Void;

/// Silently drops everything that is written to it.
pub struct Sink;

impl Write for Sink {
    type WriteError = Void;
    type FlushError = Void;
    
    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::WriteError> {
        Ok(buf.len())
    }

    fn flush(&mut self) -> Result<(), Self::FlushError> {
        Ok(())
    }
}
