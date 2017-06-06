//! This module contains glue `for std::io` and other `std` types.

use Read;
use Write;
use ExtendFromReader;
use ExtendFromReaderSlow;
use error::ExtendError;
use ReadOverwrite;
use void::Void;
use std::vec::Vec;
use std::io::{Sink, Empty};
use std::io;
use bufio::BufWrite;

impl Write for Vec<u8> {
    type WriteError = Void;
    type FlushError = Void;

    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::WriteError> {
        self.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> Result<(), Self::FlushError> {
        Ok(())
    }

    fn size_hint(&mut self, bytes: usize) {
        self.reserve(bytes)
    }

    fn uses_size_hint(&self) -> bool {
        true
    }
}

unsafe impl BufWrite for Vec<u8> {
    fn request_buffer(&mut self) -> Result<*mut [u8], Self::WriteError> {
        use ::std::slice;

        // Ensure there is a space for data
        self.reserve(1);
        unsafe {
            Ok(&mut slice::from_raw_parts_mut(self.as_mut_ptr(), self.capacity())[self.len()..])
        }
    }

    unsafe fn submit_buffer(&mut self, size: usize) {
        let new_len = self.len() + size;
        self.set_len(new_len)
    }
}

impl ExtendFromReaderSlow for Vec<u8> {
    // We could return OOM, but there is no `try_alloc`, so we have to panic.
    // That means `Vec` can never fail.
    type ExtendError = Void;

    fn extend_from_reader_slow<R: Read + ?Sized>(&mut self, reader: &mut R) -> Result<usize, ExtendError<R::ReadError, Self::ExtendError>> {
        let begin = self.len();
        self.resize(begin + 1024, 0);
        match reader.read(&mut self[begin..]) {
            Ok(bytes) => {
                // Check that returned value is correct.
                // This could be omitted, since `ReadOverwrite` is `unsafe` but this is
                // quite cheap check and avoids serious problems.
                assert!(bytes <= self.capacity() - begin);
                self.resize(begin + bytes, 0);
                Ok(bytes)
            },
            Err(e) => {
                // We have to reset len to previous value if error happens.
                self.resize(begin, 0);
                Err(ExtendError::ReadErr(e))
            },
        }
    }
}

// Efficient implementation
impl ExtendFromReader for Vec<u8> {
    fn extend_from_reader<R: Read + ReadOverwrite + ?Sized>(&mut self, reader: &mut R) -> Result<usize, ExtendError<R::ReadError, Self::ExtendError>> {
        // Prepare space
        self.reserve(1024);

        // This code is "unsafe because we use `.set_len()` to improve performance.
        // It also relies on `ReadOverwrite`.
        unsafe {
            // `std::Vec` doesn't guarantee that capacity will be greater after call to `reserve()`
            // so we don't rely on it.
            // "Vec does not guarantee any particular growth strategy when reallocating when full,
            // nor when reserve is called." - documentation
            if self.capacity() > self.len() {
                let begin = self.len();
                // This is correct in the sense it won't cause UB but the `Vec` will contain
                // uninitialized bytes. Those bytes will be overwritten by reader thanks to
                // `ReadOverwrite`
                let capacity = self.capacity();
                self.set_len(capacity);
                match reader.read(&mut self[begin..]) {
                    Ok(bytes) => {
                        // Check that returned value is correct.
                        // This could be omitted, since `ReadOverwrite` is `unsafe` but this is
                        // quite cheap check and avoids serious problems.
                        assert!(bytes <= capacity - begin);
                        self.set_len(begin + bytes);
                        Ok(bytes)
                    },
                    Err(e) => {
                        // We have to reset len to previous value if error happens.
                        self.set_len(begin);
                        Err(ExtendError::ReadErr(e))
                    },
                }
            } else {
                // Fallback for cases where `reserve` reserves nothing.
                self.extend_from_reader_slow(reader)
            }
        }
    }
}

// Same as our Sink.
impl Write for Sink {
    type WriteError = Void;
    type FlushError = Void;

    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::WriteError> {
        Ok(buf.len())
    }

    fn flush(&mut self) -> Result<(), Self::FlushError> {
        Ok(())
    }

    fn size_hint(&mut self, _bytes: usize) {
    }
}

// Same as our Empty.
impl Read for Empty {
    type ReadError = Void;

    fn read(&mut self, _buf: &mut [u8]) -> Result<usize, Self::ReadError> {
        Ok(0)
    }
}

/// Wrapper providing `std::io::Read` trait for `genio::Read` types.
pub struct StdRead<R> (R);

impl<R: Read> StdRead<R> {
    /// Wraps `genio` reader into `std` reader.
    pub fn new(reader: R) -> Self {
        StdRead(reader)
    }

    /// Unwraps inner reader.
    pub fn into_inner(self) -> R {
        self.0
    }
}

impl<E: Into<io::Error>, R: Read<ReadError=E>> io::Read for StdRead<R> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, io::Error> {
        self.0.read(buf).map_err(Into::into)
    }
}

/// Wrapper providing `std::io::Write` trait for `genio::Write` types.
pub struct StdWrite<W> (W);

impl<W: Write> StdWrite<W> {
    /// Wraps `genio` writer into `std` writer.
    pub fn new(writer: W) -> Self {
        StdWrite(writer)
    }

    /// Unwraps inner writer.
    pub fn into_inner(self) -> W {
        self.0
    }
}

impl<WE: Into<io::Error>, FE: Into<io::Error>, W: Write<WriteError=WE, FlushError=FE>> io::Write for StdWrite<W> {
    fn write(&mut self, buf: &[u8]) -> Result<usize, io::Error> {
        self.0.write(buf).map_err(Into::into)
    }

    fn flush(&mut self) -> Result<(), io::Error> {
        self.0.flush().map_err(Into::into)
    }
}

/// Wrapper providing `std::io::Read + std::io::Write` traits for `genio::Read + genio::Write` types.
pub struct StdIo<T> (T);

impl<RE: Into<io::Error>, WE: Into<io::Error>, FE: Into<io::Error>, T: Read<ReadError=RE> + Write<WriteError=WE, FlushError=FE>> StdIo<T> {
    /// Wraps `genio` reader+writer into `std` reader+writer.
    pub fn new(io: T) -> Self {
        StdIo(io)
    }

    /// Unwraps inner io.
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<E: Into<io::Error>, T: Read<ReadError=E>> io::Read for StdIo<T> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, io::Error> {
        self.0.read(buf).map_err(Into::into)
    }
}

impl<WE: Into<io::Error>, FE: Into<io::Error>, T: Write<WriteError=WE, FlushError=FE>> io::Write for StdIo<T> {
    fn write(&mut self, buf: &[u8]) -> Result<usize, io::Error> {
        self.0.write(buf).map_err(Into::into)
    }

    fn flush(&mut self) -> Result<(), io::Error> {
        self.0.flush().map_err(Into::into)
    }
}

/// Wrapper providing `genio::Read` trait for `std::io::Read` types.
pub struct GenioRead<R> (R);

impl<R: io::Read> GenioRead<R> {
    /// Wraps `std` readerinto `genio` reader.
    pub fn new(reader: R) -> Self {
        GenioRead(reader)
    }

    /// Unwraps `std` reader `genio` reader.
    pub fn into_inner(self) -> R {
        self.0
    }
}

impl<R: io::Read> Read for GenioRead<R> {
    type ReadError = io::Error;

    fn read(&mut self, buf: &mut [u8]) -> Result<usize, io::Error> {
        self.0.read(buf)
    }
}

/// Wrapper providing `genio::Write` trait for `std::io::Write` types.
pub struct GenioWrite<W> (W);

impl<W: io::Write> GenioWrite<W> {
    /// Wraps `std` writer into `genio` writer.
    pub fn new(writer: W) -> Self {
        GenioWrite(writer)
    }

    /// Unwraps `std` writer into `genio` writer.
    pub fn into_inner(self) -> W {
        self.0
    }
}

impl<W: io::Write> Write for GenioWrite<W> {
    type WriteError = io::Error;
    type FlushError = io::Error;

    fn write(&mut self, buf: &[u8]) -> Result<usize, io::Error> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> Result<(), io::Error> {
        self.0.flush()
    }

    fn size_hint(&mut self, _bytes: usize) {
    }
}

/// Wrapper providing `genio::Read + genio::Write` traits for `std::io::Read + std::io::Write` types.
pub struct GenioIo<T> (T);

impl<T: io::Read + io::Write> GenioIo<T> {
    /// Wraps `std` reader+writer into `genio` reader+writer.
    pub fn new(io: T) -> Self {
        GenioIo(io)
    }

    /// Unwraps `std` reader+writer into `genio` reader+writer.
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T: io::Read> Read for GenioIo<T> {
    type ReadError = io::Error;

    fn read(&mut self, buf: &mut [u8]) -> Result<usize, io::Error> {
        self.0.read(buf)
    }
}

impl<T: io::Write> Write for GenioIo<T> {
    type WriteError = io::Error;
    type FlushError = io::Error;

    fn write(&mut self, buf: &[u8]) -> Result<usize, io::Error> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> Result<(), io::Error> {
        self.0.flush()
    }

    fn size_hint(&mut self, _bytes: usize) {
    }
}
