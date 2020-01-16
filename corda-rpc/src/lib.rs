use oasis_amqp::{amqp, Described};
use oasis_amqp_derive::amqp as amqp_derive;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
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
