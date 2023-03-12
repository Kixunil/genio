use crate::{Read, Write, ExtendFromReader, ExtendError, OutBuf, OutBytes};
use crate::error::BufError;
use crate::bufio::{BufWrite, BufRead, BufReadRequire, BufReadProgress};
use core::convert::Infallible;
use core::mem::MaybeUninit;
use core::fmt;
use alloc::vec::Vec;
use uninit_buffer::possibly_uninit::slice::BorrowOutSlice;

const DEFAULT_BUF_SIZE: usize = 8 * 1024;

#[cfg(not(feature = "vec_try_reserve"))]
#[derive(Debug, Clone)]
pub struct AllocError(core::convert::Infallible);

#[cfg(feature = "vec_try_reserve")]
#[derive(Debug, Clone)]
pub struct AllocError(alloc::collections::TryReserveError);

impl fmt::Display for AllocError {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

#[cfg(feature = "std")]
impl std::error::Error for AllocError {
    #[inline]
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.0.source()
    }
}

/// Use try_reserve or reserve depending on enabled feature.
fn try_reserve(vec: &mut Vec<u8>, required: usize) -> Result<(), AllocError> {
    #[cfg(not(feature = "vec_try_reserve"))]
    vec.reserve(required);

    #[cfg(feature = "vec_try_reserve")]
    vec.try_reserve(required)?;
    Ok(())
}

fn try_reserve_exact(vec: &mut Vec<u8>, required: usize) -> Result<(), AllocError> {
    #[cfg(not(feature = "vec_try_reserve"))]
    vec.reserve_exact(required);

    #[cfg(feature = "vec_try_reserve")]
    vec.try_reserve_exact(required)?;
    Ok(())
}

#[cfg(feature = "alloc")]
impl ExtendFromReader for Vec<u8> {
    // We could return OOM, but there is no `try_alloc`, so we have to panic.
    // That means `Vec` can never fail.
    type ExtendError = AllocError;

    fn extend_from_reader<R: Read + ?Sized>(
        &mut self,
        reader: &mut R,
    ) -> Result<usize, ExtendError<R::ReadError, Self::ExtendError>> {
        if self.len() == self.capacity() {
            try_reserve(&mut *self, 1024).map_err(ExtendError::ExtendErr)?;
        }

        buffer::with_vec_as_out_buf(&mut *self, |out_buf| {
            reader.read(out_buf)
        }).map_err(ExtendError::ReadErr)
    }
}

impl Write for Vec<u8> {
    type WriteError = Infallible;
    type FlushError = Infallible;

    #[inline]
    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::WriteError> {
        self.extend_from_slice(buf);
        Ok(buf.len())
    }

    #[inline]
    fn flush(&mut self) -> Result<(), Self::FlushError> {
        Ok(())
    }

    #[inline]
    fn size_hint(&mut self, min_bytes: usize, max_bytes: Option<usize>) {
        match max_bytes {
            Some(max_bytes) => {
                // This is a refactored way of checking (len + min) * 2 >= len + max
                // The idea is that vec usually grows by factor of two to acheive amortized
                // constant complexity. So we allow the vec to grow to the size (len + min) * 2 -
                // len which conveniently equals to len + min*2 which also can be checked against
                // max_bytes avoiding some math operations.
                let max_len = min_bytes
                    .saturating_mul(2)
                    .saturating_add(self.len());
                if max_bytes <= max_len {
                    // We use actual maximum so we don't need spare capacity
                    let _ = try_reserve_exact(self, max_bytes);
                } else {
                    // The actual maximum is higher so allow the allocator giving us more if
                    // beneficial.
                    let _ = try_reserve(self, max_len);
                }
            },
            None => {
                let _ = try_reserve(self, min_bytes);
            },
        }
    }

    #[inline]
    fn uses_size_hint(&self) -> bool {
        true
    }
}

unsafe impl BufWrite for Vec<u8> {
    #[inline]
    fn request_buffer(&mut self) -> Result<&mut OutBytes, Self::WriteError> {
        use core::slice;

        // Ensure there is a space for data
        self.reserve(1);
        // SAFETY:
        // * len can never be so high `add` would overflow
        // * there is `capacity` bytes of valid memory at `ptr`
        // * capacity >= len
        // * casting `&mut [u8]` to `&mut [MaybeUninit<u8>]` is sound as long as the latter is not
        //   exposed to safe code. `OutBytes` prevents exposing it.
        unsafe {
            let len = self.len();
            let remaining = self.capacity() - len;
            let ptr = self.as_mut_ptr()
                .add(len) as *mut MaybeUninit<u8>;

            Ok(slice::from_raw_parts_mut(ptr, remaining).borrow_out_slice())
        }
    }

    #[inline]
    unsafe fn submit_buffer(&mut self, size: usize) {
        let new_len = self.len() + size;
        self.set_len(new_len)
    }
}

/// Wrapper that provides buffering for a reader.
#[cfg(feature = "alloc")]
pub struct BufReaderRequire<R> {
    reader: R,
    buffer: ::std::vec::Vec<u8>,
    start: usize,
}

#[cfg(feature = "alloc")]
impl<R: Read> BufReaderRequire<R> {
    /// Creates buffered reader.
    #[inline]
    pub fn new(reader: R) -> Self {
        BufReaderRequire {
            reader,
            buffer: Vec::with_capacity(DEFAULT_BUF_SIZE),
            start: 0,
        }
    }

    /// Unwraps inner reader.
    ///
    /// Any data in the internal buffer is lost.
    #[inline]
    pub fn into_inner(self) -> R {
        self.reader
    }

    /// Gets the number of bytes in the buffer.
    ///
    /// This is the amount of data that can be returned immediately, without reading from the
    /// wrapped reader.
    #[inline]
    fn available(&self) -> usize {
        self.len() - self.start
    }

    #[inline]
    fn len(&self) -> usize {
        self.buffer.len()
    }

    #[inline]
    fn capacity(&self) -> usize {
        self.buffer.capacity()
    }

    #[inline]
    fn erase_beginning(&mut self) {
        let start = self.start;
        // set to 0 first to avoid pointing out of bounds if drain panics
        self.start = 0;
        self.buffer.drain(..start);
    }
}

#[cfg(feature = "alloc")]
impl<R: Read> Read for BufReaderRequire<R> {
    type ReadError = R::ReadError;
    type BufInit = buffer::init::Uninit;

    fn read(&mut self, mut buf: OutBuf<'_, Self::BufInit>) -> Result<usize, Self::ReadError> {
        let n = {
            let data = self.fill_buf()?;
            buf.write_slice_min(data)
        };
        self.consume(n);
        Ok(n)
    }
}

impl<R: Read> BufRead for BufReaderRequire<R> {
    fn fill_buf(&mut self) -> Result<&[u8], Self::ReadError> {
        if self.available() == 0 {
            self.erase_beginning();
            let reader = &mut self.reader;
            buffer::with_vec_as_out_buf(&mut self.buffer, |out_buf| reader.read(out_buf))?;
        }
        Ok(&self.buffer[self.start..])
    }

    #[inline]
    fn consume(&mut self, amount: usize) {
        self.start = self.start.saturating_add(amount).max(self.len());
    }
}

impl<R: Read> BufReadProgress for BufReaderRequire<R> {
    type BufReadError = crate::alloc_impls::AllocError;

    fn fill_progress(&mut self) -> Result<&[u8], BufError<Self::BufReadError, Self::ReadError>> {
        let amount = self.available() + 1;
        self.require_bytes(amount)
    }
}

impl<R: Read> BufReadRequire for BufReaderRequire<R> {
    type BufReadError = crate::alloc_impls::AllocError;

    fn require_bytes(&mut self, amount: usize) -> Result<&[u8], BufError<Self::BufReadError, Self::ReadError>> {
        // if there's enough bytes available, return
        // if we don't have enough capacity reallocate inexact
        // if tail is too short or beginning is too far, memmove the beginning
        if amount <= self.available() {
            return Ok(&self.buffer[self.start..]);
        }
        if amount > self.buffer.capacity() {
            self.buffer.reserve(amount - self.len());
        }
        if amount > self.capacity() - self.len() || self.start > self.capacity() / 2 || self.available() == 0 {
            self.erase_beginning();
        }
        while self.available() < amount {
            let reader = &mut self.reader;
            let result = buffer::with_vec_as_out_buf(&mut self.buffer, |out_buf| {
                reader.read(out_buf)
            });
            match result {
                Ok(0) => return Err(BufError::End),
                Ok(_) => (),
                Err(error) => return Err(BufError::OtherErr(error)),
            }
        }
        Ok(&self.buffer[self.start..])
    }
}
