use std::array::TryFromSliceError;
use std::{fmt, io, str};

use err_derive::Error;
use serde;

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
    #[error(display = "invalid data")]
    InvalidData,
    #[error(display = "invalid format code: {}", _0)]
    InvalidFormatCode(#[source] de::InvalidFormatCode),
    #[error(display = "syntax")]
    Syntax,
    #[error(display = "unexpected end")]
    UnexpectedEnd,
    #[error(display = "I/O error: {}", _0)]
    Io(#[source] io::Error),
    #[error(display = "deserialization failed: {}", _0)]
    Deserialization(String),
    #[error(display = "serialization failed: {}", _0)]
    Serialization(String),
    #[error(display = "buffer not empty after deserialization")]
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
