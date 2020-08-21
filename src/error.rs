//! Error types and various operations on them.

use ::core::fmt;
use void;
use void::Void;

/// Specifies an error that happened during I/O operation. This enables one to compose read and
/// write errors into single type.
///
/// This is different than `std::io::Error`!
#[derive(Debug)]
pub enum IOError<R, W> {
    /// Read operation failed.
    Read(R),
    /// Write operation failed.
    Write(W),
}

impl<R, W> IOError<R, W> {
    /// Merges the variants into common type.
    pub fn merge<E>(e: IOError<R, W>) -> E
    where
        R: Into<E>,
        W: Into<E>,
    {
        match e {
            IOError::Read(e) => e.into(),
            IOError::Write(e) => e.into(),
        }
    }

    /// Get `IOError` containing references to errors.
    pub fn as_ref(&self) -> IOError<&R, &W> {
        match *self {
            IOError::Read(ref f) => IOError::Read(f),
            IOError::Write(ref s) => IOError::Write(s),
        }
    }

    /// Get `IOError` containing mutable references to errors.
    pub fn as_ref_mut(&mut self) -> IOError<&mut R, &mut W> {
        match *self {
            IOError::Read(ref mut f) => IOError::Read(f),
            IOError::Write(ref mut s) => IOError::Write(s),
        }
    }
}

/// Error that might occur when reading exact amount of bytes.
#[derive(Debug)]
pub enum ReadExactError<E> {
    /// Low-level error happened.
    Other(E),

    /// Reader reached end unexpectedly.
    ///
    /// Usually EOF, closed connection, etc.
    UnexpectedEnd,
}

impl<E> From<E> for ReadExactError<E> {
    fn from(e: E) -> Self {
        ReadExactError::Other(e)
    }
}

/// Error returned when chained readers fail. It allows inspecting which reader failed, keeping
/// it's own error type. For simplicity it can be also merged, if the two errors are convertible to
/// resulting error.
#[derive(Debug)]
pub enum ChainError<F, S> {
    /// First reader failed.
    First(F),
    /// Second reader failed.
    Second(S),
}

impl<F, S> ChainError<F, S> {
    /// If the two errors can be converted to same type, they can be easily merged by this method.
    /// This simply performs the conversion.
    pub fn merge<E>(self) -> E
    where
        F: Into<E>,
        S: Into<E>,
    {
        match self {
            ChainError::First(f) => f.into(),
            ChainError::Second(s) => s.into(),
        }
    }

    /// Get `ChainError` containing references to errors.
    pub fn as_ref(&self) -> ChainError<&F, &S> {
        match *self {
            ChainError::First(ref f) => ChainError::First(f),
            ChainError::Second(ref s) => ChainError::Second(s),
        }
    }

    /// Get ChainError containing mutable references to errors.
    pub fn as_ref_mut(&mut self) -> ChainError<&mut F, &mut S> {
        match *self {
            ChainError::First(ref mut f) => ChainError::First(f),
            ChainError::Second(ref mut s) => ChainError::Second(s),
        }
    }
}

/// This error type indicates that operation might fail in restartible manner. The most obvious
/// case is `EINTR` returned from syscalls when a signal is delivered while doing `read`.
#[derive(Debug)]
pub enum IntrError<E> {
    /// The error wasn't interruption.
    Other(E),
    /// An operation was interrupted. This variant means that operation can be retried and it will
    /// likely succeed.
    Interrupted,
}

/// Trait for custom error types that can also represent interruption.
pub trait IntoIntrError {
    /// Type representing other error (non-interrupt).
    type NonIntr;

    /// Performs the conversion.
    fn into_intr_error(self) -> IntrError<Self::NonIntr>;
}

impl<E> IntoIntrError for IntrError<E> {
    type NonIntr = E;

    fn into_intr_error(self) -> IntrError<Self::NonIntr> {
        self
    }
}

impl<T> From<Void> for IntrError<T> {
    fn from(e: Void) -> Self {
        void::unreachable(e)
    }
}

// Should this be implemented?
/*
impl IntoIntrError for Void {
    type NonIntr = Void;

    fn into_intr_error(self) -> IntrError<Self::NonIntr> {
        self.into()
    }
}
*/

/// Error that might occur when interacting with buf reader.
#[derive(Debug)]
pub enum BufError<B, E> {
    /// The underlying stream reached the end.
    End,
    /// Buffer itself failed.
    BufferErr(B),
    /// The underlying stream failed.
    OtherErr(E),
}

/// Error that might occur when doing operation on `ExtendFromReader`
#[derive(Debug)]
pub enum ExtendError<R, E> {
    /// Reader failed
    ReadErr(R),
    /// The type being extended failed.
    ExtendErr(E),
}

/// Error indicating that provided buffer was too small
#[derive(Debug)]
pub struct BufferOverflow;

impl fmt::Display for BufferOverflow {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "provided buffer was too small")
    }
}
