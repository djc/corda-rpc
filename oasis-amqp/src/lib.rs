use std::array::TryFromSliceError;
use std::collections::HashMap;
use std::convert::TryInto;
use std::{fmt, io, mem, str};

use bytes::{self, BufMut, BytesMut};
use err_derive::Error;
use oasis_amqp_derive::amqp;
use serde::{self, Deserialize, Serialize};
use serde_bytes::Bytes;
use tokio_util::codec::{Decoder, Encoder};

mod de;
mod ser;

pub struct Codec;

impl Decoder for Codec {
    type Item = BytesFrame;
    type Error = Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if src.len() < 4 {
            return Ok(None);
        }

        let bytes = if &src[..4] == b"AMQP" && src.len() >= 8 {
            src.split_to(8).freeze()
        } else {
            let len = u32::from_be_bytes((&src[..4]).try_into().unwrap()) as usize;
            println!("received {} bytes: {:?}", len, &src[..len]);
            if src.len() >= len {
                src.split_to(len).freeze().split_off(4)
            } else {
                return Ok(None);
            }
        };

        let frame = unsafe { mem::transmute(Frame::decode(&bytes)?) };
        Ok(Some(BytesFrame { bytes, frame }))
    }
}

impl Encoder for Codec {
    type Item = Frame<'static>;
    type Error = Error;

    fn encode(&mut self, item: Self::Item, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let buf = item.to_vec();
        println!("writing {} bytes: {:?}", buf.len(), buf);
        dst.put(&*buf);
        Ok(())
    }
}

pub struct BytesFrame {
    #[allow(dead_code)]
    bytes: bytes::Bytes,
    pub frame: Frame<'static>,
}

impl std::fmt::Debug for BytesFrame {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        self.frame.fmt(fmt)
    }
}

#[derive(Debug)]
pub enum Frame<'a> {
    Amqp(AmqpFrame<'a>),
    Header(Protocol),
    Sasl(sasl::Frame<'a>),
}

impl<'a> Frame<'a> {
    pub fn decode(buf: &'a [u8]) -> Result<Self, Error> {
        if &buf[..4] == b"AMQP" {
            return Ok(Frame::Header(Protocol::from_bytes(buf)));
        }

        let doff = buf[0];
        if doff < 2 {
            return Err(Error::InvalidData);
        }

        match buf[1] {
            0x00 => Ok(Frame::Amqp(AmqpFrame::decode(doff, &buf[2..])?)),
            0x01 => {
                assert_eq!(&buf[2..4], &[0, 0]);
                let (sasl, rest) = de::deserialize(&buf[4..])?;
                if !rest.is_empty() {
                    return Err(Error::TrailingCharacters);
                }
                Ok(Frame::Sasl(sasl))
            }
            _ => Err(Error::InvalidData),
        }
    }

    pub fn to_vec(&self) -> Vec<u8> {
        let mut buf = vec![0; 8];

        match self {
            Frame::Amqp(f) => {
                buf[5] = 0x00;
                ser::into_bytes(&f.performative, &mut buf).unwrap();
                buf.extend_from_slice(f.body);
                (&mut buf[6..8]).copy_from_slice(&f.channel.to_be_bytes()[..]);
            }
            Frame::Header(p) => {
                buf.copy_from_slice(p.header());
                return buf;
            }
            Frame::Sasl(f) => {
                buf[5] = 0x01;
                ser::into_bytes(f, &mut buf).unwrap();
            }
        }

        buf[4] = 2; // doff
        let len = buf.len() as u32;
        (&mut buf[..4]).copy_from_slice(&len.to_be_bytes()[..]);
        buf
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Protocol {
    Sasl,
    Amqp,
}

impl Protocol {
    fn from_bytes(bytes: &[u8]) -> Self {
        match bytes {
            SASL_PROTO_HEADER => Protocol::Sasl,
            AMQP_PROTO_HEADER => Protocol::Amqp,
            p => panic!("invalid protocol header {:?}", p),
        }
    }

    fn header(&self) -> &'static [u8] {
        match self {
            Protocol::Sasl => SASL_PROTO_HEADER,
            Protocol::Amqp => AMQP_PROTO_HEADER,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AmqpFrame<'a> {
    pub channel: u16,
    pub extended_header: Option<&'a [u8]>,
    pub performative: Performative<'a>,
    pub body: &'a [u8],
}

impl<'a> AmqpFrame<'a> {
    fn decode(doff: u8, buf: &'a [u8]) -> Result<Self, Error> {
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
            return Err(Error::TrailingCharacters);
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

#[derive(Debug, Deserialize, Serialize)]
pub enum ConnectionState {
    Start,
    HdrRcvd,
    HdrSent,
    HdrExch,
    OpenPipe,
    OcPipe,
    OpenRcvd,
    OpenSent,
    ClosePipe,
    Opened,
    CloseRcvd,
    CloseSent,
    Discarding,
    End,
}

pub struct Session {
    pub next_incoming_id: u32,
    pub incoming_window: u32,
    pub next_outgoing_id: u32,
    pub outgoing_window: u32,
    pub remote_incoming_window: u32,
    pub remote_outgoing_window: u32,
}

pub enum SessionState {
    Unmapped,
    BeginSent,
    BeginRcvd,
    Mapped,
    EndSent,
    EndRcvd,
    Discarding,
}

pub mod sasl {
    use super::*;

    #[amqp]
    #[derive(Debug, Serialize)]
    pub enum Frame<'a> {
        Mechanisms(Mechanisms),
        Init(Init<'a>),
        Outcome(Outcome<'a>),
    }

    #[amqp(descriptor("amqp:sasl-mechanisms:list", 0x00000000_00000040))]
    #[derive(Debug, Deserialize, Serialize)]
    #[serde(rename = "amqp:sasl-mechanisms:list")]
    pub struct Mechanisms {
        sasl_server_mechanisms: Vec<Mechanism>,
    }

    #[amqp(descriptor("amqp:sasl-init:list", 0x00000000_00000041))]
    #[derive(Debug, Deserialize, Serialize)]
    #[serde(rename = "amqp:sasl-init:list")]
    pub struct Init<'a> {
        pub mechanism: Mechanism,
        #[serde(borrow)]
        pub initial_response: Option<&'a Bytes>,
        pub hostname: Option<&'a str>,
    }

    #[derive(Debug, Deserialize, Serialize)]
    #[serde(rename_all = "SCREAMING_SNAKE_CASE")]
    pub enum Mechanism {
        Anonymous,
        Plain,
        ScramSha1,
    }

    #[amqp(descriptor("amqp:sasl-outcome:list", 0x00000000_00000044))]
    #[derive(Debug, Deserialize, Serialize)]
    #[serde(rename = "amqp:sasl-outcome:list")]
    pub struct Outcome<'a> {
        code: Code,
        #[serde(borrow)]
        additional_data: Option<&'a Bytes>,
    }

    #[derive(Debug, Deserialize, Serialize)]
    pub enum Code {
        Ok,
        Auth,
        Sys,
        SysPerm,
        SysTemp,
    }
}

trait Described {
    const NAME: &'static [u8];
    const ID: u64;
}

#[derive(Debug, Error)]
pub enum Error {
    #[error(display = "invalid data")]
    InvalidData,
    #[error(display = "syntax")]
    Syntax,
    #[error(display = "unexpected end")]
    UnexpectedEnd,
    #[error(display = "I/O error: {}", _0)]
    Io(#[source] io::Error),
    #[error(display = "deserialization failed: {}", _0)]
    Deserialization(String),
    #[error(display = "serialization failed: {}", _0)]
    Serialization(String),
    #[error(display = "buffer not empty after deserialization")]
    TrailingCharacters,
}

impl serde::de::Error for Error {
    fn custom<T>(msg: T) -> Self
    where
        T: fmt::Display,
    {
        Error::Deserialization(msg.to_string())
    }
}

impl serde::ser::Error for Error {
    fn custom<T>(msg: T) -> Self
    where
        T: fmt::Display,
    {
        Error::Serialization(msg.to_string())
    }
}

impl From<TryFromSliceError> for Error {
    fn from(e: TryFromSliceError) -> Self {
        Error::Deserialization(e.to_string())
    }
}

pub const MIN_MAX_FRAME_SIZE: usize = 512;
pub const AMQP_PROTO_HEADER: &[u8] = b"AMQP\x00\x01\x00\x00";
pub const SASL_PROTO_HEADER: &[u8] = b"AMQP\x03\x01\x00\x00";
