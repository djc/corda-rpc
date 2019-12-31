use oasis_amqp_derive::amqp;
use serde::{self, Deserialize, Serialize};
use serde_bytes::Bytes;

use crate::Described;

#[amqp]
#[derive(Debug, Serialize)]
pub enum Frame<'a> {
    Mechanisms(Mechanisms),
    Init(Init<'a>),
    Outcome(Outcome<'a>),
}

#[amqp(descriptor("amqp:sasl-mechanisms:list", 0x00000000_00000040))]
#[derive(Debug, Deserialize, Serialize)]
pub struct Mechanisms {
    sasl_server_mechanisms: Vec<Mechanism>,
}

#[amqp(descriptor("amqp:sasl-init:list", 0x00000000_00000041))]
#[derive(Debug, Deserialize, Serialize)]
pub struct Init<'a> {
    pub mechanism: Mechanism,
    #[serde(borrow)]
    pub initial_response: Option<&'a Bytes>,
    pub hostname: Option<&'a str>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Mechanism {
    Anonymous,
    Plain,
    ScramSha1,
}

#[amqp(descriptor("amqp:sasl-outcome:list", 0x00000000_00000044))]
#[derive(Debug, Deserialize, Serialize)]
pub struct Outcome<'a> {
    code: Code,
    #[serde(borrow)]
    additional_data: Option<&'a Bytes>,
}

#[derive(Debug, Deserialize, Serialize)]
pub enum Code {
    Ok,
    Auth,
    Sys,
    SysPerm,
    SysTemp,
}
