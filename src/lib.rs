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

#[cfg(feature = "use_std")]
extern crate std;

extern crate void;

#[cfg(feature = "use_std")]
pub mod std_impls;

pub mod error;
pub mod ext;
pub mod util;
pub mod bufio;

use void::Void;
use error::{ReadExactError, ExtendError};
use util::Chain;

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
    /// It's highly recommended to use `Void` from `void` crate if `read()` can never fail.
    type ReadError;

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
    /// No guarantees are provided about the contents of buf when this function is called,
    /// implementations cannot rely on any property of the contents of buf being true. It is
    /// recommended that implementations only write data to buf instead of reading its contents.
    ///
    /// # Errors
    ///
    /// If this function encounters any form of I/O or other error, an error
    /// variant will be returned. If an error is returned then it must be
    /// guaranteed that no bytes were read.
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::ReadError>;

    /// Read the exact number of bytes required to fill `buf`.
    ///
    /// This function reads as many bytes as necessary to completely fill the specified buffer `buf`.
    ///
    /// No guarantees are provided about the contents of `buf` when this function is called,
    /// implementations cannot rely on any property of the contents of `buf` being true. It is
    /// recommended that implementations only write data to `buf` instead of reading its contents.
    fn read_exact(&mut self, mut buf: &mut [u8]) -> Result<(), ReadExactError<Self::ReadError>> {
        if self.available_bytes(buf.len()) {
            while !buf.is_empty() {
                let read_bytes = self.read(buf)?;
                if read_bytes == 0 {
                    return Err(ReadExactError::UnexpectedEnd);
                }

                let tmp = buf;
                buf = &mut tmp[read_bytes..];
            }
            return Ok(());
        } else {
            Err(ReadExactError::UnexpectedEnd)
        }
    }

    /// Hints whether there are at least `at_least` bytes available.
    ///
    /// This function should return true if it can't determine exact amount. That is also default.
    ///
    /// # Errors
    ///
    /// It is an error to return false even if there are more bytes available.
    fn available_bytes(&self, _at_least: usize) -> bool {
        true
    }

    /// Chains another reader after `self`. When self ends (returns Ok(0)), the other reader will
    /// provide bytes to read.
    fn chain<R: Read>(self, other: R) -> Chain<Self, R> where Self: Sized {
        Chain::new(self, other)
    }

    /// Reads all bytes into any type that can be extended by a reader. This is more generic than
    /// the case of `std::io` and might enable some optimizations.
    ///
    /// Of course, `std::Vec` impls `ExtendFromReader`.
    fn read_to_end<T: ExtendFromReader>(&mut self, container: &mut T) -> Result<usize, ExtendError<Self::ReadError, T::ExtendError>> where Self: ReadOverwrite {
        let mut total_bytes = 0;

        loop {
            let bytes = container.extend_from_reader(self)?;
            if bytes == 0 {
                return Ok(total_bytes)
            }
            total_bytes += bytes;
        }
    }

    /// Creates a "by reference" adaptor for this instance of `Read`.
    ///
    /// The returned adaptor also implements `Read` and will simply borrow this current reader.
    fn by_ref(&mut self) -> &mut Self where Self: Sized {
        self
    }
}

/// Some types can be extended by reading from reader. The most well-known is probably `Vec`. It
/// is possible to implement it manually, but it may be more efficient if the type implements this
/// trait directly. In case of `Vec`, it means reading directly into uninitialized part of reserved
/// memory in case of the fast version of this trait.
pub trait ExtendFromReaderSlow {
    /// This type is returned when extending fails. For example, if not enough memory could be
    /// allocated. All other errors should be passed directly from reader.
    type ExtendError;

    /// This method performs extending from reader - that means calling `read()` just once.
    fn extend_from_reader_slow<R: Read + ?Sized>(&mut self, reader: &mut R) -> Result<usize, ExtendError<R::ReadError, Self::ExtendError>>;
}

/// This trait is similar to slow one. The difference is that thanks to reader guaranteeing
/// correctness, this one can use uninitialized buffer.
pub trait ExtendFromReader: ExtendFromReaderSlow {
    /// This method performs extending from reader - that means calling `read()` just once.
    fn extend_from_reader<R: Read + ReadOverwrite + ?Sized>(&mut self, reader: &mut R) -> Result<usize, ExtendError<R::ReadError, Self::ExtendError>>;
}

/// This marker trait declares that the Read trait is implemented correctly,
/// that means:
///
/// 1. implementation of `read()` and `read_exact()` doesn't read from provided buffer.
/// 2. if `read()` returns `Ok(n)`, then each of first `n` bytes was overwritten.
/// 3. if `read_exact()` returns `Ok(())` then each byte of buffer was overwritten.
///
/// Breaking this should not cause huge problems since untrusted input should be checked anyway but
/// it might leak internal state of the application, containing secret data like private keys.
/// Think of the Hartbleed bug.
pub unsafe trait ReadOverwrite: Read {}

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
    fn write_all(&mut self, mut buf: &[u8]) -> Result<(), Self::WriteError> {
        while !buf.is_empty() {
            let len = self.write(buf)?;
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
    fn size_hint(&mut self, bytes: usize);

    /// Reports to the caller whether size hint is actually used. This can prevent costly
    /// computation of size hint that would be thrown away.
    fn uses_size_hint(&self) -> bool {
        false
    }
}

impl<'a, R: Read + ?Sized> Read for &'a mut R {
    type ReadError = R::ReadError;

    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::ReadError> {
        (*self).read(buf)
    }
}

impl<'a> Read for &'a [u8] {
    type ReadError = Void;

    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::ReadError> {
        use ::core::cmp::min;

        let amt = min(buf.len(), self.len());
        let (a, b) = self.split_at(amt);

        // First check if the amount of bytes we want to read is small:
        // `copy_from_slice` will generally expand to a call to `memcpy`, and
        // for a single byte the overhead is significant.
        if amt == 1 {
            buf[0] = a[0];
        } else {
            buf[..amt].copy_from_slice(a);
        }

        *self = b;
        Ok(amt)
    }

    fn available_bytes(&self, at_least: usize) -> bool {
        self.len() >= at_least
    }
}

impl<'a> Write for &'a mut [u8] {
    type WriteError = error::BufferOverflow;
    type FlushError = Void;

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
    
    fn size_hint(&mut self, _bytes: usize) {}
}
