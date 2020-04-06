use oasis_amqp::{amqp, ser, Described};
use oasis_amqp_macros::amqp as amqp_derive;
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
    pub transforms_schema: Option<TransformsSchema>,
}

impl<'a, 'de: 'a, T> Envelope<'a, T>
where
    T: Deserialize<'de> + Serialize,
{
    pub fn encode(&self, buf: &mut Vec<u8>) -> Result<(), oasis_amqp::Error> {
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
