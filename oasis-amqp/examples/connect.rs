use std::io::Cursor;

use bytes::Buf;
use oasis_amqp::{
    sasl, AmqpFrame, Begin, Frame, Open, Performative, AMQP_PROTO_HEADER, MIN_MAX_FRAME_SIZE,
    SASL_PROTO_HEADER,
};
use serde_bytes::Bytes;
use tokio::net::TcpStream;
use tokio::{
    self,
    io::{AsyncReadExt, AsyncWriteExt},
};

#[tokio::main]
async fn main() {
    let mut stream = TcpStream::connect("127.0.0.1:10006").await.unwrap();
    stream.write_all(SASL_PROTO_HEADER).await.unwrap();
    let mut frames = vec![0u8; MIN_MAX_FRAME_SIZE];
    let len = stream.read(&mut frames).await.unwrap();
    println!("received {} bytes: {:?}", len, &frames[..len]);
    let mut buf = Cursor::new(&frames[..len]);

    let mut header = [0u8; 8];
    buf.copy_to_slice(&mut header);
    assert_eq!(header, SASL_PROTO_HEADER);

    let pos = buf.position() as usize;
    let frame = Frame::decode(&frames[pos..len]);
    println!("read: {:#?}", frame);

    let init = Frame::Sasl(sasl::Frame::Init(sasl::Init {
        mechanism: sasl::Mechanism::Plain,
        initial_response: Some(Bytes::new(b"\x00vxdir\x00vxdir")),
        hostname: None,
    }));

    let buf = init.to_vec();
    println!("\nwriting {:?}", buf);
    stream.write_all(&buf).await.unwrap();

    let len = stream.read(&mut frames).await.unwrap();
    println!("received {} bytes: {:?}", len, &frames[..len]);
    let msg_len = Cursor::new(&frames[..len]).get_u32_be() as usize;
    let msg = Frame::decode(&frames[..msg_len]);
    println!("read: {:#?}", msg);
    assert_eq!(&frames[msg_len..msg_len + 8], AMQP_PROTO_HEADER);

    let open = Frame::Amqp(AmqpFrame {
        channel: 0,
        extended_header: None,
        performative: Performative::Open(Open {
            container_id: "vx-web",
            ..Default::default()
        }),
        body: &[],
    });

    let buf = open.to_vec();
    println!("\nwriting {:?}", buf);
    let _ = stream.write_all(AMQP_PROTO_HEADER).await.unwrap();
    stream.write_all(&buf).await.unwrap();

    let len = stream.read(&mut frames).await.unwrap();
    println!("received {} bytes: {:?}", len, &frames[..len]);
    let msg_len = Cursor::new(&frames[..len]).get_u32_be() as usize;
    let msg = Frame::decode(&frames[..msg_len]);
    println!("read: {:#?}", msg);

    let begin = Frame::Amqp(AmqpFrame {
        channel: 0,
        extended_header: None,
        performative: Performative::Begin(Begin {
            remote_channel: None,
            next_outgoing_id: 1,
            incoming_window: 8,
            outgoing_window: 8,
            ..Default::default()
        }),
        body: &[],
    });

    let buf = begin.to_vec();
    println!("\nwriting {:?}", buf);
    let _ = stream.write_all(&buf).await.unwrap();

    let len = stream.read(&mut frames).await.unwrap();
    println!("received {} bytes: {:?}", len, &frames[..len]);
    let msg_len = Cursor::new(&frames[..len]).get_u32_be() as usize;
    let msg = Frame::decode(&frames[..msg_len]);
    println!("read: {:#?}", msg);
}
