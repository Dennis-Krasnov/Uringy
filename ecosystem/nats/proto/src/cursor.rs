/// Wrapper of an in-memory mutable buffer to facilitate write operations.
/// Inspired by [`std::io::Cursor`], but this Cursor works in a no_std environment.
pub(crate) struct Cursor<'a> {
    inner: &'a mut [u8],
    position: usize,
}

impl<'a> Cursor<'a> {
    /// Create a new cursor wrapping the provided underlying in-memory buffer.
    /// Initially the position is 0, the start of the buffer.
    pub(crate) fn new(inner: &'a mut [u8]) -> Cursor<'a> {
        Cursor { inner, position: 0 }
    }

    /// Return the current position of this cursor.
    pub(crate) const fn position(&self) -> usize {
        self.position
    }

    /// Write the buffer to self and advance the cursor by the number of bytes written.
    ///
    /// # Errors
    /// If there isn't enough space left in the in-memory mutable buffer,
    /// it's left untouched and [`Err(NatsProtoError::BufferTooSmall)`] is returned.
    pub(crate) fn put(&mut self, buffer: &[u8]) -> Result<(), BufferTooSmall> {
        if buffer.len() > (self.inner.len() - self.position) {
            return Err(BufferTooSmall);
        }

        let inner_slice = &mut self.inner[self.position..(self.position + buffer.len())];
        inner_slice.copy_from_slice(buffer);
        self.position += buffer.len();

        Ok(())
    }
}

#[derive(Debug, PartialEq)]
pub(crate) struct BufferTooSmall;

#[cfg(test)]
mod tests {
    extern crate alloc;

    use super::*;
    use alloc::vec;

    #[test]
    fn sequentially_writes_bytes() {
        let mut buffer = vec![0; 6];
        let mut cursor = Cursor::new(&mut buffer);

        cursor.put(b"abc").unwrap();
        cursor.put(b"xyz").unwrap();

        assert_eq!(buffer[..6], b"abcxyz"[..]);
    }

    #[test]
    fn keeps_track_of_position() {
        let mut buffer = vec![0; 6];
        let mut cursor = Cursor::new(&mut buffer);

        assert_eq!(cursor.position, 0);
        cursor.put(b"abc").unwrap();
        assert_eq!(cursor.position, 3);
        cursor.put(b"xyz").unwrap();
        assert_eq!(cursor.position, 6);
    }

    #[test]
    fn fails_with_insufficient_buffer_size() {
        let mut buffer = vec![0; 5];
        let mut cursor = Cursor::new(&mut buffer);

        assert!(cursor.put(b"abc").is_ok());
        assert_eq!(cursor.put(b"xyz"), Err(BufferTooSmall));
    }
}
