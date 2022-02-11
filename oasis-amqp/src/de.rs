use std::convert::TryInto;
use std::{fmt, str};

use serde::de::{self, DeserializeSeed, EnumAccess, MapAccess, SeqAccess, VariantAccess, Visitor};

use crate::{Described, Error};

pub fn deserialize<'a, T: de::Deserialize<'a>>(bytes: &'a [u8]) -> Result<(T, &'a [u8])> {
    let mut deserializer = Deserializer::from_bytes(bytes);
    let val = T::deserialize(&mut deserializer)?;
    Ok((val, deserializer.input))
}

pub struct Deserializer<'de> {
    input: &'de [u8],
    constructor: Option<usize>,
    any: bool,
}

impl<'de> Deserializer<'de> {
    pub fn from_bytes(input: &'de [u8]) -> Self {
        Deserializer {
            input,
            constructor: None,
            any: false,
        }
    }

    fn peek(&self) -> Result<u8> {
        self.input.get(0).copied().ok_or(Error::UnexpectedEnd)
    }

    fn next(&mut self) -> Result<u8> {
        let res = self.peek();
        self.input = &self.input[1..];
        res
    }

    fn assume(&mut self, assumed: u8) -> Result<()> {
        if let Ok(val) = self.next() {
            assert_eq!(val, assumed);
            Ok(())
        } else {
            Err(Error::UnexpectedEnd)
        }
    }

    fn read_u32(&mut self) -> Result<u32> {
        let (val, rest) = self.input.split_at(4);
        self.input = rest;
        Ok(u32::from_be_bytes(val.try_into()?))
    }

    fn parse_bool(&mut self) -> Result<bool> {
        Ok(match self.next_constructor()? {
            0x56 => match self.next()? {
                0x01 => true,
                0x00 => false,
                v => return Err(InvalidFormatCode::new("bool", v).into()),
            },
            0x41 => true,
            0x42 => false,
            t => return Err(InvalidFormatCode::new("bool", t as u8).into()),
        })
    }

    fn parse_descriptor(&mut self) -> Result<Descriptor<'de>> {
        self.assume(0)?;
        match self.peek()? {
            0x44 | 0x53 | 0x80 => Ok(Descriptor::Numeric(self.parse_u64()?)),
            0xa3 | 0xb3 => Ok(Descriptor::Symbol(self.parse_bytes()?)),
            f => Err(InvalidFormatCode::new("descriptor", f).into()),
        }
    }

    fn parse_u64(&mut self) -> Result<u64> {
        Ok(match self.next()? {
            0x44 => 0,
            0x53 => self.next()? as u64,
            0x80 => {
                let (val, rest) = self.input.split_at(8);
                self.input = rest;
                let val = val.try_into()?;
                u64::from_be_bytes(val)
            }
            t => return Err(InvalidFormatCode::new("u64", t).into()),
        })
    }

    fn parse_bytes(&mut self) -> Result<&'de [u8]> {
        let len = match self.next_constructor()? {
            0xa0 | 0xa3 => self.next()? as usize,
            0xb0 | 0xb3 => self.read_u32()? as usize,
            t => return Err(InvalidFormatCode::new("bytes", t as u8).into()),
        };

        let (val, rest) = self.input.split_at(len);
        self.input = rest;
        Ok(val)
    }

    fn peek_constructor(&mut self) -> Result<usize> {
        Ok(match self.constructor.as_ref() {
            Some(v) => *v,
            None => self.peek()? as usize,
        })
    }

    fn next_constructor(&mut self) -> Result<usize> {
        Ok(match self.constructor.as_ref() {
            Some(v) => *v,
            None => self.next()? as usize,
        })
    }

    // size, len, constructor
    fn composite(&mut self) -> Result<(usize, usize, Option<usize>)> {
        Ok(match self.next()? {
            0x45 => (0, 0, None),
            0xc0 => (self.next()? as usize - 1, self.next()? as usize, None),
            0xc1 => (self.next()? as usize - 1, self.next()? as usize, None),
            0xd0 => (
                self.read_u32()? as usize - 4,
                self.read_u32()? as usize,
                None,
            ),
            0xd1 => (
                self.read_u32()? as usize - 4,
                self.read_u32()? as usize,
                None,
            ),
            0xe0 => (
                self.next()? as usize - 2,
                self.next()? as usize,
                Some(self.next()? as usize),
            ),
            0xf0 => (
                self.read_u32()? as usize - 8,
                self.read_u32()? as usize,
                Some(self.read_u32()? as usize),
            ),
            t => return Err(InvalidFormatCode::new("composite type", t).into()),
        })
    }

    pub fn reader(&mut self) -> Result<DescribedReader<'de>> {
        DescribedReader::new(self.parse_descriptor()?)
    }
}

impl<'de, 'a> de::Deserializer<'de> for &'a mut Deserializer<'de> {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        match self.peek_constructor()? {
            0x40 => self.deserialize_unit(visitor),
            0x56 | 0x41 | 0x42 => self.deserialize_bool(visitor),
            0x50 => self.deserialize_u8(visitor),
            0x60 => self.deserialize_u16(visitor),
            0x43 | 0x52 | 0x70 => self.deserialize_u32(visitor),
            0x44 | 0x53 | 0x80 => self.deserialize_u64(visitor),
            0x51 => self.deserialize_i8(visitor),
            0x61 => self.deserialize_i16(visitor),
            0x54 | 0x71 => self.deserialize_i32(visitor),
            0x55 | 0x81 | 0x83 => self.deserialize_i64(visitor),
            0x72 => self.deserialize_f32(visitor),
            0x82 => self.deserialize_f64(visitor),
            0x45 | 0xc0 | 0xd0 => self.deserialize_seq(visitor),
            0x74 | 0x84 | 0x94 => unimplemented!(), // decimal32, decimal64, decimal128
            0x73 => self.deserialize_char(visitor),
            0xa1 | 0xb1 => self.deserialize_str(visitor),
            0xa0 | 0xa3 | 0xb0 | 0xb3 => self.deserialize_bytes(visitor),
            t => Err(InvalidFormatCode::new("any", t as u8).into()),
        }
    }

    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_bool(self.parse_bool()?)
    }

    fn deserialize_u8<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.assume(0x50)?;
        visitor.visit_u8(self.next()?)
    }

    fn deserialize_u16<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.assume(0x60)?;
        let (val, rest) = self.input.split_at(2);
        self.input = rest;
        let val = val.try_into()?;
        visitor.visit_u16(u16::from_be_bytes(val))
    }

    fn deserialize_u32<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        match self.next()? {
            0x43 => visitor.visit_u32(0),
            0x52 => visitor.visit_u32(self.next()? as u32),
            0x70 => visitor.visit_u32(self.read_u32()?),
            t => Err(InvalidFormatCode::new("u32", t).into()),
        }
    }

    fn deserialize_u64<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_u64(self.parse_u64()?)
    }

    fn deserialize_i8<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.assume(0x51)?;
        visitor.visit_i8(self.next()? as i8)
    }

    fn deserialize_i16<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.assume(0x61)?;
        let (val, rest) = self.input.split_at(2);
        self.input = rest;
        let val = val.try_into()?;
        visitor.visit_i16(i16::from_be_bytes(val))
    }

    fn deserialize_i32<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        match self.next()? {
            0x54 => visitor.visit_i32(self.next()? as i32),
            0x71 => {
                let (val, rest) = self.input.split_at(4);
                self.input = rest;
                let val = val.try_into()?;
                visitor.visit_i32(i32::from_be_bytes(val))
            }
            t => Err(InvalidFormatCode::new("i32", t).into()),
        }
    }

    fn deserialize_i64<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        match self.next()? {
            0x55 => visitor.visit_i64(self.next()? as i64),
            0x81 | 0x83 => {
                let (val, rest) = self.input.split_at(8);
                self.input = rest;
                let val = val.try_into()?;
                visitor.visit_i64(i64::from_be_bytes(val))
            }
            t => Err(InvalidFormatCode::new("i64", t).into()),
        }
    }

    fn deserialize_f32<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.assume(0x72)?;
        visitor.visit_f32(f32::from_bits(self.read_u32()?))
    }

    fn deserialize_f64<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.assume(0x82)?;
        let (val, rest) = self.input.split_at(8);
        self.input = rest;
        let val = u64::from_be_bytes(val.try_into()?);
        visitor.visit_f64(f64::from_bits(val))
    }

    fn deserialize_char<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        unimplemented!()
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let len = match self.next_constructor()? {
            0xa1 | 0xa3 => self.next()? as usize,
            0xb1 | 0xb3 => self.read_u32()? as usize,
            t => return Err(InvalidFormatCode::new("str", t as u8).into()),
        };

        let (val, rest) = self.input.split_at(len);
        self.input = rest;
        match str::from_utf8(val) {
            Ok(s) => visitor.visit_borrowed_str(s),
            Err(_) => Err(Error::InvalidData),
        }
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_str(visitor)
    }

    fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_borrowed_bytes(self.parse_bytes()?)
    }

    fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_byte_buf(self.parse_bytes()?.to_owned())
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        if self.input.is_empty() {
            visitor.visit_none()
        } else if self.peek()? == 0x40 {
            self.assume(0x40)?;
            visitor.visit_none()
        } else {
            visitor.visit_some(self)
        }
    }

    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.assume(0x40)?;
        visitor.visit_unit()
    }

    fn deserialize_unit_struct<V>(self, _name: &'static str, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_unit(visitor)
    }

    fn deserialize_newtype_struct<V>(self, _name: &'static str, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        // Ignore potential descriptors, which are likely here because
        // Corda confuses (heterogeneous) lists and (homogeneous) arrays.
        if self.peek()? == 0 {
            let _ = self.parse_descriptor()?;
        }

        let (size, len, constructor) = self.composite()?;
        let (input, rest) = self.input.split_at(size);
        self.input = rest;

        let mut nested = Deserializer {
            input,
            constructor,
            any: false,
        };
        visitor.visit_seq(Access {
            de: &mut nested,
            len,
        })
    }

    fn deserialize_tuple<V>(self, _len: usize, _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        unimplemented!()
    }

    fn deserialize_tuple_struct<V>(
        self,
        _name: &'static str,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let (_, len, _) = self.composite()?;
        visitor.visit_map(Map::new(self, len / 2))
    }

    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        if self.peek()? == 0 {
            let _ = self.parse_descriptor()?;
        }

        let (size, _, constructor) = self.composite()?;
        let (input, rest) = self.input.split_at(size);
        self.input = rest;

        let mut nested = Deserializer {
            input,
            constructor,
            any: false,
        };
        visitor.visit_seq(Access {
            de: &mut nested,
            len: fields.len(),
        })
    }

    fn deserialize_enum<V>(
        self,
        name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        if self.peek_constructor()? == 0 {
            self.assume(0)?;
            visitor.visit_enum(Enum { de: self })
        } else {
            self.any = name == "Any";
            let res = visitor.visit_enum(Enum { de: self });
            self.any = false;
            res
        }
    }

    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        if !self.any {
            match self.peek_constructor()? {
                0x56 | 0x41 | 0x42 => visitor.visit_u64(if self.parse_bool()? { 1 } else { 0 }),
                0x50 => {
                    self.assume(0x50)?;
                    let id = self.next()?;
                    visitor.visit_u64(id as u64)
                }
                0x43 | 0x52 | 0x70 => visitor.visit_u64(self.read_u32()? as u64),
                0x44 | 0x53 | 0x80 => self.deserialize_u64(visitor),
                0xa3 | 0xb3 => self.deserialize_bytes(visitor),
                t => Err(InvalidFormatCode::new("variant identifier", t as u8).into()),
            }
        } else {
            visitor.visit_u64(self.peek_constructor()? as u64)
        }
    }

    fn deserialize_ignored_any<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_any(visitor)
    }
}

pub struct DescribedReader<'de> {
    descriptor: Option<Descriptor<'de>>,
}

impl<'de> DescribedReader<'de> {
    pub fn new(descriptor: Descriptor<'de>) -> Result<Self> {
        Ok(Self {
            descriptor: Some(descriptor),
        })
    }

    pub fn next(&mut self, deserializer: &mut Deserializer<'de>) -> Result<()> {
        if !deserializer.input.is_empty() {
            self.descriptor = Some(deserializer.parse_descriptor()?);
        }
        Ok(())
    }

    pub fn read<T: Described + serde::de::Deserialize<'de>>(
        &mut self,
        deserializer: &mut Deserializer<'de>,
        next: bool,
    ) -> Result<Option<T>> {
        use Descriptor::*;
        let matched = match &self.descriptor {
            Some(Numeric(v)) => T::CODE == Some(*v),
            Some(Symbol(s)) => T::NAME == Some(s),
            None => return Ok(None),
        };

        if !matched {
            return Ok(None);
        }

        let t = T::deserialize(&mut *deserializer)?;
        if next {
            self.next(deserializer)?;
        }
        Ok(Some(t))
    }
}

pub enum Descriptor<'a> {
    Numeric(u64),
    Symbol(&'a [u8]),
}

struct Access<'a, 'de: 'a> {
    de: &'a mut Deserializer<'de>,
    len: usize,
}

impl<'a, 'de> SeqAccess<'de> for Access<'a, 'de> {
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>>
    where
        T: DeserializeSeed<'de>,
    {
        if self.len > 0 {
            self.len -= 1;
            Ok(Some(seed.deserialize(&mut *self.de)?))
        } else {
            Ok(None)
        }
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.len)
    }
}

struct Enum<'a, 'de: 'a> {
    de: &'a mut Deserializer<'de>,
}

impl<'de, 'a> EnumAccess<'de> for Enum<'a, 'de> {
    type Error = Error;
    type Variant = Self;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant)>
    where
        V: DeserializeSeed<'de>,
    {
        Ok((seed.deserialize(&mut *self.de)?, self))
    }
}

impl<'de, 'a> VariantAccess<'de> for Enum<'a, 'de> {
    type Error = Error;

    fn unit_variant(self) -> Result<()> {
        Ok(())
    }

    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value>
    where
        T: DeserializeSeed<'de>,
    {
        seed.deserialize(self.de)
    }

    fn tuple_variant<V>(self, _len: usize, _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        unimplemented!()
    }

    fn struct_variant<V>(self, _fields: &'static [&'static str], _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        unimplemented!()
    }
}

struct Map<'a, 'de: 'a> {
    de: &'a mut Deserializer<'de>,
    left: usize,
}

impl<'a, 'de> Map<'a, 'de> {
    fn new(de: &'a mut Deserializer<'de>, left: usize) -> Self {
        Self { de, left }
    }
}

impl<'de, 'a> MapAccess<'de> for Map<'a, 'de> {
    type Error = Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>>
    where
        K: DeserializeSeed<'de>,
    {
        if self.left < 1 {
            return Ok(None);
        }
        self.left -= 1;
        seed.deserialize(&mut *self.de).map(Some)
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value>
    where
        V: DeserializeSeed<'de>,
    {
        seed.deserialize(&mut *self.de)
    }
}

#[derive(Debug)]
pub struct InvalidFormatCode {
    expected: &'static str,
    code: u8,
}

impl InvalidFormatCode {
    fn new(expected: &'static str, code: u8) -> Self {
        Self { expected, code }
    }
}

impl std::error::Error for InvalidFormatCode {}

impl fmt::Display for InvalidFormatCode {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(
            formatter,
            "expected {}, found format code {:?}",
            self.expected, self.code
        )
    }
}

type Result<T> = std::result::Result<T, Error>;
