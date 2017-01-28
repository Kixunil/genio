pub trait Placeholder {
    type End: Write;
    // TODO: Should we require Write?
    type Hole;

    fn end(&mut self) -> Self::End;
    fn hole(&mut self) -> Self::Hole;
}

/// Sometimes when serializing data, not all information is known until rest of the data is
/// written. For example, when header containing size of the message is needed but the message
/// length is not known.
///
/// When writing directly into IO, the only way to solve this problem is to serialize the message
/// into buffer and only after that, calculate length and write it with message.
///
/// But in case the writer is already a chunk of memory (e.g. `&mut [u8]` or `Vec<[u8]>`), it's
/// possible to skip unknown data and write it later.
///
/// This trait can be used exactly in such situation. When you need to write unknown data, call
/// `insert_placeholder()` instead and then use `end()` to write the rest of data. After it's
/// written, use `hole()` to fill skipped data.
pub trait InsertPlaceholder: Write {
    type Placeholder: Placeholder;

    fn insert_placeholder(&mut self, size: usize) -> Self::Placeholder;
}
