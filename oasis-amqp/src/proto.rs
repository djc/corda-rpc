use std::convert::TryInto;
use std::{mem, str};

use bytes::{self, BufMut, BytesMut};
use futures::{sink::SinkExt, stream::StreamExt};
use serde_bytes::Bytes;
use tokio::net::{TcpStream, ToSocketAddrs};
use tokio_util::codec::{Decoder, Encoder, Framed};

use super::{amqp, de, sasl, ser, Error};

pub struct Client {
    transport: tokio_util::codec::Framed<TcpStream, Codec>,
}

impl Client {
    pub async fn connect<A: ToSocketAddrs>(addr: A) -> Result<Self, ()> {
        let stream = TcpStream::connect(addr).await.map_err(|_| ())?;
        Ok(Self {
            transport: Framed::new(stream, Codec),
        })
    }

    /// Login with the given username and password
    ///
    /// Currently this only supports SASL PLAIN login.
    pub async fn login(&mut self, user: &str, password: &str) -> Result<(), ()> {
        self.transport
            .send(&Frame::Header(Protocol::Sasl))
            .await
            .map_err(|_| ())?;
        let _header = self.transport.next().await.ok_or(()).map_err(|_| ())?;
        let _mechanisms = self.transport.next().await.ok_or(()).map_err(|_| ())?;

        let mut response = vec![0u8];
        response.extend_from_slice(user.as_bytes());
        response.push(0);
        response.extend_from_slice(password.as_bytes());

        let init = Frame::Sasl(sasl::Frame::Init(sasl::Init {
            mechanism: sasl::Mechanism::Plain,
            initial_response: Some(Bytes::new(&response)),
            hostname: None,
        }));

        self.transport.send(&init).await.map_err(|_| ())?;
        let _outcome = self.transport.next().await.ok_or(()).map_err(|_| ())?;
        let _header = self.transport.next().await.ok_or(()).map_err(|_| ())?;
        self.transport
            .send(&Frame::Header(Protocol::Amqp))
            .await
            .map_err(|_| ())
    }

    pub async fn open(&mut self, container_id: &str) -> Result<(), ()> {
        let open = Frame::Amqp(amqp::Frame {
            channel: 0,
            extended_header: None,
            performative: amqp::Performative::Open(amqp::Open {
                container_id,
                ..Default::default()
            }),
            message: None,
        });

        self.transport.send(&open).await.map_err(|_| ())?;
        let _opened = self.transport.next().await.ok_or(()).map_err(|_| ())?;
        Ok(())
    }

    pub async fn begin(&mut self) -> Result<(), ()> {
        let begin = Frame::Amqp(amqp::Frame {
            channel: 0,
            extended_header: None,
            performative: amqp::Performative::Begin(amqp::Begin {
                remote_channel: None,
                next_outgoing_id: 1,
                incoming_window: 8,
                outgoing_window: 8,
                ..Default::default()
            }),
            message: None,
        });

        self.transport.send(&begin).await.map_err(|_| ())?;
        let _begun = self.transport.next().await.ok_or(()).map_err(|_| ())?;
        Ok(())
    }

    pub async fn attach(&mut self, attach: amqp::Attach<'_>) -> Result<(), ()> {
        let is_sender = matches!(attach.role, amqp::Role::Sender);
        let attach = Frame::Amqp(amqp::Frame {
            channel: 0,
            extended_header: None,
            performative: amqp::Performative::Attach(attach),
            message: None,
        });

        self.transport.send(&attach).await.map_err(|_| ())?;
        let _attached = self.transport.next().await.ok_or(()).map_err(|_| ())?;
        if is_sender {
            let _flow = self.transport.next().await.ok_or(()).map_err(|_| ())?;
        }

        Ok(())
    }

    pub async fn flow(&mut self, flow: amqp::Flow<'_>) -> Result<(), ()> {
        let flow = Frame::Amqp(amqp::Frame {
            channel: 0,
            extended_header: None,
            performative: amqp::Performative::Flow(flow),
            message: None,
        });

        self.transport.send(&flow).await.map_err(|_| ())?;
        Ok(())
    }

    pub async fn transfer(
        &mut self,
        transfer: amqp::Transfer,
        message: amqp::Message<'_>,
    ) -> Result<(), ()> {
        let transfer = Frame::Amqp(amqp::Frame {
            channel: 0,
            extended_header: None,
            performative: amqp::Performative::Transfer(transfer),
            message: Some(message),
        });

        self.transport.send(&transfer).await.map_err(|_| ())?;
        let _transferred = self.transport.next().await.ok_or(()).map_err(|_| ())?;
        Ok(())
    }

    #[allow(clippy::should_implement_trait)]
    pub async fn next(&mut self) -> Option<Result<BytesFrame, Error>> {
        self.transport.next().await
    }
}

pub struct Codec;

impl Decoder for Codec {
    type Item = BytesFrame;
    type Error = Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if src.len() < 4 {
            return Ok(None);
        }

        let length_or_proto_tag = &src[..4];
        let bytes = if length_or_proto_tag == b"AMQP" && src.len() >= PROTO_HEADER_LENGTH {
            src.split_to(PROTO_HEADER_LENGTH).freeze()
        } else {
            let len = u32::from_be_bytes((length_or_proto_tag).try_into().unwrap()) as usize;
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

impl Encoder<&Frame<'_>> for Codec {
    type Error = Error;

    fn encode(&mut self, item: &Frame<'_>, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let buf = item.to_vec().unwrap();
        dst.put(&*buf);
        Ok(())
    }
}

pub struct BytesFrame {
    #[allow(dead_code)]
    bytes: bytes::Bytes,
    frame: Frame<'static>,
}

impl BytesFrame {
    pub fn frame<'a>(&'a self) -> &'a Frame<'a> {
        &self.frame
    }
}

impl std::fmt::Debug for BytesFrame {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        self.frame.fmt(fmt)
    }
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, PartialEq)]
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

        let result = match buf[1] {
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
        };

        if result.is_err() {
            println!("failed to decode: {:?}", buf);
        }
        result
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

    fn header(self) -> &'static [u8] {
        match self {
            Protocol::Sasl => SASL_PROTO_HEADER,
            Protocol::Amqp => AMQP_PROTO_HEADER,
        }
    }
}

/*

#[derive(Debug)]
enum ConnectionState {
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

struct Session {
    pub next_incoming_id: u32,
    pub incoming_window: u32,
    pub next_outgoing_id: u32,
    pub outgoing_window: u32,
    pub remote_incoming_window: u32,
    pub remote_outgoing_window: u32,
}

enum SessionState {
    Unmapped,
    BeginSent,
    BeginRcvd,
    Mapped,
    EndSent,
    EndRcvd,
    Discarding,
}

*/

pub const AMQP_PROTO_HEADER: &[u8] = b"AMQP\x00\x01\x00\x00";
pub const SASL_PROTO_HEADER: &[u8] = b"AMQP\x03\x01\x00\x00";
pub const PROTO_HEADER_LENGTH: usize = 8;
