/// Trait representing marker types usable for tracking of initializedness.
///
/// This trait is sealed so only three types can actually be used.
pub trait BufInit: sealed::BufInit + sealed::Combine<init::Init> + sealed::Combine<init::Uninit> + sealed::Combine<init::Dynamic> {
}

impl<T: sealed::BufInit + sealed::Combine<init::Init> + sealed::Combine<init::Uninit> + sealed::Combine<init::Dynamic>> BufInit for T {}

/// Markers for types that can be constructed from uninitialized data.
///
/// This trait is sealed so only two types can actually be used: `Uninit` and `Dynamic`.
pub trait FromUninit: BufInit + sealed::FromUninit {}

impl<T: BufInit + sealed::FromUninit> FromUninit for T {}

/// A type operator that produces the most optimal initializedness marker to be consumed by two
/// generic byte producers.
///
/// This can be used in generic code working with two generic types to pick the most performant
/// combined type. The implementation produces same type for equal types and `Dynamic` for unequal
/// types.
pub trait Combine<T: BufInit>: sealed::Combine<T> {
    /// The output of the type operator.
    type Combined: BufInit;
}

impl<T, U> Combine<U> for T where T: sealed::Combine<U>, U: BufInit {
    type Combined = <T as sealed::Combine<U>>::Combined;
}

pub(crate) mod sealed {
    use super::init;

    pub unsafe trait BufInit: Sized {
        type WithInit: super::BufInit;
        type WithDynamic: super::BufInit;
        type WithUninit: super::BufInit;

        // None means whole buffer!
        fn init_len(&self) -> Option<usize>;
        unsafe fn set_init(&mut self, len: usize);
        unsafe fn update_min(&mut self, len: usize);
        fn needs_init() -> bool;
        unsafe fn new_unchecked(init: usize) -> Self;
        fn is_dynamic() -> bool;
    }

    pub unsafe trait FromUninit: BufInit {
        fn new(init: usize) -> Self;
    }

    // The trait is unsafe because if any operand is Dynamic then Combined MUST be Dynamic
    pub unsafe trait Combine<T: BufInit>: Sized {
        type Combined: super::BufInit;
    }

    unsafe impl<T: BufInit> Combine<T> for init::Init {
        type Combined = T::WithInit;
    }

    unsafe impl<T: BufInit> Combine<T> for init::Dynamic {
        type Combined = T::WithDynamic;
    }

    unsafe impl<T: BufInit> Combine<T> for init::Uninit {
        type Combined = T::WithUninit;
    }
}

/// Contains markers for initializedness.
pub mod init {
    use super::sealed::{BufInit, FromUninit};

    /// Marker not giving any guarantees about the initializedness of storage.
    // SAFETY: this must remain zero-sized!
    pub struct Uninit(());

    /// Marker dynamically tracking initializedness of storage.
    ///
    /// This provides flexibility at the cost of overhead.
    pub struct Dynamic(usize);

    /// Marker guaranteeing that the storage is actually initialized.
    pub struct Init(());

    impl Init {
        #[inline]
        pub(crate) unsafe fn new() -> Self {
            Init(())
        }
    }

    unsafe impl FromUninit for Uninit {
        #[inline]
        fn new(_init: usize) -> Self {
            Uninit(())
        }
    }

    unsafe impl BufInit for Uninit {
        type WithInit = Dynamic;
        type WithDynamic = Dynamic;
        type WithUninit = Self;

        #[inline]
        fn init_len(&self) -> Option<usize> {
            Some(0)
        }

        #[inline]
        unsafe fn set_init(&mut self, _len: usize) {}

        #[inline]
        unsafe fn update_min(&mut self, _len: usize) {}

        #[inline]
        fn needs_init() -> bool {
            false
        }

        #[inline]
        unsafe fn new_unchecked(_init: usize) -> Self {
            Uninit(())
        }

        fn is_dynamic() -> bool {
            false
        }
    }

    unsafe impl BufInit for Init {
        type WithInit = Self;
        type WithDynamic = Dynamic;
        type WithUninit = Dynamic;

        #[inline]
        fn init_len(&self) -> Option<usize> {
            None
        }

        #[inline]
        unsafe fn set_init(&mut self, _len: usize) {}

        #[inline]
        unsafe fn update_min(&mut self, _len: usize) {}

        #[inline]
        fn needs_init() -> bool {
            true
        }

        #[inline]
        unsafe fn new_unchecked(_init: usize) -> Self {
            Init(())
        }

        fn is_dynamic() -> bool {
            false
        }
    }

    unsafe impl FromUninit for Dynamic {
        #[inline]
        fn new(init: usize) -> Self {
            Dynamic(init)
        }
    }

    unsafe impl BufInit for Dynamic {
        type WithInit = Self;
        type WithDynamic = Self;
        type WithUninit = Self;

        #[inline]
        fn init_len(&self) -> Option<usize> {
            Some(self.0)
        }

        #[inline]
        unsafe fn set_init(&mut self, len: usize) {
            self.0 = len;
        }

        #[inline]
        unsafe fn update_min(&mut self, len: usize) {
            if len > self.0 {
                self.0 = len;
            }
        }

        #[inline]
        fn needs_init() -> bool {
            false
        }

        #[inline]
        unsafe fn new_unchecked(init: usize) -> Self {
            Dynamic(init)
        }

        fn is_dynamic() -> bool {
            true
        }
    }
}
