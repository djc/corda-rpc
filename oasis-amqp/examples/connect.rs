use futures::{sink::SinkExt, stream::StreamExt};
use oasis_amqp::{
    sasl, AmqpFrame, Attach, Begin, Codec, Frame, Open, Performative, Protocol, Role, Source,
    Target, Transfer,
};
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

    let open = Frame::Amqp(AmqpFrame {
        channel: 0,
        extended_header: None,
        performative: Performative::Open(Open {
            container_id: "vx-web",
            ..Default::default()
        }),
        body: &[],
    });

    transport.send(Frame::Header(Protocol::Amqp)).await.unwrap();
    transport.send(open).await.unwrap();
    let _opened = transport.next().await.unwrap().unwrap();

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

    transport.send(begin).await.unwrap();
    let _begun = transport.next().await.unwrap().unwrap();

    let attach = Frame::Amqp(AmqpFrame {
        channel: 0,
        extended_header: None,
        performative: Performative::Attach(Attach {
            name: "vx-web-0",
            handle: 0,
            role: Role::Sender,
            snd_settle_mode: None,
            rcv_settle_mode: None,
            source: Some(Source {
                address: Some("vx-web"),
                ..Default::default()
            }),
            target: Some(Target {
                address: Some("rpc.server"),
                ..Default::default()
            }),
            unsettled: None,
            incomplete_unsettled: None,
            initial_delivery_count: None,
            max_message_size: None,
            offered_capabilities: None,
            desired_capabilities: None,
            properties: None,
        }),
        body: &[],
    });

    println!("send attach: {:#?}", attach);
    transport.send(attach).await.unwrap();
    let _attached = transport.next().await.unwrap().unwrap();
    let flow = transport.next().await.unwrap().unwrap();
    println!("read: {:#?}\n", flow);

    let transfer = Frame::Amqp(AmqpFrame {
        channel: 0,
        extended_header: None,
        performative: Performative::Transfer(Transfer {
            handle: 0,
            delivery_id: Some(0),
            ..Default::default()
        }),
        body: &[],
    });

    println!("send transfer: {:#?}", transfer);
    transport.send(transfer).await.unwrap();
    let transferred = transport.next().await;
    println!("read: {:#?}\n", transferred.unwrap().unwrap());
}
