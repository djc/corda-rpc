use oasis_amqp_macros::amqp;
use serde::{self, Deserialize, Serialize};
use serde_bytes::Bytes;

use crate::Described;

#[amqp]
#[derive(Debug, Eq, PartialEq, Serialize)]
pub enum Frame<'a> {
    Mechanisms(Mechanisms),
    Init(Init<'a>),
    Outcome(Outcome<'a>),
}

#[amqp(descriptor("amqp:sasl-mechanisms:list", 0x0000_0000_0000_0040))]
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct Mechanisms {
    pub sasl_server_mechanisms: Vec<Mechanism>,
}

#[amqp(descriptor("amqp:sasl-init:list", 0x0000_0000_0000_0041))]
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct Init<'a> {
    pub mechanism: Mechanism,
    #[serde(borrow)]
    pub initial_response: Option<&'a Bytes>,
    pub hostname: Option<&'a str>,
}

#[derive(Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Mechanism {
    Anonymous,
    Plain,
    ScramSha1,
}

#[amqp(descriptor("amqp:sasl-outcome:list", 0x0000_0000_0000_0044))]
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct Outcome<'a> {
    pub code: Code,
    #[serde(borrow)]
    pub additional_data: Option<&'a Bytes>,
}

#[derive(Debug, Deserialize, PartialEq, Eq, Serialize)]
pub enum Code {
    Ok,
    Auth,
    Sys,
    SysPerm,
    SysTemp,
}
