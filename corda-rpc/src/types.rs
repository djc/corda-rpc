use std::fmt;
use std::marker::PhantomData;

use oasis_amqp::{amqp, de, proto::BytesFrame, ser, Described, Error};
use oasis_amqp_macros::amqp as amqp_derive;
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Eq, Serialize)]
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
                        let value = &std::string::String::from_utf8_lossy(value);
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
            fn visit_enum<__A>(self, __data: __A) -> std::result::Result<Self::Value, __A::Error>
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
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct Success<T> {
    pub(crate) value: T,
}

#[amqp_derive(descriptor(name = "net.corda:????????????????????????"))]
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct Failure<T> {
    pub(crate) value: T,
}

pub trait Rpc<'r> {
    type Arguments: Serialize;
    type OkResult: 'r;
    type Error: From<()> + 'r;

    fn method(&self) -> &'static str;

    fn request(&self) -> Envelope<Self::Arguments>;

    fn response(&self, response: &'r BytesFrame) -> Result<Self::OkResult, Self::Error>;
}

#[amqp_derive(descriptor(code = 0xc562_0000_0000_0001))]
#[derive(Debug, Eq, PartialEq, Serialize)]
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

#[amqp_derive(descriptor(code = 0xc562_0000_0000_0002))]
#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct Schema<'a> {
    #[serde(borrow)]
    pub types: amqp::List<TypeNotation<'a>>,
}

#[amqp_derive(descriptor(code = 0xc562_0000_0000_0003))]
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct Descriptor<'a> {
    #[serde(borrow)]
    pub name: Option<amqp::Symbol<'a>>,
    pub code: Option<u64>,
}

#[amqp_derive(descriptor(code = 0xc562_0000_0000_0004))]
#[derive(Debug, PartialEq, Eq, Serialize)]
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
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct ObjectList(pub amqp::List<()>);

#[amqp_derive]
#[derive(Debug, Eq, PartialEq, Serialize)]
pub enum TypeNotation<'a> {
    CompositeType(CompositeType<'a>),
    RestrictedType(RestrictedType<'a>),
}

#[amqp_derive(descriptor(code = 0xc562_0000_0000_0005))]
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct CompositeType<'a> {
    pub name: &'a str,
    pub label: Option<&'a str>,
    pub provides: amqp::List<&'a str>,
    pub descriptor: Descriptor<'a>,
    pub fields: amqp::List<Field<'a>>,
}

#[amqp_derive(descriptor(code = 0xc562_0000_0000_0006))]
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct RestrictedType<'a> {
    pub name: &'a str,
    pub label: Option<&'a str>,
    pub provides: amqp::List<&'a str>,
    pub source: &'a str,
    pub descriptor: Descriptor<'a>,
    pub choices: amqp::List<Choice<'a>>,
}

#[amqp_derive(descriptor(code = 0xc562_0000_0000_0007))]
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct Choice<'a> {
    pub name: &'a str,
    pub value: &'a str,
}

#[amqp_derive(descriptor(code = 0xc562_0000_0000_0009))]
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct TransformsSchema {}

pub(crate) enum SectionId {
    DataAndStop,
}

pub(crate) const CORDA_MAGIC: &[u8; 7] = b"corda\x01\x00";
