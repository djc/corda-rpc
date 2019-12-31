use std::array::TryFromSliceError;
use std::convert::TryInto;
use std::{fmt, io, mem, str};

use bytes::{self, BufMut, BytesMut};
use err_derive::Error;
use serde;
use tokio_util::codec::{Decoder, Encoder};

pub mod amqp;
pub mod de;
pub mod sasl;
pub mod ser;

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
        let buf = item.to_vec().unwrap();
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
    Amqp(amqp::Frame<'a>),
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
            0x00 => Ok(Frame::Amqp(amqp::Frame::decode(doff, &buf[2..])?)),
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

    pub fn to_vec(&self) -> Result<Vec<u8>, Error> {
        let mut buf = vec![0; 8];

        match self {
            Frame::Amqp(f) => {
                buf[5] = 0x00;
                ser::into_bytes(&f.performative, &mut buf)?;
                if let Some(msg) = &f.message {
                    if let Some(header) = &msg.header {
                        ser::into_bytes(header, &mut buf)?;
                    }
                    if let Some(da) = &msg.delivery_annotations {
                        ser::into_bytes(da, &mut buf)?;
                    }
                    if let Some(ma) = &msg.message_annotations {
                        ser::into_bytes(ma, &mut buf)?;
                    }
                    if let Some(props) = &msg.properties {
                        ser::into_bytes(props, &mut buf)?;
                    }
                    if let Some(ap) = &msg.application_properties {
                        ser::into_bytes(ap, &mut buf)?;
                    }
                    ser::into_bytes(&msg.body, &mut buf)?;
                    if let Some(footer) = &msg.footer {
                        ser::into_bytes(footer, &mut buf)?;
                    }
                }
                (&mut buf[6..8]).copy_from_slice(&f.channel.to_be_bytes()[..]);
            }
            Frame::Header(p) => {
                buf.copy_from_slice(p.header());
                return Ok(buf);
            }
            Frame::Sasl(f) => {
                buf[5] = 0x01;
                ser::into_bytes(f, &mut buf).unwrap();
            }
        }

        buf[4] = 2; // doff
        let len = buf.len() as u32;
        (&mut buf[..4]).copy_from_slice(&len.to_be_bytes()[..]);
        Ok(buf)
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

#[derive(Debug)]
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

pub trait Described {
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
