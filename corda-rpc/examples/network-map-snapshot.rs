use std::collections::HashMap;
use std::convert::TryFrom;
use std::time::SystemTime;

use oasis_amqp::{amqp, ser, Client};
use rand::{self, Rng};
use serde_bytes::{ByteBuf, Bytes};
use tokio;
use uuid::Uuid;

use corda_rpc::{
    Descriptor, Envelope, ObjectList, RestrictedType, Schema, SectionId, TypeNotation, CORDA_MAGIC,
};

#[tokio::main]
async fn main() {
    let mut client = Client::connect("localhost:10006").await.unwrap();
    client.login("vxdir", "vxdir").await.unwrap();
    client.open("vxweb").await.unwrap();
    client.begin().await.unwrap();

    client
        .attach(amqp::Attach {
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
        })
        .await
        .unwrap();

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

    client
        .transfer(
            amqp::Transfer {
                handle: 0,
                delivery_id: Some(0),
                delivery_tag: Some(ByteBuf::from(delivery_tag.as_bytes().to_vec())),
                message_format: Some(0),
                ..Default::default()
            },
            amqp::Message {
                properties: Some(amqp::Properties {
                    message_id: Some(message_id.clone().into()),
                    reply_to: Some("vx-web-sender".into()),
                    user_id: Some(Bytes::new(b"vxdir")),
                    ..Default::default()
                }),
                application_properties: Some(amqp::ApplicationProperties(properties)),
                body: Some(amqp::Body::Data(amqp::Data(ByteBuf::from(body)))),
                ..Default::default()
            },
        )
        .await
        .unwrap();

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
}
