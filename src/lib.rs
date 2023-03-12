//! This crate provides more generic alternatives to `std::io::*` traits and types. `std::io`
//! suffers several issues because of over-use of `std::io::Error` type. One of them is allocation
//! when creating `Error` type, other is inability to cleanly define errors which make sense in
//! particular implementation, impossibility of use in `no_std` environments and more.
//!
//! To solve these problems, `genio::Read`, `genio::Write` and other traits are allowed to define
//! their own error types. Together with other utilities and `std` glue they provide a way to write
//! more clear, portable and re-usable code.

#![no_std]
#![deny(missing_docs)]

#[cfg(feature = "std")]
extern crate std;

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "byteorder")]
extern crate byteorder;

#[cfg(feature = "alloc")]
mod alloc_impls;
#[cfg(feature = "std")]
pub mod std_impls;

use core::mem::take;

pub mod bufio;
pub mod error;
pub mod ext;
pub mod util;

pub extern crate uninit_buffer as buffer;
/// Re-exported for convenience.
pub use buffer::{Buffer, OutBuf, OutBytes};

use buffer::possibly_uninit::slice::BorrowOutSlice;

use crate::{
    error::{ExtendError, ReadExactError},
    util::Chain,
};

#[cfg(feature = "byteorder")]
use byteorder::ByteOrder;

pub use buffer::{init, BufInit};

/// The Read trait allows for reading bytes from a source.
///
/// Implementors of the Read trait are sometimes called 'readers'.
///
/// Readers are defined by one required method, `read()` and a required type `ReadError`. Each call
/// to read will attempt to pull bytes from this source into a provided buffer. A number of other
/// methods are implemented in terms of read(), giving implementors a number of ways to read bytes
/// while only needing to implement a single method.
///
/// Readers are intended to be composable with one another. Many implementors throughout genio
/// take and provide types which implement the Read trait.
///
/// Please note that each call to read may involve a system call, and therefore, using something
/// that implements BufRead, such as BufReader, will be more efficient.
pub trait Read {
    /// Value of this type is returned when `read()` fails.
    ///
    /// It's highly recommended to use [`core::convert::Infallible`] if `read()` can never fail.
    type ReadError;

    /// Marker for buffers this reader accepts.
    ///
    /// This should be almost always [`buffer::init::Uninit`]. 
    /// Exceptions:
    ///
    /// * Bridges to old APIs such as `std::io::Read` need to use [`buffer::init::Init`]
    /// * [`TrackBuffer`] has to use [`buffer::init::Dynamic`] to track initializedness and unify
    ///   the types.
    type BufInit: buffer::BufInit;

    /// Pull some bytes from this source into the specified buffer, returning how many bytes were
    /// read.
    ///
    /// This function does not provide any guarantees about whether it blocks waiting for data, but
    /// if an object needs to block for a read but cannot it will typically signal this via an Err
    /// return value.
    ///
    /// If the return value of this method is Ok(n), then it must be guaranteed that 0 <= n <=
    /// buf.len(). A nonzero n value indicates that the buffer buf has been filled in with n bytes
    /// of data from this source. If n is 0, then it can indicate one of two scenarios:
    ///
    /// 1. This reader has reached its "end of file" and will likely no longer be able to produce
    ///    bytes. Note that this does not mean that the reader will always no longer be able to
    ///    produce bytes.
    ///
    /// 2. The buffer specified was 0 bytes in length.
    ///
    /// # Errors
    ///
    /// If this function encounters any form of I/O or other error, an error
    /// variant will be returned. If an error is returned then it must be
    /// guaranteed that no bytes were read.
    fn read(&mut self, buf: OutBuf<'_, Self::BufInit>) -> Result<usize, Self::ReadError>;

    /// Read the exact number of bytes required to fill `buf`.
    ///
    /// This function reads as many bytes as necessary to completely fill the specified buffer `buf`.
    fn read_exact(&mut self, mut buf: OutBuf<'_, Self::BufInit>) -> Result<(), ReadExactError<Self::ReadError>> {
        if self.available_bytes(buf.remaining()) {
            while !buf.is_full() {
                let read_bytes = self.read(buf.reborrow())?;
                if read_bytes == 0 {
                    return Err(ReadExactError::UnexpectedEnd);
                }
            }
            Ok(())
        } else {
            Err(ReadExactError::UnexpectedEnd)
        }
    }

    /// Hints whether there are at least `at_least` bytes available.
    ///
    /// This function should return true if it can't determine the exact amount.
    /// That is also the default.
    /// The method should be cheap to call. It's mainly intended for buffered readers.
    ///
    /// # Errors
    ///
    /// It is an error to return false even if there are more bytes available.
    #[inline]
    fn available_bytes(&self, _at_least: usize) -> bool {
        true
    }

    /// An expensive way to get the number of available bytes.
    ///
    /// The method returns `Some(num_available_bytes)` if it succeeded in finding out how many
    /// bytes are available, `None` otherwise. This computation can be as expensive as a `read`
    /// call. (If `read` involves syscall, this is allowed to perform syscall too.)
    ///
    /// This can be used in optimizations to allocate the whole required buffer upfront.
    /// If the number of bytes is larger than `usize::MAX` this should return `usize::MAX`.
    #[inline]
    fn retrieve_available_bytes(&self) -> Option<usize> {
        None
    }

    /// Chains another reader after `self`. When self ends (returns Ok(0)), the other reader will
    /// provide bytes to read.
    fn chain<R: Read>(self, other: R) -> Chain<Self, R>
    where
        Self: Sized,
    {
        Chain::new(self, other)
    }

    /// Reads all bytes into any type that can be extended by a reader. This is more generic than
    /// the case of `std::io` and might enable some optimizations.
    ///
    /// Of course, `std::Vec` impls `ExtendFromReader`.
    fn read_to_end<T: ExtendFromReader>(
        &mut self,
        container: &mut T,
    ) -> Result<usize, ExtendError<Self::ReadError, T::ExtendError>>
    where
        Self: Sized,
    {
        let mut total_bytes = 0;

        loop {
            let bytes = container.extend_from_reader(self)?;
            if bytes == 0 {
                return Ok(total_bytes);
            }
            total_bytes += bytes;
        }
    }

    /// Creates a reader that converts all its errors to some other type.
    ///
    /// This is useful mainly when you have multiple readers of different error type and you need to
    /// unify their error types e.g. to store them in a collection of trait objects. You can use a
    /// conversion function that converts the errors to a common type so that all the error types
    /// become the same.
    fn map_read_err<E, F: FnMut(Self::ReadError) -> E>(self, f: F) -> MapReadErr<Self, F> where Self: Sized {
        MapReadErr {
            reader: self,
            map_fn: f,
        }
    }

    /// Creates a "by reference" adaptor for this instance of `Read`.
    ///
    /// The returned adaptor also implements `Read` and will simply borrow this current reader.
    fn by_ref(&mut self) -> &mut Self
    where
        Self: Sized,
    {
        self
    }

    /// Reads an unsigned 16 bit integer from the underlying reader.
    #[cfg(feature = "byteorder")]
    fn read_u16<BO: ByteOrder>(&mut self) -> Result<u16, ReadExactError<Self::ReadError>> {
        let mut buf = buffer::new_maybe_init::<[MaybeUninit<u8>; 2], Self::BufInit>();
        self.read_exact(buf.as_out())?;
        Ok(BO::read_u16(buf.written()))
    }

    /// Reads an unsigned 32 bit integer from the underlying reader.
    #[cfg(feature = "byteorder")]
    fn read_u32<BO: ByteOrder>(&mut self) -> Result<u32, ReadExactError<Self::ReadError>> {
        let mut buf = buffer::new_maybe_init::<[MaybeUninit<u8>; 4], Self::BufInit>();
        self.read_exact(buf.as_out())?;
        Ok(BO::read_u32(buf.written()))
    }

    /// Reads an unsigned 64 bit integer from the underlying reader.
    #[cfg(feature = "byteorder")]
    fn read_u64<BO: ByteOrder>(&mut self) -> Result<u64, ReadExactError<Self::ReadError>> {
        let mut buf = buffer::new_maybe_init::<[MaybeUninit<u8>; 8], Self::BufInit>();
        self.read_exact(buf.as_out())?;
        Ok(BO::read_u64(buf.written()))
    }

    /// Reads an signed 16 bit integer from the underlying reader.
    #[cfg(feature = "byteorder")]
    fn read_i16<BO: ByteOrder>(&mut self) -> Result<i16, ReadExactError<Self::ReadError>> {
        let mut buf = buffer::new_maybe_init::<[MaybeUninit<u8>; 2], Self::BufInit>();
        self.read_exact(buf.as_out())?;
        Ok(BO::read_i16(buf.written()))
    }

    /// Reads an signed 32 bit integer from the underlying reader.
    #[cfg(feature = "byteorder")]
    fn read_i32<BO: ByteOrder>(&mut self) -> Result<i32, ReadExactError<Self::ReadError>> {
        let mut buf = buffer::new_maybe_init::<[MaybeUninit<u8>; 4], Self::BufInit>();
        self.read_exact(buf.as_out())?;
        Ok(BO::read_i32(buf.written()))
    }

    /// Reads an signed 64 bit integer from the underlying reader.
    #[cfg(feature = "byteorder")]
    fn read_i64<BO: ByteOrder>(&mut self) -> Result<i64, ReadExactError<Self::ReadError>> {
        let mut buf = buffer::new_maybe_init::<[MaybeUninit<u8>; 8], Self::BufInit>();
        self.read_exact(buf.as_out())?;
        Ok(BO::read_i64(buf.written()))
    }

    /// Reads a IEEE754 single-precision (4 bytes) floating point number from the underlying
    /// reader.
    #[cfg(feature = "byteorder")]
    fn read_f32<BO: ByteOrder>(&mut self) -> Result<f32, ReadExactError<Self::ReadError>> {
        let mut buf = buffer::new_maybe_init::<[MaybeUninit<u8>; 4], Self::BufInit>();
        self.read_exact(buf.as_out())?;
        Ok(BO::read_f32(buf.written()))
    }

    /// Reads a IEEE754 double-precision (8 bytes) floating point number from the underlying
    /// reader.
    #[cfg(feature = "byteorder")]
    fn read_f64<BO: ByteOrder>(&mut self) -> Result<f64, ReadExactError<Self::ReadError>> {
        let mut buf = buffer::new_maybe_init::<[MaybeUninit<u8>; 8], Self::BufInit>();
        self.read_exact(buf.as_out())?;
        Ok(BO::read_f64(buf.written()))
    }
}

/*
pub struct TrackBuffer<R: Read>(R);

impl<R: Read> Read for TrackBuffer<R> {
    type Error = R::Error;
    type BufInit = buffer::init::Dynamic;

    fn read(&mut self, buf: &mut Buffer<'_, Self::BufInit>) -> Result<usize, Self::ReadError> {
        self.0.read(buf.maybe_zeroed())
    }
}
*/

/// Some types can be extended by reading from reader. The most well-known is probably `Vec`. It
/// is possible to implement it manually, but it may be more efficient if the type implements this
/// trait directly. In case of `Vec`, it means reading directly into uninitialized part of reserved
/// memory.
pub trait ExtendFromReader {
    /// This type is returned when extending fails. For example, if not enough memory could be
    /// allocated. All other errors should be passed directly from reader.
    type ExtendError;

    /// This method performs extending from reader - that means calling `read()` just once.
    fn extend_from_reader<R: Read + ?Sized>(
        &mut self,
        reader: &mut R,
    ) -> Result<usize, ExtendError<R::ReadError, Self::ExtendError>>;

    /// Extends `self` with the contents of whole reader.
    ///
    /// This calls `extend_from_reader` until it returns 0.
    /// The method returns the number of bytes read.
    /// If the amount exceeds `usize::MAX` the return value is `usize::MAX`.
    fn extend_from_reader_to_end<R: Read + ?Sized>(
        &mut self,
        reader: &mut R,
    ) -> Result<usize, ExtendError<R::ReadError, Self::ExtendError>> {
        let mut total_bytes = 0;

        loop {
            let bytes = self.extend_from_reader(reader)?;
            if bytes == 0 {
                return Ok(total_bytes);
            }
            total_bytes = total_bytes.saturating_add(bytes);
        }
    }
}

/// A trait for objects which are byte-oriented sinks.
///
/// Implementors of the `Write` trait are sometimes called 'writers'.
///
/// Writers are defined by two required types `WriteError`, `FlushError` and methods, `write()`
/// and `flush()`:
///
/// The `write()` method will attempt to write some data into the object, returning how many
/// bytes were successfully written.
///
/// The `flush()` method is useful for adaptors and explicit buffers themselves for ensuring that
/// all buffered data has been pushed out to the 'true sink'.
///
/// Writers are intended to be composable with one another. Many implementors throughout
/// `genio` take and provide types which implement the `Write` trait.
pub trait Write {
    /// Value of this type is returned when `write()` fails.
    ///
    /// It's highly recommended to use `Void` from `void` crate if `read()` can never fail.
    type WriteError;

    /// Value of this type is returned when `flush()` fails.
    /// In case of low-level writers flush often does nothing and therefore doesn't return error,
    /// so this type might be Void.
    ///
    /// It's highly recommended to use `Void` from `void` crate if `read()` can never fail.
    type FlushError;

    /// Write a buffer into this object, returning how many bytes were written.
    ///
    /// This function will attempt to write the entire contents of `buf`, but the entire write may
    /// not succeed, or the write may also generate an error. A call to `write` represents *at most
    /// one* attempt to write to any wrapped object.
    ///
    /// If the return value is `Ok(n)` then it must be guaranteed that `0 <= n <= buf.len()`. A return
    /// value of `0` typically means that the underlying object is no longer able to accept bytes and
    /// will likely not be able to in the future as well, or that the buffer provided is empty.
    ///
    /// # Errors
    ///
    /// Each call to write may generate an `WriteError` indicating that the operation could not be
    /// completed. If an error is returned then no bytes in the buffer were written to this writer.
    ///
    /// It is **not** considered an error if the entire buffer could not be written to this writer.
    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::WriteError>;

    /// Flush this output stream, ensuring that all intermediately buffered contents reach their
    /// destination.
    ///
    /// # Errors
    ///
    /// It is considered an error if not all bytes could be written due to I/O errors or EOF being
    /// reached.
    fn flush(&mut self) -> Result<(), Self::FlushError>;

    /// Attempts to write an entire buffer into this `Write`.
    fn write_all(&mut self, mut buf: &[u8]) -> Result<(), error::WriteAllError<Self::WriteError>> {
        let total_len = buf.len();
        while !buf.is_empty() {
            let len = self
                .write(buf)
                .map_err(|error| error::WriteAllError {
                    bytes_written: total_len - buf.len(),
                    bytes_missing: buf.len(),
                    error,
                })?;
            buf = &buf[len..];
        }
        Ok(())
    }

    /// Hints the writer how much bytes will be written after call to this function.
    /// If the maximum amount of bytes to be written is known then it should be passed as argument.
    /// If the maximum amount is unknown, then minimum should be passed.
    ///
    /// Call to this function might enable some optimizations (e.g. pre-allocating buffer of
    /// appropriate size). The implementors must not rely on this call to provide correct values or
    /// on this function being called at all! (Especially they **must not** cause undefined
    /// behavior.) However, performance might be arbitrarilly degraded in case caller provides
    /// wrong value.
    ///
    /// The function is mandatory, as a lint to incentivize implementors to implement it, if
    /// applicable. Note that if you implement this function, you must also implement
    /// `uses_size_hint`.
    fn size_hint(&mut self, min_bytes: usize, max_bytes: Option<usize>);

    /// Reports to the caller whether size hint is actually used. This can prevent costly
    /// computation of size hint that would be thrown away.
    fn uses_size_hint(&self) -> bool {
        false
    }

    /// Writes an unsigned 16 bit integer to the underlying writer.
    #[cfg(feature = "byteorder")]
    fn write_u16<BO: ByteOrder>(&mut self, val: u16) -> Result<(), Self::WriteError> {
        let mut buf = [0; 2];
        BO::write_u16(&mut buf, val);
        self.write_all(&buf)
    }

    /// Writes an unsigned 32 bit integer to the underlying writer.
    #[cfg(feature = "byteorder")]
    fn write_u32<BO: ByteOrder>(&mut self, val: u32) -> Result<(), Self::WriteError> {
        let mut buf = [0; 4];
        BO::write_u32(&mut buf, val);
        self.write_all(&buf)
    }

    /// Writes an unsigned 64 bit integer to the underlying writer.
    #[cfg(feature = "byteorder")]
    fn write_u64<BO: ByteOrder>(&mut self, val: u64) -> Result<(), Self::WriteError> {
        let mut buf = [0; 8];
        BO::write_u64(&mut buf, val);
        self.write_all(&buf)
    }

    /// Writes an unsigned n-bytes integer to the underlying writer.
    ///
    /// If the given integer is not representable in the given number of bytes, this method panics.
    /// If nbytes > 8, this method panics.
    #[cfg(feature = "byteorder")]
    fn write_uint<BO: ByteOrder>(
        &mut self,
        val: u64,
        bytes: usize,
    ) -> Result<(), Self::WriteError> {
        let mut buf = [0; 8];
        BO::write_uint(&mut buf, val, bytes);
        self.write_all(&buf)
    }

    /// Writes a signed 16 bit integer to the underlying writer.
    #[cfg(feature = "byteorder")]
    fn write_i16<BO: ByteOrder>(&mut self, val: i16) -> Result<(), Self::WriteError> {
        let mut buf = [0; 2];
        BO::write_i16(&mut buf, val);
        self.write_all(&buf)
    }

    /// Writes a signed 32 bit integer to the underlying writer.
    #[cfg(feature = "byteorder")]
    fn write_i32<BO: ByteOrder>(&mut self, val: i32) -> Result<(), Self::WriteError> {
        let mut buf = [0; 4];
        BO::write_i32(&mut buf, val);
        self.write_all(&buf)
    }

    /// Writes a signed 64 bit integer to the underlying writer.
    #[cfg(feature = "byteorder")]
    fn write_i64<BO: ByteOrder>(&mut self, val: i64) -> Result<(), Self::WriteError> {
        let mut buf = [0; 8];
        BO::write_i64(&mut buf, val);
        self.write_all(&buf)
    }

    /// Writes a signed n-bytes integer to the underlying writer.
    ///
    /// If the given integer is not representable in the given number of bytes, this method panics.
    /// If nbytes > 8, this method panics.
    #[cfg(feature = "byteorder")]
    fn write_int<BO: ByteOrder>(&mut self, val: i64, bytes: usize) -> Result<(), Self::WriteError> {
        let mut buf = [0; 8];
        BO::write_int(&mut buf, val, bytes);
        self.write_all(&buf)
    }

    /// Writes a IEEE754 single-precision (4 bytes) floating point number to the underlying writer.
    #[cfg(feature = "byteorder")]
    fn write_f32<BO: ByteOrder>(&mut self, val: f32) -> Result<(), Self::WriteError> {
        let mut buf = [0; 4];
        BO::write_f32(&mut buf, val);
        self.write_all(&buf)
    }

    /// Writes a IEEE754 double-precision (8 bytes) floating point number to the underlying writer.
    #[cfg(feature = "byteorder")]
    fn write_f64<BO: ByteOrder>(&mut self, val: f64) -> Result<(), Self::WriteError> {
        let mut buf = [0; 4];
        BO::write_f64(&mut buf, val);
        self.write_all(&buf)
    }
}

impl<'a, R: Read + ?Sized> Read for &'a mut R {
    type ReadError = R::ReadError;
    type BufInit = R::BufInit;

    fn read(&mut self, buf: OutBuf<'_, Self::BufInit>) -> Result<usize, Self::ReadError> {
        (*self).read(buf)
    }
}

impl<'a, W: Write + ?Sized> Write for &'a mut W {
    type WriteError = W::WriteError;
    type FlushError = W::FlushError;

    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::WriteError> {
        (*self).write(buf)
    }

    fn flush(&mut self) -> Result<(), Self::FlushError> {
        (*self).flush()
    }

    fn size_hint(&mut self, min_bytes: usize, max_bytes: Option<usize>) {
        (*self).size_hint(min_bytes, max_bytes)
    }

    /// Reports to the caller whether size hint is actually used. This can prevent costly
    /// computation of size hint that would be thrown away.
    fn uses_size_hint(&self) -> bool {
        (**self).uses_size_hint()
    }
}

impl<'a> Read for &'a [u8] {
    type ReadError = core::convert::Infallible;
    type BufInit = init::Uninit;

    fn read(&mut self, mut buf: OutBuf<'_, Self::BufInit>) -> Result<usize, Self::ReadError> {
        if self.is_empty() {
            return Ok(0);
        }

        let amt = buf.write_slice_min(self);

        *self = &self[amt..];
        Ok(amt)
    }

    fn available_bytes(&self, at_least: usize) -> bool {
        self.len() >= at_least
    }
}

impl<'a> Read for &'a mut [u8] {
    type ReadError = Void;

    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::ReadError> {
        let mut immutable: &[u8] = self;
        let amt = immutable.read(buf)?;
        *self = &mut take(self)[amt..];
        Ok(amt)
    }
}

impl<'a> Write for &'a mut [u8] {
    type WriteError = error::BufferOverflow;
    type FlushError = core::convert::Infallible;

    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::WriteError> {
        if buf.len() <= self.len() {
            let (first, second) = ::core::mem::replace(self, &mut []).split_at_mut(buf.len());
            first.copy_from_slice(&buf[0..buf.len()]);
            *self = second;
            Ok(buf.len())
        } else {
            Err(error::BufferOverflow)
        }
    }

    fn flush(&mut self) -> Result<(), Self::FlushError> {
        Ok(())
    }

    fn size_hint(&mut self, _min_bytes: usize, _max_bytes: Option<usize>) {}
}

/// Reader that maps all its errors using the provided function.
///
/// This is returned from [`Read::map_read_err`] method. Check its documentation for more details.
pub struct MapReadErr<R, F> {
    reader: R,
    map_fn: F,
}

impl<R, F, E> Read for MapReadErr<R, F> where R: Read, F: FnMut(R::ReadError) -> E {
    type ReadError = E;
    type BufInit = R::BufInit;

    fn read(&mut self, buf: OutBuf<'_, Self::BufInit>) -> Result<usize, Self::ReadError> {
        self.reader.read(buf).map_err(&mut self.map_fn)
    }

    // TODO: forward other methods
}
