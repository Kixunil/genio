//! Primitives for working with maybe uninitialized byte buffers safely.
//!
//! This crate contains basic safe encapsulations of `unsafe` byte buffer-related APIs.
//! It was motivated by the `genio` crate but may be reused by other crates.
//!
//! The crate contains two important types: [`Buffer`] and [`OutBuf`].
//!
//! `Buffer` represents and underlying, maybe uninitialized, storage with additional information
//! used for tracking initializedness. Various underlying storages can be used: arrays, slices,
//! boxed slices. Both initialized and uninitialized.
//!
//! `OutBuf` is a custom mutable (actually write-only) reference to `Buffer` that can be handed out
//! to sources of bytes to be filled. Such is the case of sound, performant readers. `OutBuf`
//! erases irrelevant details like the type of backing storage by design, so it can be used in
//! trait objects. It still has a type parameter signalling whether the buffer is actually
//! initialized but this should not be a huge obstacle. The initializedness can be parametrised
//! and, if needed, dynamic tracking of initializedness can be used. This is mainly useful in case
//! of mixed byte sources.
//!
//! As mentioned, initializedness is tracked in a type parameter. The parameter is restricted to
//! implement a sealed trait, so there can truly be only three types:
//!
//! * [`init::Init`] - guarantees the buffer is actually initialized.
//! * [`init::Uninit`] - no guarantee about initializedness of the buffer.
//! * [`init::Dynamic`] - initializedness is tracked dynamically as a separate `usize` value.
//!
//! This can look a bit confusing however there's a simple way to choose the parameter:
//!
//! 0. Try using `init::Uninit`.
//! 1. If the above fails because some code needs `&mut [u8]` for writing (e.g. `std::io::Read`
//!    trait) use `init::Init`
//! 2. If the code that requires `&mut [u8]` is not guaranteed to be executed use `init::Dynamic`.
//!
//! In general, `init::Uninit` is the most performant but occasionally can not be used due to
//! old/imperfect APIs.

#![no_std]
#![deny(missing_docs)]

/// Re-export of the `possibly_uninit` crate which provides us important primitives for working
/// with uninitialized memory.
pub extern crate possibly_uninit;

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "alloc")]
use alloc::vec::Vec;
#[cfg(feature = "alloc")]
use alloc::boxed::Box;

mod buf_init;

pub use buf_init::*;
use core::mem::MaybeUninit;
use possibly_uninit::slice::BorrowOutSlice;
#[cfg(feature = "alloc")]
use core::borrow::BorrowMut;

pub use out_buf::OutBuf;
pub use buffer::Buffer;

/// Abbreviation for OutSlice<u8>
pub type OutBytes = possibly_uninit::slice::OutSlice<u8>;

/// Stores the position and the number of initialized bytes (if dynamic)
struct Meta<Init> {
    position: usize,
    init: Init,
}

/// Implements the core of `OutBuf` type, a poor man implementation of `unsafe` fields.
///
/// This is in a separate module to ensure all accesses that could violate invariants use `unsafe`
/// block. It also abstracts away the core operations making it easy to use custom DST behind a
/// feature flag.
mod out_buf {
    use crate::{BufInit, Meta, OutBytes};

    /// Type-erased out reference for [`Buffer`](crate::Buffer)
    ///
    /// This reference can be used to write into the buffer safely and passed to sources of data.
    /// It is used instead of `&mut Buffer` reference to allow casting it to non-dynamic types
    /// without unsoundness.
    ///
    /// It is also possible to use this to abstract over multiple buffers with different backing
    /// storages.
    pub struct OutBuf<'a, Init: BufInit> {
        bytes: &'a mut OutBytes,
        meta: &'a mut Meta<Init>,
    }

    impl<'a, Init: BufInit> OutBuf<'a, Init> {
        /// Creates `OutBuf` assuming correct inputs.
        ///
        /// ## Safety
        ///
        /// The requirements for calling this being sound are:
        ///
        /// * `meta.position <= bytes.len()`
        /// * if `meta.init.init_len().unwrap_or(bytes.len()) > meta.position`
        ///   then all bytes in `[meta.position..meta.init.init_len().unwrap_or(bytes.len())]`
        ///   are initialized.
        #[inline]
        pub(crate) unsafe fn new(bytes: &'a mut OutBytes, meta: &'a mut Meta<Init>) -> Self {
            debug_assert!(meta.position <= bytes.len());

            OutBuf {
                bytes,
                meta,
            }
        }

        /// Shortens the lifetime of `OutBuf`.
        ///
        /// This does the same thing `&mut *x` would do on native reference.
        #[inline]
        pub fn reborrow(&mut self) -> OutBuf<'_, Init> {
            OutBuf {
                bytes: &mut self.bytes,
                meta: &mut self.meta,
            }
        }

        #[inline]
        pub(crate) fn out_bytes_ref(&self) -> &OutBytes {
            self.bytes
        }

        #[inline]
        pub(crate) fn meta(&self) -> &Meta<Init> {
            &self.meta
        }

        #[inline]
        pub(crate) unsafe fn into_raw_parts(self) -> (&'a mut OutBytes, &'a mut Meta<Init>) {
            (self.bytes, self.meta)
        }

        #[inline]
        pub(crate) unsafe fn meta_mut(&mut self) -> &mut Meta<Init> {
            self.reborrow().into_raw_parts().1
        }
    }
}

mod buffer {
    use crate::{BufInit, Meta, init, OutBuf, OutBytes};
    use possibly_uninit::slice::BorrowOutSlice;

    /// Buffer of bytes that may be uninitialized.
    pub struct Buffer<Bytes: BorrowOutSlice<u8>, Init: BufInit> {
        bytes: Bytes,
        meta: Meta<Init>,
    }

    impl<Bytes: BorrowOutSlice<u8>, Init: BufInit> Buffer<Bytes, Init> {
        pub(crate) unsafe fn new_unchecked(bytes: Bytes, meta: Meta<Init>) -> Self {
            let slice_len = bytes.borrow_uninit_slice().len();
            debug_assert!(meta.position <= slice_len);
            debug_assert!(meta.init.init_len().unwrap_or(slice_len) <= slice_len);

            Buffer {
                bytes,
                meta,
            }
        }

        /// Returns the maximum number of bytes the buffer can store.
        pub fn capacity(&self) -> usize {
            self.bytes.borrow_uninit_slice().len()
        }

        pub(crate) fn position(&self) -> usize {
            self.meta.position
        }

        /// Returns abstracted write-only reference to the buffer.
        ///
        /// This is the primary interface for writing into the buffer that is intended to be passed
        /// to functions that fill the buffer with bytes. It is designed to allow casting it to
        /// uninit version soundly.
        ///
        /// It also prevents the consumers from touching written portion of the buffer.
        #[inline]
        pub fn as_out(&mut self) -> OutBuf<'_, Init> {
            // SAFETY: our type enforces the safety requirements of this operation
            unsafe {
                OutBuf::new(self.bytes.borrow_out_slice(), &mut self.meta)
            }
        }

        /// Returns the slice of uninitialized bytes.
        pub fn out_bytes(&mut self) -> &mut OutBytes {
            &mut self.bytes.borrow_out_slice()[self.meta.position..]
        }

        /// Returns the filled part of the buffer.
        #[inline]
        pub fn written(&self) -> &[u8] {
            // SAFETY: the invariant of this type is that all bytes up to position are initialized
            unsafe {
                let written_uninit = &self.bytes.borrow_uninit_slice()[..self.meta.position];
                core::slice::from_raw_parts(written_uninit.as_ptr() as *const u8, written_uninit.len())
            }
        }

        /// Returns the filled part of the buffer as mutable slice.
        #[inline]
        pub fn written_mut(&mut self) -> &mut [u8] {
            // SAFETY: the invariant of this type is that all bytes up to position are initialized
            unsafe {
                self.bytes.borrow_out_slice()[..self.meta.position].assume_init_mut()
            }
        }

        /// Decomposes the buffer into inner storage and position.
        #[inline]
        pub fn into_parts(self) -> (Bytes, usize) {
            (self.bytes, self.meta.position)
        }

        /// Sets the position to the beginning (0).
        #[inline]
        pub fn reset(&mut self) {
            unsafe {
                self.meta.init.update_min(self.meta.position);
                self.meta.position = 0;
            }
        }

        /// Rolls back the position by `count` bytes.
        #[inline]
        pub fn rewind(&mut self, count: usize) {
            if count > self.meta.position {
                panic!("attempt to update past beginning");
            }

            unsafe {
                self.meta.init.update_min(self.meta.position);
                self.meta.position -= count;
            }
        }

        /// Zeroes the buffer **if required** and converts it into initialized buffer.
        pub fn into_init(mut self) -> Buffer<Bytes, init::Init> {
            unsafe {
                self.as_out().perform_zeroing();
                let meta = Meta {
                    position: self.position(),
                    init: init::Init::new(),
                };

                // SAFETY: we've zeroed the buffer above as needed
                Buffer::new_unchecked(self.bytes, meta)
            }
        }

        /// Creates a buffer that tracks its initializedness.
        pub fn into_dynamic(self) -> Buffer<Bytes, init::Dynamic> {
            use crate::sealed::FromUninit;
            unsafe {
                let init_len = self.meta.init.init_len().unwrap_or(self.bytes.borrow_uninit_slice().len());
                let meta = Meta {
                    position: self.position(),
                    init: init::Dynamic::new(init_len),
                };

                Buffer::new_unchecked(self.bytes, meta)
            }
        }
    }

    impl<'a, Bytes: BorrowOutSlice<u8> + ?Sized, Init: BufInit> Buffer<&'a mut Bytes, Init> {
        /// Converts the initialized part of the buffer into primitive slice.
        ///
        /// This preserves the lifetime of the underlying reference so it can be used e.g. when
        /// returning from a function.
        pub fn into_init_slice(self) -> &'a mut [u8] {
            unsafe {
                // SAFETY: we track that the buffer is initialized up to `position`
                self.bytes.borrow_out_slice()[..self.meta.position].assume_init_mut()
            }
        }
    }
}

impl<'a, Init: BufInit> OutBuf<'a, Init> {
    /// Returns whole `bytes`, not just subslice.
    #[inline]
    pub(crate) fn out_bytes_whole(&mut self) -> &mut OutBytes {
        unsafe {
            self.reborrow().into_raw_parts().0
        }
    }

    /// Returns the remaining buffer that can be written into.
    ///
    /// Readers need to store the bytes into this slice.
    /// This method should be only used in low-level Read implementations.
    /// If you already have a slice or a byte see `write_slice` and `write_byte` methods.
    #[inline]
    pub fn out_bytes(&mut self) -> &mut OutBytes {
        let position = self.meta().position;
        &mut self.out_bytes_whole()[position..]
    }


    /// Marks `amount` of bytes having been written to the buffer.
    ///
    /// ## Safety
    ///
    /// The requirements for calling this being sound are:
    ///
    /// * `amount <= self.remaining()`
    /// * `amount` of consecutive bytes were written to `self.out-bytes` starting from 0th byte.
    #[inline]
    pub unsafe fn advance_unchecked(&mut self, amount: usize) {
        debug_assert!(amount <= self.remaining());
        self.meta_mut().position += amount;
    }

    /// Returns the number of bytes available for writing.
    // CR note: this is implemented here to make access not require `mut`
    #[inline]
    pub fn remaining(&self) -> usize {
        self.out_bytes_ref().len() - self.meta().position
    }

    /// Zeros **the uninitialized part** of the buffer
    pub(crate) fn perform_zeroing(&mut self) {
        if let Some(initialized_len) = self.meta().init.init_len() {
            let initialized_len = initialized_len
                .max(self.meta().position)
                // This is important: because of with_limit() init_len() <= self.bytes.len()
                // is NOT guaranteed even though it is for `Buffer`.
                .min(self.out_bytes_whole().len());
            let truly_uninit = &mut self.out_bytes_whole()[initialized_len..];
            // Also, because initialized_len could've been past this slice lenght we should not
            // overwrite it.
            if !truly_uninit.is_empty() {
                truly_uninit.write_zeroes();
                // SAFETY: we've just zeroed all uninit bytes
                unsafe {
                    let whole_buf_len = self.out_bytes_whole().len();
                    self.meta_mut().init.set_init(whole_buf_len);
                }
            }
        }
    }

    /// Reborrows `OutBuf` and casts to `Uninit` version.
    ///
    /// This operation is always cheap.
    pub fn as_uninit(&mut self) -> OutBuf<'_, init::Uninit> {
        self.reborrow().into_uninit()
    }

    /// Converts `OutBuf` to `Uninit` version.
    ///
    /// This operation is always cheap.
    pub fn into_uninit(self) -> OutBuf<'a, init::Uninit> {
        unsafe {
            let (bytes, meta) = self.into_raw_parts();
            // SAFETY:
            // * this type already enforces requirements for itself
            // * Meta<init::Uninit> has the same layout as `usize` because `init::Uninit` has zero
            //   size
            OutBuf::new(bytes, &mut *(&mut meta.position as *mut _ as *mut Meta<init::Uninit>))
        }
    }

    /// Zeroes the uninitialized part of the buffer, reborrows and casts it to `Init` version.
    pub fn zeroing_as_init(&mut self) -> OutBuf<'_, init::Init> {
        self.reborrow().zeroing_into_init()
    }

    /// Zeroes the uninitialized part of the buffer and converts the reference to `Init` version.
    pub fn zeroing_into_init(mut self) -> OutBuf<'a, init::Init> {
        unsafe {
            self.perform_zeroing();
            let (bytes, meta) = self.into_raw_parts();
            OutBuf::new(bytes, &mut *(&mut meta.position as *mut _ as *mut Meta<init::Init>))
        }
    }

    /// Calls the closure with reborrowed `OutBuf` and returns the slice that the closure
    /// have written into.
    #[inline]
    pub fn scoped<R, F: FnOnce(OutBuf<'_, Init>) -> R>(mut self, f: F) -> (&'a mut [u8], R) {
        let old_pos = self.meta().position;
        let result = f(self.reborrow());
        let new_pos = self.meta().position;
        unsafe {
            let (bytes, _) = self.into_raw_parts();
            let written = &mut bytes[old_pos..new_pos];

            (written.assume_init_mut(), result)
        }
    }

    /// Creates a new `OutBuf` with remaining length at most `limt`
    #[inline]
    pub fn with_limit(&mut self, limit: usize) -> OutBuf<'_, Init> {
        unsafe {
            let limit = self.remaining().min(limit);
            let max_len = self.meta().position + limit;
            let (bytes, meta) = self.reborrow().into_raw_parts();

            OutBuf::new(&mut bytes[..max_len], meta)
        }
    }

    /// Writes a byte slice into the buffer and advance the position by slice length
    ///
    /// ## Panics
    ///
    /// This method panicks if the length of the slice is greater than what buffer can hold.
    #[inline]
    pub fn write_slice(&mut self, bytes: &[u8]) {
        unsafe {
            if bytes.len() > self.out_bytes().len() {
                panic!("Attempt to write past the end of the buffer (buffer len: {}, write len: {})", self.out_bytes().len(), bytes.len());
            }
            self.out_bytes()[..bytes.len()].copy_from_slice(bytes);
            self.advance_unchecked(bytes.len());
        }
    }

    /// Writes as many bytes from slice as fit into the buffer.
    ///
    /// This method is similar to `write_slice` but it truncates the slice being written
    /// instead of panicking.
    ///
    /// Returns the number of bytes written
    #[inline]
    pub fn write_slice_min(&mut self, bytes: &[u8]) -> usize {
        unsafe {
            let to_write = bytes.len().min(self.out_bytes().len());
            // First check if the amount of bytes we want to read is small:
            // `copy_from_slice` will generally expand to a call to `memcpy`, and
            // for a single byte the overhead is significant.
            if to_write == 1 {
                self.write_byte(bytes[0]);
            } else {
                self.out_bytes()[..to_write].copy_from_slice(&bytes[..to_write]);
                self.advance_unchecked(to_write);
            }

            to_write
        }
    }

    /// Returns true if no more bytes can be written to the buffer
    #[inline]
    pub fn is_full(&self) -> bool {
        self.remaining() == 0
    }

    /// Writes a single byte into the buffer and advances the position by one
    ///
    /// ## Panics
    ///
    /// This method panicks if the buffer is full.
    #[inline]
    pub fn write_byte(&mut self, byte: u8) {
        self.write_slice(&[byte]);
    }

    /// Uncombines specified operand of `Combine`.
    ///
    /// ## Safety
    ///
    /// `T` MUST be an operand of `Combine<_, Combined=Self>`
    unsafe fn uncombine<T: BufInit>(mut self) -> OutBuf<'a, T> {
        // only Init needs init so if Init is Init it's already init and doesn't need init
        // if it's not init it needs init :)
        if T::needs_init() && !Init::needs_init() {
            self.perform_zeroing();
        }

        let (bytes, meta) = self.into_raw_parts();
        // SAFETY:
        // * We did initialize the buffer above if it was needed
        // * see each branch for additional requirements.
        if T::is_dynamic() {
            // Combine requires that Combined is Dynamic if an operand is Dynamic
            // and this fn requires that Combined is Self
            // So Self is Dynamic and T is Dynamic, thus this is a no-op but Rust doesn't understand
            // that.
            OutBuf::new(bytes, &mut *(meta as *mut _ as *mut Meta<T>))
        } else {
            // If T is not Dynamic, it's one of the other two and thus zero-sized.
            // Because T is zero-sized the layout of Meta<T> is the same as the layout of usize.
            OutBuf::new(bytes, &mut *(&mut meta.position as *mut _ as *mut Meta<T>))
        }
    }

    /// Casts this buffer initailizedness into the left operand of the `Combine` trait.
    ///
    /// This performs initialization if required but there was not way to avoid it anyway.
    #[inline]
    pub fn uncombine_left<L, R>(self) -> OutBuf<'a, L> where L: BufInit + Combine<R, Combined=Init>, R: BufInit {
        unsafe {
            // The type signature proves L is an operand of Combine<R, Combined=Self>
            self.uncombine::<L>()
        }
    }

    /// Casts this buffer initailizedness into the right operand of the `Combine` trait.
    ///
    /// This performs initialization if required but there was not way to avoid it anyway.
    #[inline]
    pub fn uncombine_right<L, R>(self) -> OutBuf<'a, R> where L: BufInit + Combine<R, Combined=Init>, R: BufInit {
        unsafe {
            // The type signature proves R is an operand of Combine<R, Combined=Self>
            self.uncombine::<R>()
        }
    }
}

impl<'a> OutBuf<'a, init::Init> {
    /// Returns the underlying buffer as "initialized" mutable reference.
    ///
    /// **Important:** reading from this is still a bad idea even if not memory-unsafe!
    /// This is provided for legacy code that doesn't use [`OutBytes`].
    #[inline]
    pub fn bytes_mut(&mut self) -> &mut [u8] {
        unsafe {
            // SAFETY: we have type proof this is actually initialized.
            self.out_bytes().assume_init_mut()
        }
    }

    /// Advances the buffer position by `amount` bytes.
    ///
    /// This does the same thing as `advance_unchecked()` but it checks the bounds and is only
    /// available when the buffer is type-proven to be initialized.
    ///
    /// ## Panics
    ///
    /// This method panicks if the amount would move the position past the end of the buffer.
    #[inline]
    pub fn advance(&mut self, amount: usize) {
        unsafe {
            if amount > self.remaining() {
                panic!("Attempt to advance past the buffer");
            }
            // SAFETY: we have type proof this is actually initialized and we've just checked
            // the amount.
            self.advance_unchecked(amount);
        }
    }
}

/// Marker trait guaranteeing that the bytes in storage are initialized.
///
/// This is used to prove safety of some operations.
///
/// ## Safety
///
/// This trait MUST NOT be implemented on types that return uninitialized slices from
/// `borrow_uninit_slice()`.
pub unsafe trait BorrowOutBytesInit: BorrowOutSlice<u8> {}

macro_rules! impl_borrow_out_bytes_init_array {
    ($($n:expr),* $(,)?) => {
        $(
            unsafe impl BorrowOutBytesInit for [u8; $n] {}
        )*
    }
}

impl_borrow_out_bytes_init_array! {
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25,
    26, 27, 28, 29, 30, 31, 32, 64, 128, 256, 512, 1024, 2048, 4096, 8192, 16384, 32768, 65536,
}

unsafe impl<'a> BorrowOutBytesInit for &'a mut [u8] {}
#[cfg(feature = "alloc")]
unsafe impl BorrowOutBytesInit for Box<[u8]> {}

impl<S: BorrowOutBytesInit, I: BufInit> Buffer<S, I> {
    /// Creates a new buffer using the provided storage and position starting at 0
    ///
    /// This version of buffer is guaranteed to be initialized.
    #[inline]
    pub fn new_from_init(bytes: S) -> Self {
        unsafe {
            let meta = Meta {
                position: 0,
                // SAFETY: trait bound on S guarantees that the argument is initialized.
                init: I::new_unchecked(bytes.borrow_uninit_slice().len()),
            };
            Buffer::new_unchecked(bytes, meta)
        }
    }
}

impl<S: BorrowOutSlice<u8>, I: FromUninit> Buffer<S, I> {
    /// Creates a new buffer using provided storage and position starting at 0
    ///
    /// This can be used to create either `Dynamic` or `Uninit` buffer.
    #[inline]
    pub fn new(bytes: S) -> Self {
        unsafe {
            let len = bytes.borrow_uninit_slice().len();
            let meta = Meta {
                position: 0,
                init: I::new(if bytes.is_init() { len } else { 0 })
            };

            Buffer::new_unchecked(bytes, meta)
        }
    }
}

impl<S: BorrowOutSlice<u8>, I: BufInit> Buffer<S, I> {
    /// Creates a new buffer using provided storage and position starting at 0 initializing the
    /// buffer if required.
    ///
    /// This can be used to create any buffer generically but the cost of initialization will not
    /// be caught by type errors. This may be an issue in high-performance applications as
    /// refactoring the code could cause performance regressions. Such regressions would cause type
    /// errors if non-generic methods were used.
    #[inline]
    pub fn new_maybe_init(mut bytes: S) -> Self {
        unsafe {
            if I::needs_init() {
                bytes.zero_if_needed();
            }
            let len = bytes.borrow_uninit_slice().len();
            let meta = Meta {
                position: 0,
                init: I::new_unchecked(if bytes.is_init() { len } else { 0 })
            };

            Buffer::new_unchecked(bytes, meta)
        }
    }
}

#[cfg(feature = "alloc")]
impl<Init: BufInit> From<Buffer<Box<[u8]>, Init>> for Vec<u8> {
    fn from(value: Buffer<Box<[u8]>, Init>) -> Self {
        unsafe {
            let (bytes, position) = value.into_parts();
            let capacity = bytes.len();
            let ptr = Box::into_raw(bytes) as *mut u8;
            Vec::from_raw_parts(ptr, position, capacity)
        }
    }
}

#[cfg(feature = "alloc")]
impl<Init: BufInit> From<Buffer<Box<[MaybeUninit<u8>]>, Init>> for Vec<u8> {
    fn from(value: Buffer<Box<[MaybeUninit<u8>]>, Init>) -> Self {
        unsafe {
            let (bytes, position) = value.into_parts();
            let capacity = bytes.len();
            let ptr = Box::into_raw(bytes) as *mut u8;
            Vec::from_raw_parts(ptr, position, capacity)
        }
    }
}

/// Extension trait for byte slices implementing helper method(s)
pub trait ByteSliceExt: BorrowOutSlice<u8> {
    /// The version of buffer created from this slice.
    type Init: BufInit;

    /// Treats this slice as a buffer of bytes
    fn as_buffer(&mut self) -> Buffer<&mut Self, Self::Init>;
}

impl ByteSliceExt for [u8] {
    type Init = init::Init;

    fn as_buffer(&mut self) -> Buffer<&mut Self, Self::Init> {
        Buffer::new_from_init(self)
    }
}

impl ByteSliceExt for [MaybeUninit<u8>] {
    type Init = init::Uninit;

    fn as_buffer(&mut self) -> Buffer<&mut Self, Self::Init> {
        Buffer::new(self)
    }
}

/// Allows generically creating uninitialized byte storages.
///
/// This is mainly used as implementation detail of [`new_uninit`] and the related helper
/// functions. It is implemented for uninit arrays and boxed arrays (with `alloc` feature) and you
/// can implement it for your own types too.
pub trait NewUninit: BorrowOutSlice<u8> + Sized {
    /// Creates byte storage with all bytes being uninitialized.
    fn new_uninit() -> Self;
}

/// Helper making it easier to construct uninitialized buffers.
///
/// You can use this to quickly construct a buffer holding an array on stack (probably the most
/// frequent use case) or other types. It is specifically **not** a method of [`Buffer`] to reduce
/// the boilerplate related to generics.
pub fn new_uninit<T: NewUninit>() -> Buffer<T, init::Uninit> {
    Buffer::new(T::new_uninit())
}

/// Helper making it easier to construct buffers in generic code.
///
/// This helper can be very useful when writing code that works with buffers generic over their
/// initializedness. It creates uninitialized buffer if possible but will zero it out if required
/// by the `Init` type parameter.
///
/// You can use this to quickly construct a buffer holding an array on stack (probably the most
/// frequent use case) or other types. It is specifically **not** a method of [`Buffer`] to reduce
/// the boilerplate related to generics.
pub fn new_maybe_init<T: NewUninit, Init: BufInit>() -> Buffer<T, Init> {
    Buffer::new_maybe_init(T::new_uninit())
}

/// Helper for creating uninitalized boxes in older versions of Rust.
///
/// This is used in the following two functions.
#[cfg(feature = "alloc")]
fn uninit_boxed_slice(capacity: usize) -> Box<[MaybeUninit<u8>]> {
    unsafe {
        let mut vec = Vec::<MaybeUninit<u8>>::with_capacity(capacity);
        // SAFETY:
        // * with_capacity is guaranteed to allocate `capacity` items (bytes in this case)
        // * `set_len` means essentially `assume_init` for the part of the buffer up to len
        //   and `assume_init` is valid on `MaybeUninit<MaybeUninit<T>>`.
        vec.set_len(capacity);
        // Since len == capacity this will not reallocate
        vec.into_boxed_slice()
    }
}

/// Creates a boxed slice holding uninitialized bytes.
///
/// This function heap-allocates an uninitialized slice and uses it as a backing storage for a
/// buffer. While allocating on heap is generally slower than on stack, this has the benefit of
/// being faster to move around.
///
/// If possible it's still better to use `new_uninit::<Box<[MaybeUninit]>>()` because it doesn't
/// need to store the capacity. Some situations when this isn't possible:
///
/// * `capacity` is dynamic - not known at compile time
/// * You need collection of buffers with different sizes
/// * You can't use const generics (due to MSRV) and you need unusual capacity.
#[cfg(feature = "alloc")]
pub fn new_uninit_boxed_slice(capacity: usize) -> Buffer<Box<[MaybeUninit<u8>]>, init::Uninit> {
    Buffer::new(uninit_boxed_slice(capacity))
}

/// Creates a boxed slice holding bytes initializing them if required.
///
/// This function is very similar to [`new_uninit_boxed_slice`] so that documentation applies here
/// too. The main difference is this one can create a buffer of arbitrary initializedness and
/// zeroes out the bytes if required. Thus it may be silently slower than the uninit one but works
/// in generic code.
#[cfg(feature = "alloc")]
pub fn new_maybe_init_boxed_slice(capacity: usize) -> Buffer<Box<[MaybeUninit<u8>]>, init::Uninit> {
    Buffer::new_maybe_init(uninit_boxed_slice(capacity))
}

/// Creates an `OutBuf` from given `Vec<u8>` (can be a reference) and calls a closure with it
/// updating the length of `Vec` after the closure returns.
///
/// This is very useful for safely reading into uninitialized portion of `Vec` without leaking the
/// fact that the underlying storage is actually a `Vec`.
#[cfg(feature = "alloc")]
pub fn with_vec_as_out_buf<I, R, V, F>(mut vec: V, fun: F) -> R where
        I: BufInit,
        V: BorrowMut<Vec<u8>>,
        F: FnOnce(OutBuf<'_, I>) -> R {

    unsafe {
        let vec = vec.borrow_mut();
        let len = vec.len();
        let capacity = vec.capacity();
        let uninit_ptr = vec.as_mut_ptr().add(len).cast::<MaybeUninit<u8>>();
        let vec_uninit: &mut [MaybeUninit<u8>] = core::slice::from_raw_parts_mut(uninit_ptr, capacity - len);
        let mut buffer = Buffer::<_, I>::new_maybe_init(vec_uninit);
        let result = fun(buffer.as_out());
        let new_len = len + buffer.written().len();
        vec.set_len(new_len);
        result
    }
}

mod new_uninit_arr_impl {
    use super::NewUninit;
    use core::mem::MaybeUninit;
    #[cfg(feature = "alloc")]
    use alloc::boxed::Box;

    macro_rules! impl_new_uninit_array {
        ($($n:expr),* $(,)*) => {
            $(
                impl NewUninit for [MaybeUninit<u8>; $n] {
                    fn new_uninit() -> Self {
                        unsafe {
                            MaybeUninit::<[MaybeUninit<u8>; $n]>::uninit()
                                // SAFETY: assume_init on an array of uninitialized items is sound.
                                .assume_init()
                        }
                    }
                }

                #[cfg(feature = "alloc")]
                impl NewUninit for Box<[MaybeUninit<u8>; $n]> {
                    fn new_uninit() -> Self {
                        Box::new(NewUninit::new_uninit())
                    }
                }
            )*
        }
    }

    impl_new_uninit_array! {
        0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24,
        25, 26, 27, 28, 29, 30, 31, 32, 64, 128, 256, 512, 1024, 2048, 4096, 8192, 16384, 32768,
        65536,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_uninit_basic_ops() {
        let mut buffer = new_uninit::<[MaybeUninit<u8>; 32]>();
        assert_eq!(buffer.written(), &[]);
        assert_eq!(buffer.as_out().remaining(), 32);
        buffer.as_out().write_slice(&[]);
        assert_eq!(buffer.written(), &[]);
        assert_eq!(buffer.as_out().remaining(), 32);
        buffer.as_out().write_slice(&[42]);
        assert_eq!(buffer.as_out().remaining(), 31);
        assert_eq!(buffer.written(), &[42]);
        buffer.as_out().write_slice(&[1, 2, 3, 4, 5]);
        assert_eq!(buffer.as_out().remaining(), 26);
        assert_eq!(buffer.written(), &[42, 1, 2, 3, 4, 5]);
        buffer.as_out().write_byte(47);
        assert_eq!(buffer.as_out().remaining(), 25);
        assert_eq!(buffer.written(), &[42, 1, 2, 3, 4, 5, 47]);
        for i in 0..25 {
            buffer.as_out().write_byte(255 - i);
            assert_eq!(buffer.as_out().remaining(), usize::from(24 - i));
            assert_eq!(*buffer.written().last().unwrap(), 255 - i);
        }
        assert!(buffer.as_out().is_full());
    }
}
