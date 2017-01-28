use Write;
use util::placeholder::*;

/// Forbids all operations except writes.
///
/// One use case is to pass `Vec` as writer without fear that consumer would change previously
/// written content.
pub struct WriteOnly<W> (W);

impl<W: Write> WriteOnly<W> {
    pub fn new(writer: W) -> Self {
        WriteOnly(writer)
    }
}

impl<W: Write> From<W> for WriteOnly<W> {
    fn from(writer: W) -> Self {
        WriteOnly(writer)
    }
}

impl<W: Write> Write for WriteOnly<W> {
    type WriteError = W::WriteError;
    type FlushError = W::FlushError;

    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::WriteError> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> Result<(), Self::FlushError> {
        self.0.flush()
    }

    fn size_hint(&mut self, min: usize, max: Option<usize>) {
        self.0.size_hint(min, max);
    }
}
pub struct WriteOnlyPlaceholder<P> (P);

impl<P: Placeholder> Placeholder for WriteOnlyPlaceholder<P> {
    type End = WriteOnly<P::End>;
    type Hole = P::Hole;

    fn end(&mut self) -> Self::End {
        WriteOnly(self.0.end())
    }

    fn hole(&mut self) -> Self::Hole {
        self.0.hole()
    }
}

impl<W: InsertPlaceholder> InsertPlaceholder for WriteOnly<W> {
    type Placeholder = WriteOnlyPlaceholder<W::Placeholder>;

    fn insert_placeholder(&mut self, size: usize) -> Self::Placeholder {
        WriteOnlyPlaceholder(self.0.insert_placeholder(size))
    }
}

