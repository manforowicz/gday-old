mod reader;
mod writer;

#[cfg(test)]
mod tests;

use bytes::BytesMut;
pub use reader::EncryptedReader;
pub use writer::EncryptedWriter;

const MAX_CHUNK_SIZE: usize = 8 * 1024;

const CIPHERTEXT_OVERHEAD: usize = 16;

struct HelperBuf {
    buf: BytesMut,
    cursor: usize,
}

impl HelperBuf {
    fn with_capacity(capacity: usize) -> Self {
        Self {
            buf: BytesMut::with_capacity(capacity),
            cursor: 0,
        }
    }

    fn advance_cursor(&mut self, num_bytes: usize) {
        self.cursor += num_bytes;
        assert!(self.cursor <= self.buf.len());

        if self.cursor == self.buf.len() {
            self.cursor = 0;
            self.buf.clear();
        }
    }

    fn data(&self) -> &[u8] {
        &self.buf[self.cursor..]
    }

    fn spare_capacity_len(&self) -> usize {
        self.buf.capacity() - self.buf.len()
    }

    fn wrap(&mut self) {
        let (blank, data) = self.buf.split_at_mut(self.cursor);
        let data_len = data.len();
        blank[0..data_len].copy_from_slice(data);
        self.buf.resize(data_len, 0);
        self.cursor = 0;
    }
}
