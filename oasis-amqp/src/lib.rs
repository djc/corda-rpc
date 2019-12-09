use std::array::TryFromSliceError;
use std::convert::TryInto;
use std::fmt;
use std::io::Cursor;
use std::str;

use bytes::Buf;
use err_derive::Error;
use oasis_amqp_derive::amqp;
use serde::{self, Deserialize, Serialize};
use serde_bytes::Bytes;

mod de;
mod ser;

#[derive(Debug)]
pub enum Frame<'a> {
    Amqp(AmqpFrame),
    Sasl(sasl::Frame<'a>),
}

impl<'a> Frame<'a> {
    pub fn decode(buf: &'a [u8]) -> Result<Self, Error> {
        let mut cur = Cursor::new(buf);
        let len = cur.get_u32_be();
        if cur.remaining() < (len - 4).try_into().unwrap() {
            return Err(Error::InvalidData);
        }

        let doff = cur.get_u8();
        if doff < 2 {
            return Err(Error::InvalidData);
        }

        let ty = cur.get_u8();
        let nested = &buf[cur.position() as usize..];
        match ty {
            0x00 => Ok(Frame::Amqp(AmqpFrame::decode(nested, doff)?)),
            0x01 => Ok(Frame::Sasl(de::deserialize(&nested[2..])?)),
            _ => Err(Error::InvalidData),
        }
    }

    pub fn to_vec(&self) -> Vec<u8> {
        let mut buf = vec![0; 8];
        let ty = match self {
            Frame::Amqp(f) => {
                ser::into_bytes(f, &mut buf).unwrap();
                0x00
            }
            Frame::Sasl(f) => {
                ser::into_bytes(f, &mut buf).unwrap();
                0x01
            }
        };

        buf[4] = 2;
        buf[5] = ty;
        let len = buf.len() as u32;
        (&mut buf[..4]).copy_from_slice(&len.to_be_bytes()[..]);
        buf
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AmqpFrame {}

impl AmqpFrame {
    fn decode(_: &[u8], _: u8) -> Result<Self, Error> {
        unimplemented!()
    }
}

#[amqp]
#[derive(Debug, Serialize)]
pub enum Performative<'a> {
    Open(Open<'a>),
    Begin(Begin),
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

#[amqp(descriptor("amqp:open:begin", 0x00000000_00000011))]
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename = "amqp:open:begin")]
pub struct Begin {
    pub remote_channel: Option<u16>,
    pub next_outgoing_id: u16,
    pub incoming_window: u32,
    pub outgoing_window: u32,
    pub handle_max: u32,
    /*pub offered_capabilities: Option<Vec<&'a str>>,
    pub desired_capabilities: Option<Vec<&'a str>>,
    pub properties: Option<Vec<(&'a [u8], &'a [u8])>>,*/
}

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
