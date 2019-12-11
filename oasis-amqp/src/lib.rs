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

#[derive(Default)]
pub struct Codec {
    sending: Option<Protocol>,
    receiving: Option<Protocol>,
}

impl Decoder for Codec {
    type Item = BytesFrame;
    type Error = Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if self.receiving.is_none() {
            let header = src.split_to(8);
            self.receiving = Some(Protocol::from_bytes(&header));
        }

        if src.len() < 4 {
            return Ok(None);
        }

        let len = u32::from_be_bytes((&src[..4]).try_into().unwrap()) as usize;
        println!("received {} bytes: {:?}", len, &src[..len]);
        if src.len() < len {
            return Ok(None);
        }

        let bytes = src.split_to(len).freeze();
        let frame = Frame::decode(&bytes[4..])?;
        if let Frame::Sasl(sasl::Frame::Outcome(_)) = frame {
            self.receiving = None;
        }

        let frame = unsafe { mem::transmute(frame) };
        Ok(Some(BytesFrame { bytes, frame }))
    }
}

impl Encoder for Codec {
    type Item = Frame<'static>;
    type Error = Error;

    fn encode(&mut self, item: Self::Item, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let proto = item.protocol();
        if self.sending != Some(proto) {
            dst.put(proto.header());
            self.sending = Some(proto);
        }

        let buf = item.to_vec();
        println!("writing {:?}", buf);
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
    Sasl(sasl::Frame<'a>),
}

impl<'a> Frame<'a> {
    pub fn decode(buf: &'a [u8]) -> Result<Self, Error> {
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
        buf[4] = 2; // doff
        match self {
            Frame::Amqp(f) => {
                buf[5] = 0x00;
                ser::into_bytes(&f.performative, &mut buf).unwrap();
                buf.extend_from_slice(f.body);
                (&mut buf[6..8]).copy_from_slice(&f.channel.to_be_bytes()[..]);
            }
            Frame::Sasl(f) => {
                buf[5] = 0x01;
                ser::into_bytes(f, &mut buf).unwrap();
            }
        }

        let len = buf.len() as u32;
        (&mut buf[..4]).copy_from_slice(&len.to_be_bytes()[..]);
        buf
    }

    fn protocol(&self) -> Protocol {
        match self {
            Frame::Sasl(_) => Protocol::Sasl,
            Frame::Amqp(_) => Protocol::Amqp,
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum Protocol {
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
    pub source: Option<&'a str>,
    pub target: Option<&'a str>,
    pub unsettled: Option<HashMap<&'a [u8], u32>>,
    pub incomplete_unsettled: Option<bool>,
    pub initial_delivery_count: Option<u32>,
    pub max_message_size: Option<u64>,
    pub offered_capabilities: Option<Vec<&'a str>>,
    pub desired_capabilities: Option<Vec<&'a str>>,
    pub properties: Option<Vec<(&'a [u8], &'a [u8])>>,
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

#[derive(Debug, Deserialize, Serialize)]
pub enum Role {
    Sender,
    Receiver,
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
