use std::fmt;
use std::marker::PhantomData;

use oasis_amqp::{amqp, de, ser, Described, Error};
use oasis_amqp_macros::amqp as amqp_derive;
use serde::{Deserialize, Serialize};
use serde_bytes;

#[amqp_derive(descriptor(name = "net.corda:ncUcZzvT9YGn0ItdoWW3QQ=="))]
#[derive(Debug, Deserialize, Serialize)]
pub struct NodeInfo<'a> {
    #[serde(borrow)]
    addresses: amqp::List<NetworkHostAndPort<'a>>,
    legal_identities_and_certs: amqp::List<PartyAndCertificate<'a>>,
    platform_version: i32,
    serial: i64,
}

#[amqp_derive(descriptor(name = "net.corda:IA+5d7+UvO6yts6wDzr86Q=="))]
#[derive(Debug, Deserialize, Serialize)]
pub struct NetworkHostAndPort<'a> {
    host: &'a str,
    port: i32,
}

#[amqp_derive(descriptor(name = "net.corda:GaPpq/rL9KtfTOQDN9ZCbA=="))]
#[derive(Debug, Deserialize, Serialize)]
pub struct PartyAndCertificate<'a> {
    #[serde(borrow)]
    cert_path: CertPath<'a>,
}

#[amqp_derive(descriptor(name = "net.corda:e+qsW/cJ4ajGpb8YkJWB1A=="))]
#[derive(Deserialize, Serialize)]
pub struct CertPath<'a> {
    #[serde(with = "serde_bytes")]
    data: &'a [u8],
    ty: &'a str,
}

impl<'a> fmt::Debug for CertPath<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("CertPath")
            .field("data", &"[certificate data elided]")
            .field("ty", &self.ty)
            .finish()
    }
}

#[derive(Debug, Serialize)]
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

        const VARIANTS: &[&'static str] = &["Success", "Error"];
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
#[derive(Debug, Deserialize, Serialize)]
pub struct Success<T> {
    value: T,
}

#[amqp_derive(descriptor(name = "net.corda:????????????????????????"))]
#[derive(Debug, Deserialize, Serialize)]
pub struct Failure<T> {
    value: T,
}

pub enum SectionId {
    DataAndStop,
    AltDataAndStop,
    Encoding,
}

#[amqp_derive(descriptor(code = 0xc5620000_00000001))]
#[derive(Debug, Deserialize, Serialize)]
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

        if buf.len() < 1 || buf[0] != (SectionId::DataAndStop as u8) {
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
#[derive(Debug, Deserialize, Serialize)]
pub struct Schema<'a> {
    #[serde(borrow)]
    pub types: amqp::List<TypeNotation<'a>>,
}

#[amqp_derive(descriptor(code = 0xc5620000_00000003))]
#[derive(Debug, Deserialize, Serialize)]
pub struct Descriptor<'a> {
    #[serde(borrow)]
    pub name: Option<amqp::Symbol<'a>>,
    pub code: Option<u64>,
}

#[amqp_derive(descriptor(code = 0xc5620000_00000004))]
#[derive(Debug, Deserialize, Serialize)]
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
#[derive(Debug, Deserialize, Serialize)]
pub struct ObjectList(pub amqp::List<()>);

#[amqp_derive]
#[derive(Debug, Serialize)]
pub enum TypeNotation<'a> {
    CompositeType(CompositeType<'a>),
    RestrictedType(RestrictedType<'a>),
}

#[amqp_derive(descriptor(code = 0xc5620000_00000005))]
#[derive(Debug, Deserialize, Serialize)]
pub struct CompositeType<'a> {
    pub name: &'a str,
    pub label: Option<&'a str>,
    pub provides: amqp::List<&'a str>,
    pub descriptor: Descriptor<'a>,
    pub fields: amqp::List<Field<'a>>,
}

#[amqp_derive(descriptor(code = 0xc5620000_00000006))]
#[derive(Debug, Deserialize, Serialize)]
pub struct RestrictedType<'a> {
    pub name: &'a str,
    pub label: Option<&'a str>,
    pub provides: amqp::List<&'a str>,
    pub source: &'a str,
    pub descriptor: Descriptor<'a>,
    pub choices: amqp::List<Choice<'a>>,
}

#[amqp_derive(descriptor(code = 0xc5620000_00000007))]
#[derive(Debug, Deserialize, Serialize)]
pub struct Choice<'a> {
    pub name: &'a str,
    pub value: &'a str,
}

#[amqp_derive(descriptor(code = 0xc5620000_00000009))]
#[derive(Debug, Deserialize, Serialize)]
pub struct TransformsSchema {}

pub const CORDA_MAGIC: &[u8; 7] = b"corda\x01\x00";

#[cfg(test)]
mod tests {
    use super::*;
    use oasis_amqp::ser;

    #[test]
    fn encode() {
        let envelope = Envelope {
            obj: ObjectList(amqp::List::default()),
            schema: Schema {
                types: vec![TypeNotation::RestrictedType(RestrictedType {
                    name: "java.util.List<java.lang.Object>",
                    label: None,
                    provides: amqp::List::default(),
                    source: "list",
                    descriptor: Descriptor {
                        name: Some("net.corda:1BLPJgNvsxdvPcbrIQd87g==".into()),
                        code: None,
                    },
                    choices: amqp::List::default(),
                })]
                .into(),
            },
            transforms_schema: None,
        };

        let mut body = vec![];
        body.extend_from_slice(CORDA_MAGIC);
        body.push(SectionId::DataAndStop as u8);
        ser::into_bytes(&envelope, &mut body).unwrap();
        assert_eq!(
            body,
            vec![
                99, 111, 114, 100, 97, 1, 0, 0, 0, 128, 197, 98, 0, 0, 0, 0, 0, 1, 208, 0, 0, 0,
                213, 0, 0, 0, 3, 0, 163, 34, 110, 101, 116, 46, 99, 111, 114, 100, 97, 58, 49, 66,
                76, 80, 74, 103, 78, 118, 115, 120, 100, 118, 80, 99, 98, 114, 73, 81, 100, 56, 55,
                103, 61, 61, 208, 0, 0, 0, 4, 0, 0, 0, 0, 0, 128, 197, 98, 0, 0, 0, 0, 0, 2, 208,
                0, 0, 0, 147, 0, 0, 0, 1, 208, 0, 0, 0, 138, 0, 0, 0, 1, 0, 128, 197, 98, 0, 0, 0,
                0, 0, 6, 208, 0, 0, 0, 119, 0, 0, 0, 6, 161, 32, 106, 97, 118, 97, 46, 117, 116,
                105, 108, 46, 76, 105, 115, 116, 60, 106, 97, 118, 97, 46, 108, 97, 110, 103, 46,
                79, 98, 106, 101, 99, 116, 62, 64, 208, 0, 0, 0, 4, 0, 0, 0, 0, 161, 4, 108, 105,
                115, 116, 0, 128, 197, 98, 0, 0, 0, 0, 0, 3, 208, 0, 0, 0, 41, 0, 0, 0, 2, 163, 34,
                110, 101, 116, 46, 99, 111, 114, 100, 97, 58, 49, 66, 76, 80, 74, 103, 78, 118,
                115, 120, 100, 118, 80, 99, 98, 114, 73, 81, 100, 56, 55, 103, 61, 61, 64, 208, 0,
                0, 0, 4, 0, 0, 0, 0, 64
            ]
        );
    }
}
