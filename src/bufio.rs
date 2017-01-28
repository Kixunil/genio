//! Contains traits and impls for buffering.

use Read;
use Write;
use error::BufError;

/// A `BufRead` is a type of `Read`er which has an internal buffer, allowing it to perform extra ways
/// of reading.
pub trait BufRead: Read {
    /// Fills the internal buffer of this object, returning the buffer contents.
    /// This function is a lower-level call. It needs to be paired with the consume() method to
    /// function properly. 
    fn fill_buf(&mut self) -> Result<&[u8], Self::ReadError>;
    /// Tells this buffer that `amount` bytes have been consumed from the buffer, so they should no
    /// longer be returned in calls to `read`.
    fn consume(&mut self, amount: usize);
}

/// When reading from reader, sometimes it's beneficial to read `n` bytes at once. However, BufRead
/// itself doesn't guarantee that more bytes will be available when calling `fill_buf` multiple
/// times. This trait provides `fill_progress` and `require_bytes` functions with that guarantee.
pub trait BufReadProgress: Read {
    /// Error that occurs in buffer itself. Most often if buffer is out of memory.
    type BufReadError;

    /// Fills the internal buffer guaranteeing that successive calls to this function return more
    /// and more bytes (or an error).
    fn fill_progress(&mut self) -> Result<&[u8], BufError<Self::BufReadError, Self::ReadError>>;

    /// Fill the buffer until at least `amount` bytes are available.
    fn require_bytes(&mut self, amount: usize) -> Result<&[u8], BufError<Self::BufReadError, Self::ReadError>>;
}

/// When writing, it might be better to serialize directly into a buffer. This trait allows such
/// situation.
pub trait BufWrite: Write {
    /// Requests buffer for writing. `min` hints that at least `min` bytes will be written. `Some(max)`
    /// hints that at most `max` bytes will be written. However, consumer can't rely on buffer
    /// returning any amount and must check it by calling `len()` on returned buffer.
    fn request_buffer(&mut self, min: usize, max: Option<usize>) -> &mut [u8];

    /// Tells the buf writer that `size` bytes were written into buffer.
    fn submit_buffer(&mut self, size: usize);
}

/// This trait allows requiring buffer of specified size.
pub trait BufWriteRequire: BufWrite {
    /// Indicates error in buffer. Most often caused by size being too large but it might be also
    /// a write error, if buf writer tried to flush buffer.
    type BufWriteError;

    /// Require buffer with minimum `size` bytes. It is an error to return smaller buffer but
    /// `unsafe` code can't rely on it. 
    fn require_buffer(&mut self, size: usize) -> Result<&mut [u8], Self::BufWriteError>;
}
