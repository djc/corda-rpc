use std::collections::HashMap;
use std::convert::TryFrom;
use std::time::SystemTime;

use oasis_amqp::{amqp, Client};
use rand::{self, Rng};
use serde_bytes::{ByteBuf, Bytes};
use tokio;
use uuid::Uuid;

use corda_rpc::{Descriptor, Envelope, ObjectList, RestrictedType, Schema, TypeNotation};

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

    let rcv_queue_name = format!(
        "rpc.client.vxdir.{}",
        rand::thread_rng().gen::<u64>() & 0xefffffff_ffffffff,
    );

    client
        .attach(amqp::Attach {
            name: rcv_queue_name.clone(),
            handle: 1,
            role: amqp::Role::Receiver,
            snd_settle_mode: None,
            rcv_settle_mode: None,
            source: Some(amqp::Source {
                address: Some(rcv_queue_name.clone()),
                ..Default::default()
            }),
            target: Some(amqp::Target {
                address: Some("vx-web".into()),
                ..Default::default()
            }),
            unsettled: None,
            incomplete_unsettled: None,
            initial_delivery_count: None,
            max_message_size: None,
            offered_capabilities: None,
            desired_capabilities: None,
            properties: None,
        })
        .await
        .unwrap();

    client
        .flow(amqp::Flow {
            next_incoming_id: Some(1),
            incoming_window: 2_147_483_647,
            next_outgoing_id: 1,
            outgoing_window: 2_147_483_647,
            handle: Some(1),
            delivery_count: Some(0),
            link_credit: Some(1000),
            available: None,
            drain: None,
            echo: None,
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

    let mut properties = HashMap::new();
    properties.insert("_AMQ_VALIDATED_USER", amqp::Any::Str("vxdir".into()));
    properties.insert("tag", amqp::Any::I32(0));
    properties.insert("method-name", amqp::Any::Str("networkMapSnapshot".into()));
    properties.insert("rpc-id", amqp::Any::Str(rpc_id.clone().into()));
    properties.insert("rpc-id-timestamp", amqp::Any::I64(timestamp));
    properties.insert("rpc-session-id", amqp::Any::Str(rpc_session_id.into()));
    properties.insert("rpc-session-id-timestamp", amqp::Any::I64(timestamp));
    properties.insert("deduplication-sequence-number", amqp::Any::I64(0));

    let mut body = vec![];
    Envelope {
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
        transforms_schema: None,
    }
    .encode(&mut body)
    .unwrap();

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
                    message_id: Some(rpc_id.clone().into()),
                    reply_to: Some(rcv_queue_name.clone().into()),
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

    println!("waiting for response...");
    let next = client.next().await.unwrap();
    println!("read: {:#?}\n", next);
}
