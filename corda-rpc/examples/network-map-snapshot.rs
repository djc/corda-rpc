use std::collections::HashMap;
use std::convert::TryFrom;
use std::time::SystemTime;

use oasis_amqp::{amqp, proto::Frame, Client};
use rand::{self, Rng};
use serde_bytes::{ByteBuf, Bytes};
use structopt::StructOpt;
use tokio;
use uuid::Uuid;

use corda_rpc::{NetworkMapSnapshot, Rpc};

#[tokio::main]
async fn main() {
    let options = Options::from_args();
    let mut client = Client::connect(&options.address).await.unwrap();
    client
        .login(&options.user, &options.password)
        .await
        .unwrap();
    client.open(CONTAINER).await.unwrap();
    client.begin().await.unwrap();

    client
        .attach(amqp::Attach {
            name: "corda-rpc-sender".into(),
            handle: 0,
            role: amqp::Role::Sender,
            snd_settle_mode: None,
            rcv_settle_mode: None,
            source: Some(amqp::Source {
                address: Some(CONTAINER.into()),
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
        "rpc.client.{}.{}",
        options.user,
        rand::thread_rng().gen::<u64>() & 0xefffffff_ffffffff,
    );

    client
        .attach(amqp::Attach {
            name: &rcv_queue_name,
            handle: 1,
            role: amqp::Role::Receiver,
            snd_settle_mode: None,
            rcv_settle_mode: None,
            source: Some(amqp::Source {
                address: Some(&rcv_queue_name),
                ..Default::default()
            }),
            target: Some(amqp::Target {
                address: Some(CONTAINER.into()),
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

    let req = NetworkMapSnapshot;
    let now = SystemTime::now();
    let timestamp = now.duration_since(SystemTime::UNIX_EPOCH).unwrap();
    let timestamp = i64::try_from(timestamp.as_millis()).unwrap();

    let rpc_id = format!("{:x}", Uuid::new_v4().to_hyphenated());
    let rpc_session_id = format!("{:x}", Uuid::new_v4().to_hyphenated());
    let delivery_tag = Uuid::new_v4();

    let mut properties = HashMap::new();
    properties.insert("_AMQ_VALIDATED_USER", amqp::Any::Str(&options.user));
    properties.insert("tag", amqp::Any::I32(0));
    properties.insert("method-name", amqp::Any::Str(req.method()));
    properties.insert("rpc-id", amqp::Any::Str(&rpc_id));
    properties.insert("rpc-id-timestamp", amqp::Any::I64(timestamp));
    properties.insert("rpc-session-id", amqp::Any::Str(&rpc_session_id));
    properties.insert("rpc-session-id-timestamp", amqp::Any::I64(timestamp));
    properties.insert("deduplication-sequence-number", amqp::Any::I64(0));

    let mut body = vec![];
    req.request().encode(&mut body).unwrap();

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
                    user_id: Some(Bytes::new(options.user.as_bytes())),
                    ..Default::default()
                }),
                application_properties: Some(amqp::ApplicationProperties(properties)),
                body: Some(amqp::Body::Data(amqp::Data(&body))),
                ..Default::default()
            },
        )
        .await
        .unwrap();

    let frame = client.next().await.unwrap().unwrap();
    let message = match frame.frame() {
        Frame::Amqp(frame) => &frame.message,
        _ => unreachable!(),
    };

    let body = match message.as_ref().unwrap().body {
        Some(amqp::Body::Data(amqp::Data(data))) => data,
        Some(amqp::Body::Value(amqp::Value(amqp::Any::Bytes(data)))) => data,
        _ => unreachable!(),
    };

    println!("{:#?}", req.response(&body).unwrap());
}

#[derive(Debug, StructOpt)]
struct Options {
    #[structopt(short, long)]
    user: String,
    #[structopt(short, long)]
    password: String,
    address: String,
}

const CONTAINER: &str = "corda-rpc";
