//! Contains traits and impls for buffering.

use crate::error::BufError;
use crate::{Read, Write, OutBytes, BorrowOutSlice};
use core::convert::Infallible;
#[cfg(feature = "alloc")]
pub use crate::alloc_impls::BufReaderRequire;

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

impl<'a, R: BufRead> BufRead for &'a mut R {
    fn fill_buf(&mut self) -> Result<&[u8], Self::ReadError> {
        (*self).fill_buf()
    }

    fn consume(&mut self, amount: usize) {
        (*self).consume(amount)
    }
}

impl<'a> BufRead for &'a [u8] {
    fn fill_buf(&mut self) -> Result<&[u8], Self::ReadError> {
        Ok(*self)
    }

    fn consume(&mut self, mut amount: usize) {
        if amount > self.len() {
            amount = self.len();
        }

        let buf = *self;
        *self = &buf[amount..];
    }
}

/// When reading from reader, sometimes it's beneficial to read `n` bytes at once. However, BufRead
/// itself doesn't guarantee that more bytes will be available when calling `fill_buf` multiple
/// times. This trait provides `fill_progress` function with that guarantee.
pub trait BufReadProgress: BufRead {
    /// Error that occurs in buffer itself. Most often if buffer is out of memory.
    type BufReadError;

    /// Fills the internal buffer guaranteeing that successive calls to this function return more
    /// and more bytes (or an error).
    fn fill_progress(&mut self) -> Result<&[u8], BufError<Self::BufReadError, Self::ReadError>>;
}

/// When reading from reader, sometimes it's beneficial to read `n` bytes at once. However, BufRead
/// itself doesn't guarantee that more bytes will be available when calling `fill_buf` multiple
///  times. This trait provides `require_bytes` function that allows reading required amount of
///  bytes.
pub trait BufReadRequire: BufRead {
    /// Error that occurs in buffer itself. Most often if buffer is out of memory.
    type BufReadError;

    /// Fill the buffer until at least `amount` bytes are available.
    fn require_bytes(
        &mut self,
        amount: usize,
    ) -> Result<&[u8], BufError<Self::BufReadError, Self::ReadError>>;
}

impl<'a> BufReadRequire for &'a [u8] {
    type BufReadError = Infallible;

    fn require_bytes(
        &mut self,
        amount: usize,
    ) -> Result<&[u8], BufError<Self::BufReadError, Self::ReadError>> {
        if amount <= self.len() {
            Ok(*self)
        } else {
            Err(BufError::End)
        }
    }
}

/// When writing, it might be better to serialize directly into a buffer. This trait allows such
/// situation.
///
/// This trait is `unsafe` because it optimizes buffers to not require zeroing.
pub unsafe trait BufWrite: Write {
    /// Requests buffer for writing.
    ///
    /// The returned slice must always be non-empty. If non-emty slice can't be returned, `Err` must
    /// be returned instead. If the underlying writer is full, it has to flush the buffer.
    fn request_buffer(&mut self) -> Result<&mut OutBytes, Self::WriteError>;

    /// Tells the buf writer that `size` bytes were written into buffer.
    ///
    /// The `size` must NOT be bigger by mistake!
    unsafe fn submit_buffer(&mut self, size: usize);

    /// Writes single byte. Since this is buffered, the operation will be efficient.
    fn write_byte(&mut self, byte: u8) -> Result<(), Self::WriteError> {
        self.write(&[byte]).map(drop)
    }
}

/// This trait allows requiring buffer of specified size.
pub unsafe trait BufWriteRequire: BufWrite {
    /// Indicates error in buffer. Most often caused by size being too large but it might be also
    /// a write error, if buf writer tried to flush buffer.
    type BufWriteError;

    /// Require buffer with minimum `size` bytes. It is an error to return smaller buffer but
    /// `unsafe` code can't rely on it.
    fn require_buffer(&mut self, size: usize) -> Result<&mut OutBytes, Self::BufWriteError>;
}

/// Wrapper that provides buffering for a writer.
pub struct BufWriter<W: Write, Storage: BorrowOutSlice<u8>> {
    writer: W,
    buffer: crate::Buffer<Storage, crate::buffer::init::Uninit>,
    write_pos: usize,
}

impl<W: Write, B: BorrowOutSlice<u8>> BufWriter<W, B> {
    /// Creates buffered writer.
    ///
    /// Warning: buffer must be non-zero! Otherwise the program may panic!
    pub fn new(writer: W, storage: B) -> Self {
        assert!(!storage.borrow_uninit_slice().is_empty());

        BufWriter {
            writer,
            buffer: crate::Buffer::new(storage),
            write_pos: 0,
        }
    }

    fn flush_if_full(&mut self) -> Result<(), <Self as Write>::WriteError> {
        if self.buffer.as_out().is_full() {
            self.flush()
        } else {
            Ok(())
        }
    }
}

impl<W: Write, B: BorrowOutSlice<u8>> Write for BufWriter<W, B> {
    type WriteError = W::WriteError;
    type FlushError = W::WriteError;

    fn write(&mut self, data: &[u8]) -> Result<usize, Self::WriteError> {
        if self.buffer.as_out().is_full() {
            self.flush()?;
            if data.len() >= self.buffer.capacity() {
                return self.writer.write(&data);
            }
        }

        Ok(self.buffer.as_out().write_slice_min(data))
    }

    fn flush(&mut self) -> Result<(), Self::FlushError> {
        let mut to_flush = &self.buffer.written()[self.write_pos..];
        while !to_flush.is_empty() {
            self.write_pos += self.writer.write(to_flush)?;
            to_flush = &self.buffer.written()[self.write_pos..];
        }
        self.write_pos = 0;
        self.buffer.reset();
        Ok(())
    }

    fn size_hint(&mut self, min_bytes: usize, max_bytes: Option<usize>) {
        let min = min_bytes.saturating_add(self.buffer.written().len() - self.write_pos);
        let max = max_bytes.map(|max| max.saturating_add(self.buffer.written().len() - self.write_pos));
        self.writer.size_hint(min, max);
    }

    fn uses_size_hint(&self) -> bool {
        self.writer.uses_size_hint()
    }
}

unsafe impl<W: Write, B: BorrowOutSlice<u8>> BufWrite for BufWriter<W, B> {
    fn request_buffer(&mut self) -> Result<&mut OutBytes, Self::WriteError> {
        self.flush_if_full()?;

        // This simply returns pointer to the uninitialized buffer
        Ok(self.buffer.out_bytes())
    }

    unsafe fn submit_buffer(&mut self, size: usize) {
        self.buffer.as_out().advance_unchecked(size)
    }
}

unsafe impl<'a, W: BufWrite> BufWrite for &'a mut W {
    fn request_buffer(&mut self) -> Result<&mut OutBytes, Self::WriteError> {
        (*self).request_buffer()
    }

    unsafe fn submit_buffer(&mut self, size: usize) {
        (*self).submit_buffer(size)
    }
}

unsafe impl<'a> BufWrite for &'a mut [u8] {
    fn request_buffer(&mut self) -> Result<&mut OutBytes, Self::WriteError> {
        if self.is_empty() {
            Err(crate::error::BufferOverflow)
        } else {
            Ok(self.borrow_out_slice())
        }
    }

    unsafe fn submit_buffer(&mut self, size: usize) {
        let tmp = core::mem::replace(self, &mut []);
        *self = &mut tmp[size..];
    }
}
