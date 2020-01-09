use std::collections::HashMap;
use std::convert::TryFrom;
use std::time::SystemTime;

use futures::{sink::SinkExt, stream::StreamExt};
use oasis_amqp::{amqp, sasl, ser, Codec, Described, Frame, Protocol};
use oasis_amqp_derive::amqp as amqp_derive;
use rand::{self, Rng};
use serde::{Deserialize, Serialize};
use serde_bytes::{ByteBuf, Bytes};
use tokio;
use tokio::net::TcpStream;
use tokio_util::codec::Framed;
use uuid::Uuid;

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
            name: "vx-web-sender".into(),
            handle: 0,
            role: amqp::Role::Sender,
            snd_settle_mode: None,
            rcv_settle_mode: None,
            source: Some(amqp::Source {
                address: Some("vx-web".into()),
                ..Default::default()
            }),
            target: Some(amqp::Target {
                address: Some("rpc.server".into()),
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

    let rpc_id = format!("{:x}", Uuid::new_v4().to_hyphenated());
    let rpc_session_id = format!("{:x}", Uuid::new_v4().to_hyphenated());
    let delivery_tag = Uuid::new_v4();
    let msg_id = format!("{:x?}", &rand::thread_rng().gen::<[u8; 8]>());
    let message_id = format!(
        "rpc.client.vxdir.{}",
        &msg_id[1..msg_id.len() - 1].replace(", ", "")
    );

    let mut properties = HashMap::new();
    properties.insert("JMSReplyTo", amqp::Any::Str(msg_id.into()));
    properties.insert("_AMQ_VALIDATED_USER", amqp::Any::Str("vxdir".into()));
    properties.insert("tag", amqp::Any::I32(0));
    properties.insert("method-name", amqp::Any::Str("networkMapSnapshot".into()));
    properties.insert("rpc-id", amqp::Any::Str(rpc_id.into()));
    properties.insert("rpc-id-timestamp", amqp::Any::I64(timestamp));
    properties.insert("rpc-session-id", amqp::Any::Str(rpc_session_id.into()));
    properties.insert("rpc-session-id-timestamp", amqp::Any::I64(timestamp));
    properties.insert("deduplication-sequence-number", amqp::Any::I64(0));

    let mut body = vec![];
    body.extend_from_slice(CORDA_MAGIC);
    body.push(SectionId::DataAndStop as u8);
    let envelope = Envelope {
        obj: ObjectList(amqp::List::default()),
        schema: Schema {
            types: vec![TypeNotation::RestrictedType(RestrictedType {
                name: "java.util.List<java.lang.Object>",
                label: None,
                provides: amqp::List::default(),
                source: "list",
                descriptor: Descriptor {
                    name: Some("net.corda:1BLPJgNvsxdvPcbrIQd87g==".into()),
                    code: None,
                },
                choices: amqp::List::default(),
            })]
            .into(),
        },
    };
    ser::into_bytes(&envelope, &mut body).unwrap();
    println!("body: {:?}", body);

    let transfer = Frame::Amqp(amqp::Frame {
        channel: 0,
        extended_header: None,
        performative: amqp::Performative::Transfer(amqp::Transfer {
            handle: 0,
            delivery_id: Some(0),
            delivery_tag: Some(ByteBuf::from(delivery_tag.as_bytes().to_vec())),
            message_format: Some(0),
            ..Default::default()
        }),
        message: Some(amqp::Message {
            properties: Some(amqp::Properties {
                message_id: Some(message_id.clone().into()),
                reply_to: Some("vx-web-sender".into()),
                user_id: Some(Bytes::new(b"vxdir")),
                ..Default::default()
            }),
            application_properties: Some(amqp::ApplicationProperties(properties)),
            body: Some(amqp::Body::Data(amqp::Data(ByteBuf::from(body)))),
            ..Default::default()
        }),
    });

    println!("send transfer: {:#?}", transfer);
    transport.send(transfer).await.unwrap();
    let transferred = transport.next().await.unwrap().unwrap();
    println!("read: {:#?}\n", transferred);

    /*
    let attach = Frame::Amqp(amqp::Frame {
        channel: 0,
        extended_header: None,
        performative: amqp::Performative::Attach(amqp::Attach {
            name: message_id.clone(),
            handle: 1,
            role: amqp::Role::Receiver,
            snd_settle_mode: None,
            rcv_settle_mode: None,
            source: Some(amqp::Source {
                address: Some("vx-web"),
                ..Default::default()
            }),
            target: Some(amqp::Target {
                address: Some("rpc.client".into()),
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
        message: None,
    });

    println!("send transfer: {:#?}", attach);
    transport.send(attach).await.unwrap();
    */
    let next = transport.next().await;
    println!("read: {:#?}\n", next);
}

#[derive(Debug, Deserialize, Serialize)]
enum SectionId {
    DataAndStop,
    AltDataAndStop,
    Encoding,
}

#[amqp_derive(descriptor(code = 0xc5620000_00000001))]
#[derive(Debug, Deserialize, Serialize)]
struct Envelope<'a, T> {
    pub obj: T,
    #[serde(borrow)]
    pub schema: Schema<'a>,
}

#[amqp_derive(descriptor(code = 0xc5620000_00000002))]
#[derive(Debug, Deserialize, Serialize)]
struct Schema<'a> {
    #[serde(borrow)]
    types: amqp::List<TypeNotation<'a>>,
}

#[amqp_derive(descriptor(code = 0xc5620000_00000003))]
#[derive(Debug, Deserialize, Serialize)]
struct Descriptor<'a> {
    #[serde(borrow)]
    name: Option<amqp::Symbol<'a>>,
    code: Option<u64>,
}

#[amqp_derive(descriptor(code = 0xc5620000_00000004))]
#[derive(Debug, Deserialize, Serialize)]
struct Field<'a> {
    name: &'a str,
    #[serde(rename = "type")]
    ty: &'a str,
    #[serde(borrow)]
    requires: amqp::List<&'a str>,
    default: Option<&'a str>,
    label: Option<&'a str>,
    mandatory: bool,
    multiple: bool,
}

#[amqp_derive(descriptor(name = "net.corda:1BLPJgNvsxdvPcbrIQd87g=="))]
#[derive(Debug, Deserialize, Serialize)]
struct ObjectList(amqp::List<()>);

#[amqp_derive]
#[derive(Debug, Serialize)]
enum TypeNotation<'a> {
    CompositeType(CompositeType<'a>),
    RestrictedType(RestrictedType<'a>),
}

#[amqp_derive(descriptor(code = 0xc5620000_00000005))]
#[derive(Debug, Deserialize, Serialize)]
struct CompositeType<'a> {
    name: &'a str,
    label: Option<&'a str>,
    provides: amqp::List<&'a str>,
    descriptor: Descriptor<'a>,
    fields: amqp::List<Field<'a>>,
}

#[amqp_derive(descriptor(code = 0xc5620000_00000006))]
#[derive(Debug, Deserialize, Serialize)]
struct RestrictedType<'a> {
    name: &'a str,
    label: Option<&'a str>,
    provides: amqp::List<&'a str>,
    source: &'a str,
    descriptor: Descriptor<'a>,
    choices: amqp::List<Choice<'a>>,
}

#[amqp_derive(descriptor(code = 0xc5620000_00000007))]
#[derive(Debug, Deserialize, Serialize)]
struct Choice<'a> {
    name: &'a str,
    value: &'a str,
}

#[amqp_derive(descriptor(code = 0xc5620000_00000009))]
#[derive(Debug, Deserialize, Serialize)]
struct TransformsSchema {}

const CORDA_MAGIC: &[u8; 7] = b"corda\x01\x00";
