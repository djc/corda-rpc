use oasis_amqp_macros::amqp;
use serde::{self, Deserialize, Serialize};
use serde_bytes::Bytes;

use crate::Described;

#[amqp]
#[derive(Debug, PartialEq, Serialize)]
pub enum Frame<'a> {
    Mechanisms(Mechanisms),
    Init(Init<'a>),
    Outcome(Outcome<'a>),
}

#[amqp(descriptor("amqp:sasl-mechanisms:list", 0x00000000_00000040))]
#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct Mechanisms {
    pub sasl_server_mechanisms: Vec<Mechanism>,
}

#[amqp(descriptor("amqp:sasl-init:list", 0x00000000_00000041))]
#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct Init<'a> {
    pub mechanism: Mechanism,
    #[serde(borrow)]
    pub initial_response: Option<&'a Bytes>,
    pub hostname: Option<&'a str>,
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Mechanism {
    Anonymous,
    Plain,
    ScramSha1,
}

#[amqp(descriptor("amqp:sasl-outcome:list", 0x00000000_00000044))]
#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct Outcome<'a> {
    pub code: Code,
    #[serde(borrow)]
    pub additional_data: Option<&'a Bytes>,
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub enum Code {
    Ok,
    Auth,
    Sys,
    SysPerm,
    SysTemp,
}
