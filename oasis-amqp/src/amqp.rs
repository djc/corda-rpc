use std::borrow::Cow;
use std::collections::HashMap;
use std::convert::TryInto;

use oasis_amqp_derive::amqp;
use serde::{self, Deserialize, Serialize};
use serde_bytes::{ByteBuf, Bytes};

use crate::{de, Described};

#[derive(Debug, Deserialize, Serialize)]
pub struct Frame<'a> {
    pub channel: u16,
    #[serde(borrow)]
    pub extended_header: Option<&'a Bytes>,
    pub performative: Performative<'a>,
    pub message: Option<Message<'a>>,
}

impl<'a> Frame<'a> {
    pub(crate) fn decode(doff: u8, buf: &'a [u8]) -> Result<Self, crate::Error> {
        let (channel, buf) = buf.split_at(2);
        let channel = u16::from_be_bytes(channel.try_into().unwrap());

        let (extended, buf) = buf.split_at((doff - 2) as usize);
        let extended_header = if !extended.is_empty() {
            Some(Bytes::new(extended))
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
            message: None,
        })
    }
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Message<'a> {
    pub header: Option<Header>,
    #[serde(borrow)]
    pub delivery_annotations: Option<DeliveryAnnotations<'a>>,
    pub message_annotations: Option<MessageAnnotations<'a>>,
    pub properties: Option<Properties<'a>>,
    pub application_properties: Option<ApplicationProperties<'a>>,
    pub body: Option<Body<'a>>,
    pub footer: Option<Footer<'a>>,
}

#[amqp(descriptor("amqp:header:list", 0x00000000_00000070))]
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename = "amqp:header:list")]
pub struct Header {
    pub durable: Option<bool>,
    pub priority: Option<u8>,
    pub ttl: Option<u32>, // ms
    pub first_acquirer: Option<bool>,
    pub delivery_count: Option<u32>,
}

#[amqp(descriptor("amqp:delivery-annotations:map", 0x00000000_00000071))]
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename = "amqp:delivery-annotations:map")]
pub struct DeliveryAnnotations<'a>(#[serde(borrow)] pub HashMap<&'a str, &'a str>);

#[amqp(descriptor("amqp:message-annotations:map", 0x00000000_00000072))]
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename = "amqp:message-annotations:map")]
pub struct MessageAnnotations<'a>(#[serde(borrow)] pub HashMap<&'a str, &'a str>);

#[amqp(descriptor("amqp:properties:list", 0x00000000_00000073))]
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename = "amqp:properties:list")]
pub struct Properties<'a> {
    pub message_id: Option<Cow<'a, str>>,
    pub user_id: Option<&'a Bytes>,
    pub to: Option<&'a str>,
    pub subject: Option<&'a str>,
    pub reply_to: Option<&'a str>,
    pub correlation_id: Option<&'a str>,
    pub content_type: Option<&'a str>,
    pub content_encoding: Option<&'a str>,
    pub absolute_expiry_time: Option<u32>,
    pub creation_time: Option<u32>,
    pub group_id: Option<&'a str>,
    pub group_sequence: Option<u32>,
    pub reply_to_group_id: Option<&'a str>,
}

#[amqp(descriptor("amqp:application-properties:map", 0x00000000_00000074))]
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename = "amqp:application-properties:map")]
pub struct ApplicationProperties<'a>(#[serde(borrow)] pub HashMap<&'a str, Any<'a>>);

#[amqp]
#[derive(Debug, Serialize)]
pub enum Body<'a> {
    #[serde(borrow)]
    Data(Data<'a>),
    Sequence(Sequence),
    Value(Value),
}

#[amqp(descriptor("amqp:data:binary", 0x00000000_00000075))]
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename = "amqp:data:binary")]
pub struct Data<'a>(#[serde(borrow)] pub &'a Bytes);

#[amqp(descriptor("amqp:amqp-sequence:list", 0x00000000_00000076))]
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename = "amqp:amqp-sequence:list")]
pub struct Sequence {}

#[amqp(descriptor("amqp:value:*", 0x00000000_00000077))]
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename = "amqp:value:*")]
pub struct Value {}

#[amqp(descriptor("amqp:footer:map", 0x00000000_00000078))]
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename = "amqp:footer:map")]
pub struct Footer<'a>(#[serde(borrow)] pub HashMap<&'a str, &'a str>);

#[amqp]
#[derive(Debug, Serialize)]
pub enum Performative<'a> {
    Open(Open<'a>),
    Begin(Begin<'a>),
    Attach(Attach<'a>),
    Flow(Flow<'a>),
    Transfer(Transfer),
    Disposition(Disposition),
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
    pub unsettled: Option<HashMap<&'a Bytes, u32>>,
    pub incomplete_unsettled: Option<bool>,
    pub initial_delivery_count: Option<u32>,
    pub max_message_size: Option<u64>,
    pub offered_capabilities: Option<Vec<&'a str>>,
    pub desired_capabilities: Option<Vec<&'a str>>,
    pub properties: Option<Vec<(&'a Bytes, &'a Bytes)>>,
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
    pub properties: Option<Vec<(&'a Bytes, &'a Bytes)>>,
}

#[amqp(descriptor("amqp:transfer:list", 0x00000000_00000014))]
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename = "amqp:transfer:list")]
pub struct Transfer {
    pub handle: u32,
    pub delivery_id: Option<u32>,
    pub delivery_tag: Option<ByteBuf>,
    pub message_format: Option<u32>,
    pub settled: Option<bool>,
    pub more: Option<bool>,
    pub rcv_settle_mode: Option<ReceiverSettleMode>,
    pub state: Option<DeliveryState>,
    pub resume: Option<bool>,
    pub aborted: Option<bool>,
    pub batchable: Option<bool>,
}

#[amqp(descriptor("amqp:disposition:list", 0x00000000_00000015))]
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename = "amqp:disposition:list")]
pub struct Disposition {
    pub role: Role,
    pub first: u32,
    pub last: Option<u32>,
    settled: Option<bool>,
    state: Option<DeliveryState>,
    batchable: Option<bool>,
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
    info: Option<Vec<(&'a Bytes, &'a Bytes)>>,
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

#[amqp]
#[derive(Debug, Serialize)]
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

#[amqp]
#[derive(Debug, Serialize)]
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

#[derive(Debug, Deserialize, Serialize)]
pub enum Any<'a> {
    None,
    Bool(bool),
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    F32(f32),
    F64(f64),
    Bytes(&'a [u8]),
    Symbol(Cow<'a, str>),
    Str(Cow<'a, str>),
}
