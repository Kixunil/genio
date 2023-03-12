use crate::error::{IntoIntrError, IntrError};
use crate::{Read, Write, OutBuf};

/// Restarts all interrupted operations.
///
/// All Read and Write operations that might fail with error indicating interruption (like EINTR)
/// will be restarted when wrapped in this type.
///
/// The error types indicate that interrupted error case has been handled.
pub struct Restarting<T>(T);

impl<T> Restarting<T> {
    /// Creates restarting IO.
    pub fn new(io: T) -> Self {
        Restarting(io)
    }

    /// Converts to lower-level reader.
    pub fn into_inner(self) -> T {
        self.0
    }

    fn restart_call<S, E: IntoIntrError, F: FnMut() -> Result<S, E>>(
        mut f: F,
    ) -> Result<S, E::NonIntr> {
        loop {
            match f().map_err(IntoIntrError::into_intr_error) {
                Ok(success) => return Ok(success),
                Err(IntrError::Other(e)) => return Err(e),
                Err(IntrError::Interrupted) => (),
            }
        }
    }
}

impl<R> Read for Restarting<R>
where
    R: Read,
    R::ReadError: IntoIntrError,
{
    type ReadError = <<R as Read>::ReadError as IntoIntrError>::NonIntr;
    type BufInit = R::BufInit;

    fn read(&mut self, mut buf: OutBuf<'_, Self::BufInit>) -> Result<usize, Self::ReadError> {
        Self::restart_call(|| self.0.read(buf.reborrow()))
    }
}

impl<W> Write for Restarting<W>
where
    W: Write,
    W::WriteError: IntoIntrError,
    W::FlushError: IntoIntrError,
{
    type WriteError = <<W as Write>::WriteError as IntoIntrError>::NonIntr;
    type FlushError = <<W as Write>::FlushError as IntoIntrError>::NonIntr;

    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::WriteError> {
        Self::restart_call(|| self.0.write(buf))
    }

    fn flush(&mut self) -> Result<(), Self::FlushError> {
        Self::restart_call(|| self.0.flush())
    }

    fn size_hint(&mut self, min_bytes: usize, max_bytes: Option<usize>) {
        self.0.size_hint(min_bytes, max_bytes);
    }

    fn uses_size_hint(&self) -> bool {
        self.0.uses_size_hint()
    }
}
