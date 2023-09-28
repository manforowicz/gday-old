use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::test]
async fn test_encryption() {

    

    let key: [u8; 32] = rand::random();
    let (read_stream, write_stream) = tokio::io::duplex(100);
    let mut buf = vec![0u8; 3];
    let mut writer = crate::EncryptedWriter::new(write_stream, key).await.unwrap();
    let mut reader = crate::EncryptedReader::new(read_stream, key).await.unwrap();


    writer.write_all(b"abc").await.unwrap();
    writer.flush().await.unwrap();
    reader.read_exact(&mut buf).await.unwrap();
    assert_eq!(buf.as_slice(), b"abc");
    

    
    writer.write_all(b"def").await.unwrap();
    
    writer.flush().await.unwrap();
    println!("WOHOHFSOFHSOFHSOHF:");
    reader.read_exact(&mut buf).await.unwrap();
    println!("WOHOHFSOFHSOFHSOHF:");
    assert_eq!(buf.as_slice(), b"def");

    writer.write_all(b"hij").await.unwrap();
    writer.flush().await.unwrap();
    reader.read_exact(&mut buf).await.unwrap();
    assert_eq!(buf.as_slice(), b"hij");

}