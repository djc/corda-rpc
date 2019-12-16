use std::collections::HashMap;
use std::convert::TryInto;

use oasis_amqp_derive::amqp;
use serde::{self, Deserialize, Serialize};

use crate::{de, Described};

#[derive(Debug, Deserialize, Serialize)]
pub struct Frame<'a> {
    pub channel: u16,
    pub extended_header: Option<&'a [u8]>,
    pub performative: Performative<'a>,
    pub body: &'a [u8],
}

impl<'a> Frame<'a> {
    pub(crate) fn decode(doff: u8, buf: &'a [u8]) -> Result<Self, crate::Error> {
        let (channel, buf) = buf.split_at(2);
        let channel = u16::from_be_bytes(channel.try_into().unwrap());

        let (extended, buf) = buf.split_at((doff - 2) as usize);
        let extended_header = if !extended.is_empty() {
            Some(extended)
        } else {
            None
        };

        let (performative, buf) = de::deserialize(buf)?;
        if !buf.is_empty() {
            return Err(crate::Error::TrailingCharacters);
        }

        Ok(Self {
            channel,
            extended_header,
            performative,
            body: &[],
        })
    }
}

#[amqp]
#[derive(Debug, Serialize)]
pub enum Performative<'a> {
    Open(Open<'a>),
    Begin(Begin<'a>),
    Attach(Attach<'a>),
    Transfer(Transfer<'a>),
    Flow(Flow<'a>),
    Detach(Detach<'a>),
    Close(Close<'a>),
}

#[amqp(descriptor("amqp:open:list", 0x00000000_00000010))]
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename = "amqp:open:list")]
pub struct Open<'a> {
    pub container_id: &'a str,
    pub hostname: Option<&'a str>,
    pub max_frame_size: Option<u32>,
    pub channel_max: Option<u16>,
    pub idle_timeout: Option<u32>,
    pub outgoing_locales: Option<Vec<&'a str>>,
    pub incoming_locales: Option<Vec<&'a str>>,
    pub offered_capabilities: Option<Vec<&'a str>>,
    pub properties: Option<Vec<(&'a [u8], &'a [u8])>>,
}

#[amqp(descriptor("amqp:begin:list", 0x00000000_00000011))]
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename = "amqp:begin:list")]
pub struct Begin<'a> {
    pub remote_channel: Option<u16>,
    pub next_outgoing_id: u32,
    pub incoming_window: u32,
    pub outgoing_window: u32,
    pub handle_max: Option<u32>,
    #[serde(borrow)]
    pub offered_capabilities: Option<Vec<&'a str>>,
    pub desired_capabilities: Option<Vec<&'a str>>,
    pub properties: Option<Vec<(&'a [u8], &'a [u8])>>,
}

#[amqp(descriptor("amqp:attach:list", 0x00000000_00000012))]
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename = "amqp:attach:list")]
pub struct Attach<'a> {
    #[serde(borrow)]
    pub name: &'a str,
    pub handle: u32,
    pub role: Role,
    pub snd_settle_mode: Option<SenderSettleMode>,
    pub rcv_settle_mode: Option<ReceiverSettleMode>,
    pub source: Option<Source<'a>>,
    pub target: Option<Target<'a>>,
    pub unsettled: Option<HashMap<&'a [u8], u32>>,
    pub incomplete_unsettled: Option<bool>,
    pub initial_delivery_count: Option<u32>,
    pub max_message_size: Option<u64>,
    pub offered_capabilities: Option<Vec<&'a str>>,
    pub desired_capabilities: Option<Vec<&'a str>>,
    pub properties: Option<Vec<(&'a [u8], &'a [u8])>>,
}

#[amqp(descriptor("amqp:flow:list", 0x00000000_00000013))]
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename = "amqp:flow:list")]
pub struct Flow<'a> {
    pub next_incoming_id: Option<u32>,
    pub incoming_window: u32,
    pub next_outgoing_id: u32,
    pub outgoing_window: u32,
    pub handle: Option<u32>,
    pub delivery_count: Option<u32>,
    pub link_credit: Option<u32>,
    pub available: Option<u32>,
    pub drain: Option<bool>,
    pub echo: Option<bool>,
    #[serde(borrow)]
    pub properties: Option<Vec<(&'a [u8], &'a [u8])>>,
}

#[amqp(descriptor("amqp:transfer:list", 0x00000000_00000014))]
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename = "amqp:transfer:list")]
pub struct Transfer<'a> {
    pub handle: u32,
    pub delivery_id: Option<u32>,
    pub delivery_tag: Option<&'a [u8]>,
    pub message_format: Option<u32>,
    pub settled: Option<bool>,
    pub more: Option<bool>,
    pub rcv_settle_mode: Option<ReceiverSettleMode>,
    pub state: Option<DeliveryState>,
    pub resume: Option<bool>,
    pub aborted: Option<bool>,
    pub batchable: Option<bool>,
}

#[amqp(descriptor("amqp:detach:list", 0x00000000_00000016))]
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename = "amqp:detach:list")]
pub struct Detach<'a> {
    pub handle: u32,
    pub closed: Option<bool>,
    #[serde(borrow)]
    pub error: Option<AmqpError<'a>>,
}

#[amqp(descriptor("amqp:close:list", 0x00000000_00000018))]
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename = "amqp:close:list")]
pub struct Close<'a> {
    #[serde(borrow)]
    pub error: Option<AmqpError<'a>>,
}

#[amqp(descriptor("amqp:error:list", 0x00000000_0000001d))]
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename = "amqp:error:list")]
pub struct AmqpError<'a> {
    #[serde(borrow)]
    condition: &'a str,
    description: &'a str,
    info: Option<Vec<(&'a [u8], &'a [u8])>>,
}

#[derive(Debug, Deserialize)]
pub enum Role {
    Sender,
    Receiver,
}

impl Serialize for Role {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_bool(match self {
            Role::Sender => false,
            Role::Receiver => true,
        })
    }
}

#[amqp(descriptor("amqp:source:list", 0x00000000_00000028))]
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename = "amqp:source:list")]
pub struct Source<'a> {
    pub address: Option<&'a str>,
    pub durable: Option<u32>,
    pub expiry_policy: Option<ExpiryPolicy>,
    pub timeout: Option<u32>,
    pub dynamic: Option<bool>,
    pub dynamic_node_properties: Option<Vec<(&'a str, &'a str)>>,
    pub distribution_mode: Option<DistributionMode>,
    pub filter: Option<Vec<(&'a str, &'a str)>>,
    pub default_outcome: Option<Outcome>,
    pub outcomes: Option<Vec<&'a str>>,
    pub capabilities: Option<Vec<&'a str>>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DistributionMode {
    Move,
    Copy,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DeliveryState {
    Received(Received),
    Accepted(Accepted),
    Rejected(Rejected),
    Released(Released),
    Modified(Modified),
    Declared(Declared),
    TransactionalState(TransactionalState),
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Outcome {
    Received(Received),
    Accepted(Accepted),
    Rejected(Rejected),
    Released(Released),
    Modified(Modified),
    Declared(Declared),
}

#[amqp(descriptor("amqp:received:list", 0x00000000_00000023))]
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename = "amqp:received:list")]
pub struct Received {}

#[amqp(descriptor("amqp:accepted:list", 0x00000000_00000024))]
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename = "amqp:accepted:list")]
pub struct Accepted {}

#[amqp(descriptor("amqp:rejected:list", 0x00000000_00000025))]
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename = "amqp:rejected:list")]
pub struct Rejected {}

#[amqp(descriptor("amqp:released:list", 0x00000000_00000026))]
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename = "amqp:released:list")]
pub struct Released {}

#[amqp(descriptor("amqp:modified:list", 0x00000000_00000027))]
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename = "amqp:modified:list")]
pub struct Modified {}

#[amqp(descriptor("amqp:declared:list", 0x00000000_00000033))]
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename = "amqp:declared:list")]
pub struct Declared {}

#[amqp(descriptor("amqp:transactional-state:list", 0x00000000_00000034))]
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename = "amqp:transactional-state:list")]
pub struct TransactionalState {}

#[amqp(descriptor("amqp:target:list", 0x00000000_00000029))]
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename = "amqp:target:list")]
pub struct Target<'a> {
    pub address: Option<&'a str>,
    pub durable: Option<u32>,
    pub expiry_policy: Option<ExpiryPolicy>,
    pub timeout: Option<u32>,
    pub dynamic: Option<bool>,
    pub dynamic_node_properties: Option<Vec<(&'a str, &'a str)>>,
    pub capabilities: Option<Vec<&'a str>>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExpiryPolicy {
    LinkDetach,
    SessionEnd,
    ConnectionClose,
    Never,
}

#[derive(Debug, Deserialize, Serialize)]
pub enum SenderSettleMode {
    Unsettled,
    Settled,
    Mixed,
}

#[derive(Debug, Deserialize, Serialize)]
pub enum ReceiverSettleMode {
    First,
    Second,
}
