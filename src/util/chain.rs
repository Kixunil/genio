use crate::error::ChainError;
use crate::{Read, OutBuf};
use buffer::Combine;

/// Chains two readers.
///
/// All reads are forwarded to the first reader until it returns end. Then all other reads are
/// forwarded to the second reader.
pub struct Chain<F, S> {
    first: F,
    second: S,
    first_finished: bool,
}

impl<F: Read, S: Read> Chain<F, S> {
    /// Creates chain of readers.
    pub fn new(first: F, second: S) -> Self {
        Chain {
            first: first,
            second: second,
            first_finished: false,
        }
    }
}

impl<F: Read, S: Read> Read for Chain<F, S> where F::BufInit: Combine<S::BufInit> {
    type ReadError = ChainError<F::ReadError, S::ReadError>;
    type BufInit = <F::BufInit as Combine<S::BufInit>>::Combined;

    fn read(&mut self, buf: OutBuf<'_, Self::BufInit>) -> Result<usize, Self::ReadError> {
        if self.first_finished {
            self.second.read(buf.uncombine_right::<F::BufInit, S::BufInit>()).map_err(ChainError::Second)
        } else {
            self.first.read(buf.uncombine_left::<F::BufInit, S::BufInit>()).map_err(ChainError::First).map(|l| {
                if l == 0 {
                    self.first_finished = true;
                }
                l
            })
        }
    }
}
