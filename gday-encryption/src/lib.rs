mod reader;
mod writer;

#[cfg(test)]
mod tests;

pub use reader::EncryptedReader;
pub use writer::EncryptedWriter;

const MAX_CHUNK_SIZE: usize = 10_000;


