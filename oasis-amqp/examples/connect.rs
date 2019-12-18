use std::collections::HashMap;
use std::convert::TryFrom;
use std::time::SystemTime;

use futures::{sink::SinkExt, stream::StreamExt};
use oasis_amqp::{amqp, sasl, Codec, Frame, Protocol};
use serde_bytes::Bytes;
use tokio;
use tokio::net::TcpStream;
use tokio_util::codec::Framed;

#[tokio::main]
async fn main() {
    let stream = TcpStream::connect("127.0.0.1:10006").await.unwrap();
    println!("local addr {:?}", stream.local_addr());
    let mut transport = Framed::new(stream, Codec);

    transport.send(Frame::Header(Protocol::Sasl)).await.unwrap();
    let _header = transport.next().await.unwrap().unwrap();
    let _mechanisms = transport.next().await.unwrap().unwrap();

    let init = Frame::Sasl(sasl::Frame::Init(sasl::Init {
        mechanism: sasl::Mechanism::Plain,
        initial_response: Some(Bytes::new(b"\x00vxdir\x00vxdir")),
        hostname: None,
    }));

    transport.send(init).await.unwrap();
    let _outcome = transport.next().await.unwrap().unwrap();
    let _header = transport.next().await.unwrap().unwrap();

    let open = Frame::Amqp(amqp::Frame {
        channel: 0,
        extended_header: None,
        performative: amqp::Performative::Open(amqp::Open {
            container_id: "vx-web",
            ..Default::default()
        }),
        message: None,
    });

    transport.send(Frame::Header(Protocol::Amqp)).await.unwrap();
    transport.send(open).await.unwrap();
    let _opened = transport.next().await.unwrap().unwrap();

    let begin = Frame::Amqp(amqp::Frame {
        channel: 0,
        extended_header: None,
        performative: amqp::Performative::Begin(amqp::Begin {
            remote_channel: None,
            next_outgoing_id: 1,
            incoming_window: 8,
            outgoing_window: 8,
            ..Default::default()
        }),
        message: None,
    });

    transport.send(begin).await.unwrap();
    let _begun = transport.next().await.unwrap().unwrap();

    let attach = Frame::Amqp(amqp::Frame {
        channel: 0,
        extended_header: None,
        performative: amqp::Performative::Attach(amqp::Attach {
            name: "vx-web-0",
            handle: 0,
            role: amqp::Role::Sender,
            snd_settle_mode: None,
            rcv_settle_mode: None,
            source: Some(amqp::Source {
                address: Some("vx-web"),
                ..Default::default()
            }),
            target: Some(amqp::Target {
                address: Some("rpc.server"),
                ..Default::default()
            }),
            unsettled: None,
            incomplete_unsettled: None,
            initial_delivery_count: Some(0),
            max_message_size: None,
            offered_capabilities: None,
            desired_capabilities: None,
            properties: None,
        }),
        message: None,
    });

    transport.send(attach).await.unwrap();
    let _attached = transport.next().await.unwrap().unwrap();
    let _flow = transport.next().await.unwrap().unwrap();

    let now = SystemTime::now();
    let timestamp = now.duration_since(SystemTime::UNIX_EPOCH).unwrap();
    let timestamp = i64::try_from(timestamp.as_millis()).unwrap();

    let mut properties = HashMap::new();
    properties.insert("tag", amqp::Any::I32(0));
    properties.insert("method-name", amqp::Any::Str("networkMapSnapshot"));
    properties.insert(
        "rpc-id",
        amqp::Any::Str("c662b784-d9d1-4320-9c39-62bffa3975b6"),
    );
    properties.insert("rpc-id-timestamp", amqp::Any::I64(timestamp));
    properties.insert(
        "rpc-session-id",
        amqp::Any::Str("2ac4ccb9-0da0-4ddc-8db7-cee4bb0115a1"),
    );
    properties.insert("rpc-session-id-timestamp", amqp::Any::I64(timestamp));

    let transfer = Frame::Amqp(amqp::Frame {
        channel: 0,
        extended_header: None,
        performative: amqp::Performative::Transfer(amqp::Transfer {
            handle: 0,
            delivery_id: Some(0),
            delivery_tag: Some(Bytes::new(b"foobar!")),
            message_format: Some(0),
            ..Default::default()
        }),
        message: Some(amqp::Message {
            properties: Some(amqp::Properties {
                message_id: Some("rpc.server.vxdir.6543431538375707788"),
                reply_to: Some("vx-web"),
                ..Default::default()
            }),
            application_properties: Some(properties),
            body: Some(amqp::Body::Data(amqp::Data(Bytes::new(b"")))),
            ..Default::default()
        }),
    });

    println!("send transfer: {:#?}", transfer);
    transport.send(transfer).await.unwrap();
    let transferred = transport.next().await.unwrap().unwrap();
    println!("read: {:#?}\n", transferred);
}
