use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::test]
async fn test_all() {
    let key: [u8; 32] = rand::random();
    let (read_stream, write_stream) = tokio::io::duplex(100);
    let mut buf = vec![0u8; 3];
    let mut writer = crate::EncryptedWriter::new(write_stream, key).await.unwrap();
    let mut reader = crate::EncryptedReader::new(read_stream, key).await.unwrap();

    let test_data = [b"abc", b"def", b"ghi", b"jkl", b"mno", b"prs", b"tuw", b"yzz"];

    for msg in test_data {
        writer.write_all(msg).await.unwrap();
        writer.flush().await.unwrap();
        reader.read_exact(&mut buf).await.unwrap();
        assert_eq!(buf, msg[..]);
    }
}