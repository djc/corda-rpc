use std::fmt;

use oasis_amqp::{amqp, proto::BytesFrame, Described};
use oasis_amqp_macros::amqp as amqp_derive;
use serde::{Deserialize, Serialize};

use crate::types::{
    Descriptor, Envelope, Failure, ObjectList, RestrictedType, Rpc, Schema, Success, Try,
    TypeNotation,
};

pub struct NetworkMapSnapshot;

impl<'r> Rpc<'r> for NetworkMapSnapshot {
    type Arguments = ObjectList;
    type OkResult = Vec<NodeInfo<'r>>;
    type Error = ();

    fn method(&self) -> &'static str {
        "networkMapSnapshot"
    }

    fn request(&self) -> Envelope<Self::Arguments> {
        Envelope {
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
        }
    }

    fn response(&self, response: &'r BytesFrame) -> Result<Self::OkResult, Self::Error> {
        let body = response.body().ok_or(())?;
        let rsp = Envelope::<Try<amqp::List<NodeInfo>, ()>>::decode(body).map_err(|_| ())?;
        match rsp.obj {
            Try::Success(Success { value }) => Ok(value.0),
            Try::Failure(Failure { value: () }) => Err(()),
        }
    }
}

#[amqp_derive(descriptor(name = "net.corda:ncUcZzvT9YGn0ItdoWW3QQ=="))]
#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct NodeInfo<'a> {
    #[serde(borrow)]
    pub addresses: amqp::List<NetworkHostAndPort<'a>>,
    pub legal_identities_and_certs: amqp::List<PartyAndCertificate<'a>>,
    pub platform_version: i32,
    pub serial: i64,
}

#[amqp_derive(descriptor(name = "net.corda:IA+5d7+UvO6yts6wDzr86Q=="))]
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct NetworkHostAndPort<'a> {
    pub host: &'a str,
    pub port: i32,
}

#[amqp_derive(descriptor(name = "net.corda:GaPpq/rL9KtfTOQDN9ZCbA=="))]
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct PartyAndCertificate<'a> {
    #[serde(borrow)]
    pub cert_path: CertPath<'a>,
}

#[amqp_derive(descriptor(name = "net.corda:e+qsW/cJ4ajGpb8YkJWB1A=="))]
#[derive(PartialEq, Eq, Serialize)]
pub struct CertPath<'a> {
    #[serde(with = "serde_bytes")]
    pub data: &'a [u8],
    pub ty: &'a str,
}

impl<'a> fmt::Debug for CertPath<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("CertPath")
            .field("data", &"[certificate data elided]")
            .field("ty", &self.ty)
            .finish()
    }
}
