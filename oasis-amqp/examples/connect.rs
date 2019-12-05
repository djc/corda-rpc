use std::io::Cursor;

use bytes::Buf;
use oasis_amqp::{sasl, Frame};
use serde_bytes::Bytes;
use tokio::net::TcpStream;
use tokio::{
    self,
    io::{AsyncReadExt, AsyncWriteExt},
};

#[tokio::main]
async fn main() {
    let mut stream = TcpStream::connect("127.0.0.1:10006").await.unwrap();
    stream
        .write_all(oasis_amqp::SASL_PROTO_HEADER)
        .await
        .unwrap();
    let mut frames = [0u8; 1024];
    let len = stream.read(&mut frames).await.unwrap();
    println!("received {} bytes: {:?}", len, &frames[..len]);
    let mut buf = Cursor::new(&frames[..len]);

    let mut header = [0u8; 8];
    buf.copy_to_slice(&mut header);
    assert_eq!(header, oasis_amqp::SASL_PROTO_HEADER);

    let pos = buf.position() as usize;
    let frame = Frame::decode(&frames[pos..len]);
    println!("{:#?}", frame);

    let init = Frame::Sasl(sasl::Frame::Init(sasl::Init {
        mechanism: sasl::Mechanism::Plain,
        initial_response: Some(Bytes::new(b"\x00vxdir\x00vxdir")),
        hostname: None,
    }));

    let buf = init.to_vec();
    stream.write_all(&buf).await.unwrap();

    let len = stream.read(&mut frames).await.unwrap();
    let msg_len = Cursor::new(&frames[..len]).get_u32_be() as usize;
    let msg = Frame::decode(&frames[..msg_len]);
    println!("{:#?}", msg);
}
