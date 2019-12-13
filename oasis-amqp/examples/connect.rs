use futures::{sink::SinkExt, stream::StreamExt};
use oasis_amqp::{
    sasl, AmqpFrame, Attach, Begin, Codec, Frame, Open, Performative, Protocol, Role, Source,
    Target,
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

    println!("send header");
    transport.send(Frame::Header(Protocol::Sasl)).await.unwrap();
    println!("read: {:#?}\n", transport.next().await.unwrap().unwrap());
    println!("read: {:#?}\n", transport.next().await.unwrap().unwrap());

    let init = Frame::Sasl(sasl::Frame::Init(sasl::Init {
        mechanism: sasl::Mechanism::Plain,
        initial_response: Some(Bytes::new(b"\x00vxdir\x00vxdir")),
        hostname: None,
    }));

    println!("send init");
    transport.send(init).await.unwrap();
    println!("read: {:#?}\n", transport.next().await.unwrap().unwrap());
    println!("read: {:#?}\n", transport.next().await.unwrap().unwrap());

    let open = Frame::Amqp(AmqpFrame {
        channel: 0,
        extended_header: None,
        performative: Performative::Open(Open {
            container_id: "vx-web",
            ..Default::default()
        }),
        body: &[],
    });

    println!("send header");
    transport.send(Frame::Header(Protocol::Amqp)).await.unwrap();

    println!("send open");
    transport.send(open).await.unwrap();
    println!("read: {:#?}\n", transport.next().await.unwrap().unwrap());

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

    println!("send begin");
    transport.send(begin).await.unwrap();
    println!("read: {:#?}\n", transport.next().await.unwrap().unwrap());

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
                address: Some("RPC"),
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
    println!("read: {:#?}\n", transport.next().await.unwrap().unwrap());
}
