use std::collections::HashMap;
use std::convert::TryFrom;
use std::time::SystemTime;

use oasis_amqp::{amqp, proto::BytesFrame};
use rand::{self, Rng};
use serde_bytes::Bytes;
use tokio::net::ToSocketAddrs;
use uuid::Uuid;

use crate::types::Rpc;

pub struct Client {
    inner: oasis_amqp::Client,
    user: String,
    container: String,
}

impl Client {
    pub async fn new<A: ToSocketAddrs>(
        address: A,
        user: String,
        password: &str,
        container: String,
    ) -> Result<Self, ()> {
        let mut inner = oasis_amqp::Client::connect(address).await.map_err(|_| ())?;
        inner.login(&user, &password).await?;
        inner.open(&container).await?;
        inner.begin().await?;

        let sender_name = format!("corda-rpc-{:x}", Uuid::new_v4().to_hyphenated());
        inner
            .attach(amqp::Attach {
                name: &sender_name,
                handle: 0,
                role: amqp::Role::Sender,
                snd_settle_mode: None,
                rcv_settle_mode: None,
                source: Some(amqp::Source {
                    address: Some(&container),
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
            })
            .await?;

        Ok(Self {
            inner,
            user,
            container,
        })
    }

    pub async fn call<'r, T: Rpc<'static>>(&mut self, rpc: &T) -> Result<BytesFrame, T::Error> {
        let rcv_queue_name = format!(
            "rpc.client.{}.{}",
            self.user,
            rand::thread_rng().gen::<u64>() & 0xefff_ffff_ffff_ffff,
        );

        self.inner
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
                    address: Some(&self.container),
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
            .await?;

        self.inner
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
            .await?;

        let now = SystemTime::now();
        let timestamp = now.duration_since(SystemTime::UNIX_EPOCH).unwrap();
        let timestamp = i64::try_from(timestamp.as_millis()).unwrap();

        let rpc_id = format!("{:x}", Uuid::new_v4().to_hyphenated());
        let rpc_session_id = format!("{:x}", Uuid::new_v4().to_hyphenated());
        let delivery_tag = Uuid::new_v4();

        let mut properties = HashMap::new();
        properties.insert("_AMQ_VALIDATED_USER", amqp::Any::Str(&self.user));
        properties.insert("tag", amqp::Any::I32(0));
        properties.insert("method-name", amqp::Any::Str(rpc.method()));
        properties.insert("rpc-id", amqp::Any::Str(&rpc_id));
        properties.insert("rpc-id-timestamp", amqp::Any::I64(timestamp));
        properties.insert("rpc-session-id", amqp::Any::Str(&rpc_session_id));
        properties.insert("rpc-session-id-timestamp", amqp::Any::I64(timestamp));
        properties.insert("deduplication-sequence-number", amqp::Any::I64(0));

        let mut body = vec![];
        rpc.request().encode(&mut body).unwrap();

        self.inner
            .transfer(
                amqp::Transfer {
                    handle: 0,
                    delivery_id: Some(0),
                    delivery_tag: Some(delivery_tag.as_bytes().to_vec()),
                    message_format: Some(0),
                    ..Default::default()
                },
                amqp::Message {
                    properties: Some(amqp::Properties {
                        message_id: Some(rpc_id.clone().into()),
                        reply_to: Some(rcv_queue_name.clone().into()),
                        user_id: Some(Bytes::new(self.user.as_bytes())),
                        ..Default::default()
                    }),
                    application_properties: Some(amqp::ApplicationProperties(properties)),
                    body: Some(amqp::Body::Data(amqp::Data(&body))),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        match self.inner.next().await {
            Some(Ok(frame)) => Ok(frame),
            _ => Err(().into()),
        }
    }
}
