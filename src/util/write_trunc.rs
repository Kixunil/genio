use Write;

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
            self.writer.write(buf).map(|written| if self.remaining > 0 { written } else { len })
        } else {
            Ok(len)
        }
    }

    fn write_all(&mut self, mut buf: &[u8]) -> Result<(), Self::WriteError> {
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

    fn size_hint(&mut self, bytes: usize) {
        if bytes as u64 > self.remaining {
            self.writer.size_hint(self.remaining as usize)
        } else {
            self.writer.size_hint(bytes)
        }
    }

    fn uses_size_hint(&self) -> bool {
        self.writer.uses_size_hint()
    }
}
