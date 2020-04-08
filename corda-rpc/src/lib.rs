use std::collections::HashMap;
use std::convert::TryFrom;
use std::fmt;
use std::marker::PhantomData;
use std::time::SystemTime;

use oasis_amqp::{amqp, de, proto::BytesFrame, ser, Described, Error};
use oasis_amqp_macros::amqp as amqp_derive;
use rand::{self, Rng};
use serde::{Deserialize, Serialize};
use serde_bytes::{ByteBuf, Bytes};
use tokio::net::ToSocketAddrs;
use uuid::Uuid;

mod network_map_snapshot;
pub use network_map_snapshot::{NetworkMapSnapshot, NodeInfo};

#[cfg(test)]
mod tests;

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
            rand::thread_rng().gen::<u64>() & 0xefffffff_ffffffff,
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
                    delivery_tag: Some(ByteBuf::from(delivery_tag.as_bytes().to_vec())),
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

pub trait Rpc<'r> {
    type Arguments: Serialize;
    type OkResult: 'r;
    type Error: From<()> + 'r;

    fn method(&self) -> &'static str;

    fn request(&self) -> Envelope<Self::Arguments>;

    fn response(&self, response: &'r BytesFrame) -> Result<Self::OkResult, Self::Error>;
}

#[derive(Debug, PartialEq, Serialize)]
pub enum Try<T, E> {
    Success(Success<T>),
    Failure(Failure<E>),
}

impl<'de, T, E> serde::Deserialize<'de> for Try<T, E>
where
    T: Deserialize<'de>,
    E: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        enum Field<T, E> {
            F0(PhantomData<T>),
            F1(PhantomData<E>),
        }

        struct FieldVisitor<T, E> {
            t1: PhantomData<T>,
            t2: PhantomData<E>,
        }

        impl<T, E> Default for FieldVisitor<T, E> {
            fn default() -> Self {
                Self {
                    t1: PhantomData::default(),
                    t2: PhantomData::default(),
                }
            }
        }

        impl<'de, T, E> serde::de::Visitor<'de> for FieldVisitor<T, E> {
            type Value = Field<T, E>;
            fn expecting(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
                fmt::Formatter::write_str(fmt, "variant identifier")
            }
            fn visit_bytes<__E>(self, value: &[u8]) -> Result<Self::Value, __E>
            where
                __E: serde::de::Error,
            {
                match Some(value) {
                    Success::<T>::NAME => Ok(Field::F0(PhantomData::default())),
                    Failure::<E>::NAME => Ok(Field::F1(PhantomData::default())),
                    _ => {
                        let value = &serde::export::from_utf8_lossy(value);
                        Err(serde::de::Error::unknown_variant(value, VARIANTS))
                    }
                }
            }
        }

        impl<'de, T, E> serde::Deserialize<'de> for Field<T, E> {
            #[inline]
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                serde::Deserializer::deserialize_identifier(deserializer, FieldVisitor::default())
            }
        }

        struct Visitor<'de, T, E>
        where
            T: Deserialize<'de>,
            E: Deserialize<'de>,
        {
            marker: PhantomData<Try<T, E>>,
            lifetime: PhantomData<&'de ()>,
        }
        impl<'de, T, E> serde::de::Visitor<'de> for Visitor<'de, T, E>
        where
            T: Deserialize<'de>,
            E: Deserialize<'de>,
        {
            type Value = Try<T, E>;
            fn expecting(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
                fmt::Formatter::write_str(fmt, "enum #name_str")
            }
            fn visit_enum<__A>(self, __data: __A) -> serde::export::Result<Self::Value, __A::Error>
            where
                __A: serde::de::EnumAccess<'de>,
            {
                match match serde::de::EnumAccess::variant(__data) {
                    Ok(__val) => __val,
                    Err(__err) => {
                        return Err(__err);
                    }
                } {
                    (Field::<T, E>::F0(_), __variant) => Result::map(
                        serde::de::VariantAccess::newtype_variant::<Success<T>>(__variant),
                        Try::Success,
                    ),
                    (Field::<T, E>::F1(_), __variant) => Result::map(
                        serde::de::VariantAccess::newtype_variant::<Failure<E>>(__variant),
                        Try::Failure,
                    ),
                }
            }
        }

        const VARIANTS: &[&str] = &["Success", "Error"];
        serde::Deserializer::deserialize_enum(
            deserializer,
            "Try",
            VARIANTS,
            Visitor {
                marker: PhantomData::<Try<T, E>>,
                lifetime: PhantomData,
            },
        )
    }
}

#[amqp_derive(descriptor(name = "net.corda:e+qsW/cJ4ajGpb8YkJWB1A=="))]
#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct Success<T> {
    value: T,
}

#[amqp_derive(descriptor(name = "net.corda:????????????????????????"))]
#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct Failure<T> {
    value: T,
}

pub enum SectionId {
    DataAndStop,
    AltDataAndStop,
    Encoding,
}

#[amqp_derive(descriptor(code = 0xc5620000_00000001))]
#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct Envelope<'a, T> {
    pub obj: T,
    #[serde(borrow)]
    pub schema: Schema<'a>,
    pub transforms_schema: Option<TransformsSchema>,
}

impl<'a, T> Envelope<'a, T> {
    pub fn decode<'b>(mut buf: &'b [u8]) -> Result<Envelope<'b, T>, Error>
    where
        T: Deserialize<'b>,
    {
        if buf.len() < CORDA_MAGIC.len() || &buf[..7] != CORDA_MAGIC {
            return Err(Error::InvalidData);
        }
        buf = &buf[7..];

        if buf.is_empty() || buf[0] != (SectionId::DataAndStop as u8) {
            return Err(Error::InvalidData);
        }
        buf = &buf[1..];

        let (this, rest) = de::deserialize::<Envelope<T>>(buf)?;
        if !rest.is_empty() {
            return Err(Error::TrailingCharacters);
        }

        Ok(this)
    }

    pub fn encode(&self, buf: &mut Vec<u8>) -> Result<(), oasis_amqp::Error>
    where
        T: Serialize,
    {
        buf.extend_from_slice(CORDA_MAGIC);
        buf.push(SectionId::DataAndStop as u8);
        ser::into_bytes(self, buf)
    }
}

#[amqp_derive(descriptor(code = 0xc5620000_00000002))]
#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct Schema<'a> {
    #[serde(borrow)]
    pub types: amqp::List<TypeNotation<'a>>,
}

#[amqp_derive(descriptor(code = 0xc5620000_00000003))]
#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct Descriptor<'a> {
    #[serde(borrow)]
    pub name: Option<amqp::Symbol<'a>>,
    pub code: Option<u64>,
}

#[amqp_derive(descriptor(code = 0xc5620000_00000004))]
#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct Field<'a> {
    pub name: &'a str,
    #[serde(rename = "type")]
    pub ty: &'a str,
    #[serde(borrow)]
    pub requires: amqp::List<&'a str>,
    pub default: Option<&'a str>,
    pub label: Option<&'a str>,
    pub mandatory: bool,
    pub multiple: bool,
}

#[amqp_derive(descriptor(name = "net.corda:1BLPJgNvsxdvPcbrIQd87g=="))]
#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct ObjectList(pub amqp::List<()>);

#[amqp_derive]
#[derive(Debug, PartialEq, Serialize)]
pub enum TypeNotation<'a> {
    CompositeType(CompositeType<'a>),
    RestrictedType(RestrictedType<'a>),
}

#[amqp_derive(descriptor(code = 0xc5620000_00000005))]
#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct CompositeType<'a> {
    pub name: &'a str,
    pub label: Option<&'a str>,
    pub provides: amqp::List<&'a str>,
    pub descriptor: Descriptor<'a>,
    pub fields: amqp::List<Field<'a>>,
}

#[amqp_derive(descriptor(code = 0xc5620000_00000006))]
#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct RestrictedType<'a> {
    pub name: &'a str,
    pub label: Option<&'a str>,
    pub provides: amqp::List<&'a str>,
    pub source: &'a str,
    pub descriptor: Descriptor<'a>,
    pub choices: amqp::List<Choice<'a>>,
}

#[amqp_derive(descriptor(code = 0xc5620000_00000007))]
#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct Choice<'a> {
    pub name: &'a str,
    pub value: &'a str,
}

#[amqp_derive(descriptor(code = 0xc5620000_00000009))]
#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct TransformsSchema {}

pub const CORDA_MAGIC: &[u8; 7] = b"corda\x01\x00";
