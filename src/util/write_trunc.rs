use crate::Write;
use crate::error::WriteAllError;

/// Truncates writing so that at most `n` bytes in total are written into the writer.
///
/// Excess bytes are silently dropped.
pub struct WriteTrunc<W> {
    writer: W,
    remaining: u64,
}

impl<W: Write> WriteTrunc<W> {
    /// Initializes truncate.
    pub fn new(writer: W, max_bytes: u64) -> Self {
        WriteTrunc {
            writer: writer,
            remaining: max_bytes,
        }
    }

    /// Returns how many bytes remain until they will be truncated.
    pub fn remaining_bytes(&self) -> u64 {
        self.remaining
    }
}

impl<W: Write> Write for WriteTrunc<W> {
    type WriteError = W::WriteError;
    type FlushError = W::FlushError;

    fn write(&mut self, mut buf: &[u8]) -> Result<usize, Self::WriteError> {
        let len = buf.len();
        if len as u64 > self.remaining {
            // `self.remaining as usize` can't overflow because `len as u64 > self.remaining`
            let tmp = &buf[..(self.remaining as usize)];
            buf = tmp;
        }

        self.remaining -= buf.len() as u64;

        if buf.len() > 0 {
            self.writer
                .write(buf)
                .map(|written| if self.remaining > 0 { written } else { len })
        } else {
            Ok(len)
        }
    }

    fn write_all(&mut self, mut buf: &[u8]) -> Result<(), WriteAllError<Self::WriteError>> {
        let len = buf.len();
        if len as u64 > self.remaining {
            let tmp = &buf[..(self.remaining as usize)];
            buf = tmp;
        }

        self.remaining -= buf.len() as u64;

        if buf.len() > 0 {
            self.writer.write_all(buf)
        } else {
            Ok(())
        }
    }

    fn flush(&mut self) -> Result<(), Self::FlushError> {
        self.writer.flush()
    }

    fn size_hint(&mut self, min_bytes: usize, max_bytes: Option<usize>) {
        let (min_bytes, max_bytes) = if min_bytes as u64 > self.remaining {
            (self.remaining as usize, Some(self.remaining as usize))
        } else if self.remaining < usize::MAX as u64 {
            let max_bytes = max_bytes
                .unwrap_or(self.remaining as usize)
                .min(self.remaining as usize);
            (min_bytes, Some(max_bytes))
        } else {
            (min_bytes, max_bytes)
        };

        self.writer.size_hint(min_bytes, max_bytes)
    }

    fn uses_size_hint(&self) -> bool {
        self.writer.uses_size_hint()
    }
}
