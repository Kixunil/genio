//! This module contains various generic utilities related to IO.

mod bytes;
mod chain;
mod empty;
mod repeat;
mod repeat_bytes;
mod restarting;
mod sink;
mod write_trunc;

pub use self::bytes::Bytes;
pub use self::chain::Chain;
pub use self::empty::Empty;
pub use self::repeat::Repeat;
pub use self::repeat_bytes::RepeatBytes;
pub use self::restarting::Restarting;
pub use self::sink::Sink;
pub use self::write_trunc::WriteTrunc;

const DEFAULT_BUF_SIZE: usize = 8 * 1024;

use crate::error::IOError;
use crate::{Read, Write};
use core::mem::MaybeUninit;

/// Copies the entire contents of a reader into a writer.
///
/// This function will continuously read data from reader and then write it into writer in a
/// streaming fashion until reader returns EOF.
///
/// On success, the total number of bytes that were copied from reader to writer is returned.
///
/// # Errors
///
/// This function will return an error immediately if any call to `read` or write returns an error.
/// **Warning:** This function does not restart calls if they are interrupted by EINTR. Use
/// `genio::util::Restarting` to restart calls.
pub fn copy<R: ?Sized + Read, W: ?Sized + Write>(
    reader: &mut R,
    writer: &mut W,
) -> Result<u64, IOError<R::ReadError, W::WriteError>> {
    use crate::ext::{ReadExt, ReadResult};

    let mut buf = buffer::new_maybe_init::<[MaybeUninit<u8>; DEFAULT_BUF_SIZE], R::BufInit>();
    let mut written = 0;

    while let ReadResult::Bytes(b) = reader.read_ext(buf.as_out()).map_err(IOError::Read)? {
        writer.write_all(b).map_err(|error| IOError::Write(error.into_inner()))?;
        written += b.len() as u64;
        buf.reset();
    }
    Ok(written)
}
