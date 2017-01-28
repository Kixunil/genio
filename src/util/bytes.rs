use Read;

/// Represents reader as iterator over bytes.
///
/// Note: this iterator is unbuffered, so it's advised to use `BufReader` to improve performance.
pub struct Bytes<R> {
    reader: R,
}

impl<R: Read> Bytes<R> {
    /// Creates the iterator.
    pub fn new(reader: R) -> Self {
        Bytes {
            reader: reader,
        }
    }
}

impl<R: Read> Iterator for Bytes<R> {
    type Item = Result<u8, R::ReadError>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut buf = [0];
        match self.reader.read(&mut buf) {
            Ok(0) => None,
            Ok(_) => Some(Ok(buf[0])),
            Err(e) => Some(Err(e)),
        }
    } 
}
