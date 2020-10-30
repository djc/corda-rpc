use std::array::TryFromSliceError;
use std::{fmt, io};

use thiserror::Error;

pub mod amqp;
pub mod de;
pub mod proto;
pub mod sasl;
pub mod ser;

pub use proto::Client;

pub trait Described {
    const NAME: Option<&'static [u8]>;
    const CODE: Option<u64>;
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid data")]
    InvalidData,
    #[error("invalid format code: {0}")]
    InvalidFormatCode(#[from] de::InvalidFormatCode),
    #[error("syntax")]
    Syntax,
    #[error("unexpected end")]
    UnexpectedEnd,
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("deserialization failed: {0}")]
    Deserialization(String),
    #[error("serialization failed: {0}")]
    Serialization(String),
    #[error("buffer not empty after deserialization")]
    TrailingCharacters,
}

impl serde::de::Error for Error {
    fn custom<T>(msg: T) -> Self
    where
        T: fmt::Display,
    {
        Error::Deserialization(msg.to_string())
    }
}

impl serde::ser::Error for Error {
    fn custom<T>(msg: T) -> Self
    where
        T: fmt::Display,
    {
        Error::Serialization(msg.to_string())
    }
}

impl From<TryFromSliceError> for Error {
    fn from(e: TryFromSliceError) -> Self {
        Error::Deserialization(e.to_string())
    }
}
