//! Contains traits and impls for buffering.

use Read;
use Write;
use error::BufError;
use ::void::Void;

const DEFAULT_BUF_SIZE: usize = 8 * 1024;

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
    fn require_bytes(&mut self, amount: usize) -> Result<&[u8], BufError<Self::BufReadError, Self::ReadError>>;
}

impl<'a> BufReadRequire for &'a [u8] {
    type BufReadError = Void;

    fn require_bytes(&mut self, amount: usize) -> Result<&[u8], BufError<Self::BufReadError, Self::ReadError>> {
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
/// This triat is `unsafe` because it optimizes buffers to not require zeroing.
pub unsafe trait BufWrite: Write {
    /// Requests buffer for writing.
    /// The buffer is represented as a pointer because it may contain uninitialized memory - only
    /// writing is allowed. The pointer must not outlive Self!
    ///
    /// The returned slice must always be non-empty. If non-emty slice can't be returned, `Err` must
    /// be returned instead. If the underlying writer is full, it has to flush the buffer.
    fn request_buffer(&mut self) -> Result<*mut [u8], Self::WriteError>;

    /// Tells the buf writer that `size` bytes were written into buffer.
    ///
    /// The `size` must NOT be bigger by mistake!
    unsafe fn submit_buffer(&mut self, size: usize);

    /// Writes single byte. Since this is buffered, the operation will be efficient.
    fn write_byte(&mut self, byte: u8) -> Result<(), Self::WriteError> {
        unsafe {
            (*self.request_buffer()?)[0] = byte;
            self.submit_buffer(1);
        }
        Ok(())
    }
}

/// This trait allows requiring buffer of specified size.
pub unsafe trait BufWriteRequire: BufWrite {
    /// Indicates error in buffer. Most often caused by size being too large but it might be also
    /// a write error, if buf writer tried to flush buffer.
    type BufWriteError;

    /// Require buffer with minimum `size` bytes. It is an error to return smaller buffer but
    /// `unsafe` code can't rely on it. 
    fn require_buffer(&mut self, size: usize) -> Result<*mut [u8], Self::BufWriteError>;
}

/// Represents type that can serve as (possibly uninitialized) buffer
pub trait AsRawBuf {
    /// Returns a pointer to the buffer. It may point to uninitialized data. The pointer must not
    /// outlive the buffer.
    fn as_raw_buf(&mut self) -> *mut [u8];

    /// Returns the length of the buffer.
    fn len(&self) -> usize;
}

impl<T: AsRef<[u8]> + AsMut<[u8]>> AsRawBuf for T {
    fn as_raw_buf(&mut self) -> *mut [u8] {
        self.as_mut()
    }

    fn len(&self) -> usize {
        self.as_ref().len()
    }
}

/// Wrapper that provides buffering for a reader.
#[cfg(feature = "std")]
pub struct BufReaderRequire<R> {
    reader: R,
    buffer: ::std::vec::Vec<u8>,
    start: usize,
    end: usize,
}

#[cfg(feature = "std")]
impl<R: Read> BufReaderRequire<R> {
    /// Creates buffered reader.
    pub fn new(reader: R) -> Self {
        let mut buffer = ::std::vec::Vec::new();
        buffer.resize(DEFAULT_BUF_SIZE, 0);
        BufReaderRequire {
            reader,
            buffer,
            start: 0,
            end: 0,
        }
    }

    /// Unwraps inner reader.
    ///
    /// Any data in the internal buffer is lost.
    pub fn into_inner(self) -> R {
        self.reader
    }

    /// Gets the number of bytes in the buffer.
    ///
    /// This is the amount of data that can be returned immediately, without reading from the
    /// wrapped reader.
    fn available(&self) -> usize {
        self.end - self.start
    }
}

#[cfg(feature = "std")]
impl<R: Read> Read for BufReaderRequire<R> {
    type ReadError = R::ReadError;

    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::ReadError> {
        let n = {
            let data = self.fill_buf()?;
            let n = data.len().max(buf.len());
            buf[..n].copy_from_slice(&data[..n]);
            n
        };
        self.consume(n);
        Ok(n)
    }
}

#[cfg(feature = "std")]
impl<R: Read> BufRead for BufReaderRequire<R> {
    fn fill_buf(&mut self) -> Result<&[u8], Self::ReadError> {
        if self.available() == 0 {
            self.start = 0;
            self.end = self.reader.read(&mut self.buffer[..])?;
        }
        Ok(&self.buffer[self.start..self.end])
    }

    fn consume(&mut self, amount: usize) {
        self.start = self.start.saturating_add(amount).max(self.buffer.len());
    }
}

#[cfg(feature = "std")]
impl<R: Read> BufReadProgress for BufReaderRequire<R> {
    type BufReadError = Void;

    fn fill_progress(&mut self) -> Result<&[u8], BufError<Self::BufReadError, Self::ReadError>> {
        let amount = self.available() + 1;
        self.require_bytes(amount)
    }
}

#[cfg(feature = "std")]
impl<R: Read> BufReadRequire for BufReaderRequire<R> {
    type BufReadError = Void;

    fn require_bytes(&mut self, amount: usize) -> Result<&[u8], BufError<Self::BufReadError, Self::ReadError>> {
        if self.available() >= amount {
            return Ok(&self.buffer[self.start..self.end]);
        }
        if amount > self.buffer.len() {
            let len = self.buffer.len();
            self.buffer.reserve(amount - len);
            let new_capacity = self.buffer.capacity();
            self.buffer.resize(new_capacity, 0);
        }
        if amount > self.buffer.len() - self.start {
            self.buffer.drain(..self.start);
            self.end -= self.start;
            self.start = 0;
            let capacity = self.buffer.capacity();
            self.buffer.resize(capacity, 0);
        }
        while self.available() < amount {
            match self.reader.read(&mut self.buffer[self.end..]) {
                Ok(0) => return Err(BufError::End),
                Ok(read_len) => self.end += read_len,
                Err(error) => return Err(BufError::OtherErr(error)),
            }
        }
        Ok(&self.buffer[self.start..self.end])
    }
}

/// Wrapper that provides buffering for a writer.
pub struct BufWriter<W, B> {
    writer: W,
    buffer: B,
    cursor: usize,
}

impl<W: Write, B: AsRawBuf> BufWriter<W, B> {
    /// Creates buffered writer.
    ///
    /// Warning: buffer must be non-zero! Otherwise the program may panic!
    pub fn new(writer: W, buffer: B) -> Self {
        BufWriter {
            writer,
            buffer,
            cursor: 0,
        }
    }

    fn flush_if_full(&mut self) -> Result<(), <Self as Write>::WriteError> {
        if self.cursor == self.buffer.len() {
            self.flush()
        } else {
            Ok(())
        }
    }
}

impl<W: Write, B: AsRawBuf> Write for BufWriter<W, B> {
    type WriteError = W::WriteError;
    type FlushError = W::WriteError;

    fn write(&mut self, data: &[u8]) -> Result<usize, Self::WriteError> {
        let buf_len = self.buffer.len();
        if self.cursor == buf_len {
            self.flush()?;
            if data.len() >= buf_len {
                return self.writer.write(&data);
            }
        }

        // This is correct because it only writes to the buffer
        unsafe {
            // Get the ref to uninitialized buffer
            let buf = &mut (*self.buffer.as_raw_buf())[self.cursor..];

            // Calculate how much bytes to copy (doesn't read uninitialized).
            let to_copy = ::core::cmp::min(buf_len, data.len());

            // Copy data. Overwrites uninitialized.
            buf[0..to_copy].copy_from_slice(&data[0..to_copy]);

            // Updates cursor by exactly the amount of bytes overwritten.
            self.cursor += to_copy;

            Ok(to_copy)
        }
    }

    fn flush(&mut self) -> Result<(), Self::FlushError> {
        // This is correct because it gets slice to initialized data
        let buf = unsafe {
            &mut (*self.buffer.as_raw_buf())[0..self.cursor]
        };

        self.cursor = 0;

        self.writer.write_all(buf)
    }

    fn size_hint(&mut self, bytes: usize) {
        self.writer.size_hint(bytes)
    }

    fn uses_size_hint(&self) -> bool {
        self.writer.uses_size_hint()
    }
}

unsafe impl<W: Write, B: AsRawBuf> BufWrite for BufWriter<W, B> {
    fn request_buffer(&mut self) -> Result<*mut [u8], Self::WriteError> {
        self.flush_if_full()?;

        // This simply returns pointer to the uninitialized buffer
        unsafe {
            let slice = &mut (*self.buffer.as_raw_buf())[self.cursor..];
            assert!(slice.len() > 0);
            Ok(slice)
        }
    }

    unsafe fn submit_buffer(&mut self, size: usize) {
        self.cursor += size;
    }
}

unsafe impl<'a, W: BufWrite> BufWrite for &'a mut W {
    fn request_buffer(&mut self) -> Result<*mut [u8], Self::WriteError> {
        (*self).request_buffer()
    }

    unsafe fn submit_buffer(&mut self, size: usize) {
        (*self).submit_buffer(size)
    }
}

unsafe impl<'a> BufWrite for &'a mut [u8] {
    fn request_buffer(&mut self) -> Result<*mut [u8], Self::WriteError> {
        if self.len() > 0 {
            Ok(*self)
        } else {
            Err(::error::BufferOverflow)
        }
    }

    unsafe fn submit_buffer(&mut self, size: usize) {
        let tmp = ::core::mem::replace(self, &mut []);
        *self = &mut tmp[size..];
    }
}
