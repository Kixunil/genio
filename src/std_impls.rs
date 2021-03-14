//! This module contains glue `for std::io` and other `std` types.

use crate::Read;
use crate::Write;
use crate::OutBuf;
use std::io;
use std::io::{Empty, Sink};
use core::convert::Infallible;
use buffer::Buffer;

// Same as our Sink.
impl Write for Sink {
    type WriteError = Infallible;
    type FlushError = Infallible;

    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::WriteError> {
        Ok(buf.len())
    }

    fn flush(&mut self) -> Result<(), Self::FlushError> {
        Ok(())
    }

    fn size_hint(&mut self, _min_bytes: usize, _max_bytes: Option<usize>) {}
}

// Same as our Empty.
impl Read for Empty {
    type ReadError = Infallible;
    type BufInit = buffer::init::Uninit;

    fn read(&mut self, _buf: OutBuf<'_, Self::BufInit>) -> Result<usize, Self::ReadError> {
        Ok(0)
    }
}

/// Wrapper providing `std::io::Read` trait for `genio::Read` types.
pub struct StdRead<R>(R);

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

impl<E: Into<io::Error>, R: Read<ReadError = E>> io::Read for StdRead<R> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, io::Error> {
        let mut buffer: Buffer<&mut [u8], R::BufInit> = Buffer::new_from_init(buf);
        self.0.read(buffer.as_out()).map_err(Into::into)
    }
}

/// Wrapper providing `std::io::Write` trait for `genio::Write` types.
pub struct StdWrite<W>(W);

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

impl<WE: Into<io::Error>, FE: Into<io::Error>, W: Write<WriteError = WE, FlushError = FE>> io::Write
    for StdWrite<W>
{
    fn write(&mut self, buf: &[u8]) -> Result<usize, io::Error> {
        self.0.write(buf).map_err(Into::into)
    }

    fn flush(&mut self) -> Result<(), io::Error> {
        self.0.flush().map_err(Into::into)
    }
}

/// Wrapper providing `std::io::Read + std::io::Write` traits for `genio::Read + genio::Write` types.
pub struct StdIo<T>(T);

impl<
        RE: Into<io::Error>,
        WE: Into<io::Error>,
        FE: Into<io::Error>,
        T: Read<ReadError = RE> + Write<WriteError = WE, FlushError = FE>,
    > StdIo<T>
{
    /// Wraps `genio` reader+writer into `std` reader+writer.
    pub fn new(io: T) -> Self {
        StdIo(io)
    }

    /// Unwraps inner io.
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<E: Into<io::Error>, T: Read<ReadError = E>> io::Read for StdIo<T> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, io::Error> {
        let mut buffer: Buffer<&mut [u8], T::BufInit> = Buffer::new_from_init(buf);
        self.0.read(buffer.as_out()).map_err(Into::into)
    }
}

impl<WE: Into<io::Error>, FE: Into<io::Error>, T: Write<WriteError = WE, FlushError = FE>> io::Write
    for StdIo<T>
{
    fn write(&mut self, buf: &[u8]) -> Result<usize, io::Error> {
        self.0.write(buf).map_err(Into::into)
    }

    fn flush(&mut self) -> Result<(), io::Error> {
        self.0.flush().map_err(Into::into)
    }
}

/// Wrapper providing `genio::Read` trait for `std::io::Read` types.
pub struct GenioRead<R>(R);

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
    type BufInit = buffer::init::Init;

    fn read(&mut self, mut buf: OutBuf<'_, Self::BufInit>) -> Result<usize, io::Error> {
        let amount = self.0.read(&mut buf.bytes_mut())?;
        buf.advance(amount);
        Ok(amount)
    }
}

/// Wrapper providing `genio::Write` trait for `std::io::Write` types.
pub struct GenioWrite<W>(W);

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

    fn size_hint(&mut self, _min_bytes: usize, _max_bytes: Option<usize>) {}
}

/// Wrapper providing `genio::Read + genio::Write` traits for `std::io::Read + std::io::Write` types.
pub struct GenioIo<T>(T);

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
    type BufInit = buffer::init::Init;

    fn read(&mut self, mut buf: OutBuf<'_, Self::BufInit>) -> Result<usize, io::Error> {
        let amount = self.0.read(&mut buf.bytes_mut())?;
        buf.advance(amount);
        Ok(amount)
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

    fn size_hint(&mut self, _min_bytes: usize, _max_bytes: Option<usize>) {}
}
