//! This module contains various extension traits.

use Read;
use error::ReadExactError;

/// Result of successful read operation.
pub enum ReadResult<'a> {
    /// Some bytes were read.
    /// The slice will always contain at least one byte.
    Bytes(&'a mut [u8]),
    /// No bytes available (End reached).
    End,
}

impl<'a> ReadResult<'a> {
    /// Helps mapping `ReadResult` to `ReadExactError`
    ///
    /// # Example
    ///
    /// Assuming a function returns `ReadExactError<T>`
    /// ```no_compile
    /// let bytes = reader.read_ext(&mut buf)?.require_bytes()?;
    pub fn require_bytes<T>(self) -> Result<&'a mut [u8], ReadExactError<T>> {
        match self {
            ReadResult::Bytes(b) => Ok(b),
            ReadResult::End => Err(ReadExactError::UnexpectedEnd),
        }
    }
}

/// After reading to buffer, it's often useful to adjust the slice. Also, reading may be done in
/// loop, which ends when reader has no more data.
///
/// To make handling of these cases easier, one can use `read_ext` method which returns
/// `ReadResult` instead of usize.
///
/// Thanks to it you may write:
///
/// ```no_compile
/// let mut buf = [0; 1024];
/// while let Bytes(bytes) = reader.read_ext(&mut buf)? {
///     // Process  bytes here.
/// }
/// ```
/// instead of this:
/// ```no_compile
/// let mut buf = [0; 1024];
/// loop {
///     let len = reader.read(&mut buf)?;
///     if len == 0 {
///         break;
///     }
///
///     let bytes = &mut buf[..len];
///     // Process bytes here.
/// }
pub trait ReadExt: Read {
    /// Reads from the reader and converts the result.
    fn read_ext<'a, 'b>(&'a mut self, buf: &'b mut [u8]) -> Result<ReadResult<'b>, Self::ReadError>;
}

impl<R: Read + ?Sized> ReadExt for R {
    fn read_ext<'a, 'b>(&'a mut self, buf: &'b mut [u8]) -> Result<ReadResult<'b>, Self::ReadError> {
        let len = self.read(buf)?;
        if len > 0 {
            Ok(ReadResult::Bytes(&mut buf[..len]))
        } else {
            Ok(ReadResult::End)
        }
    }
}

