use std::convert::TryInto;
use std::str;

use serde::de::{self, DeserializeSeed, EnumAccess, SeqAccess, VariantAccess, Visitor};

use crate::Error;

pub fn deserialize<'a, T: de::Deserialize<'a>>(bytes: &'a [u8]) -> Result<(T, &'a [u8])> {
    let mut deserializer = Deserializer::from_bytes(bytes);
    let val = T::deserialize(&mut deserializer)?;
    Ok((val, deserializer.input))
}

pub struct Deserializer<'de> {
    input: &'de [u8],
    constructor: Option<usize>,
}

impl<'de> Deserializer<'de> {
    pub fn from_bytes(input: &'de [u8]) -> Self {
        Deserializer {
            input,
            constructor: None,
        }
    }

    fn peek(&self) -> Result<u8> {
        self.input.get(0).map(|b| *b).ok_or(Error::UnexpectedEnd)
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
            0xd0 => (
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
            _ => return Err(Error::InvalidData),
        })
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
            0x55 | 0x81 => self.deserialize_i64(visitor),
            0x72 => self.deserialize_f32(visitor),
            0x82 => self.deserialize_f64(visitor),
            0x45 | 0xc0 | 0xd0 => self.deserialize_seq(visitor),
            0x74 | 0x84 | 0x94 => unimplemented!(), // decimal32, decimal64, decimal128
            0x73 => self.deserialize_char(visitor),
            0xa1 | 0xb1 => self.deserialize_str(visitor),
            0xa0 | 0xa3 | 0xb0 | 0xb3 => self.deserialize_bytes(visitor),
            _ => Err(Error::Syntax),
        }
    }

    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        match self.next_constructor()? {
            0x56 => {
                let val = self.next()?;
                visitor.visit_bool(match val {
                    0x01 => true,
                    0x00 => false,
                    _ => return Err(Error::InvalidData),
                })
            }
            0x41 => visitor.visit_bool(true),
            0x42 => visitor.visit_bool(false),
            _ => return Err(Error::InvalidData),
        }
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
            _ => Err(Error::InvalidData),
        }
    }

    fn deserialize_u64<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        match self.next()? {
            0x44 => visitor.visit_u64(0),
            0x53 => visitor.visit_u64(self.next()? as u64),
            0x80 => {
                let (val, rest) = self.input.split_at(8);
                self.input = rest;
                let val = val.try_into()?;
                visitor.visit_u64(u64::from_be_bytes(val))
            }
            _ => Err(Error::InvalidData),
        }
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
            _ => Err(Error::InvalidData),
        }
    }

    fn deserialize_i64<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        match self.next()? {
            0x55 => visitor.visit_i64(self.next()? as i64),
            0x81 => {
                let (val, rest) = self.input.split_at(8);
                self.input = rest;
                let val = val.try_into()?;
                visitor.visit_i64(i64::from_be_bytes(val))
            }
            _ => Err(Error::InvalidData),
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
            _ => return Err(Error::InvalidData),
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
        let len = match self.next_constructor()? {
            0xa0 | 0xa3 => self.next()? as usize,
            0xb0 | 0xb3 => self.read_u32()? as usize,
            _ => return Err(Error::InvalidData),
        };

        let (val, rest) = self.input.split_at(len);
        self.input = rest;
        visitor.visit_borrowed_bytes(val)
    }

    fn deserialize_byte_buf<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        unimplemented!()
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

    fn deserialize_unit<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        unimplemented!()
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
        let (size, len, constructor) = self.composite()?;
        let (input, rest) = self.input.split_at(size);
        self.input = rest;

        let mut nested = Deserializer { input, constructor };
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

    fn deserialize_map<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        unimplemented!()
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
        let (size, _, constructor) = self.composite()?;
        let (input, rest) = self.input.split_at(size);
        self.input = rest;

        let mut nested = Deserializer { input, constructor };
        visitor.visit_seq(Access {
            de: &mut nested,
            len: fields.len(),
        })
    }

    fn deserialize_enum<V>(
        self,
        _name: &'static str,
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
            visitor.visit_enum(Enum { de: self })
        }
    }

    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        match self.peek_constructor()? {
            0x50 => {
                self.assume(0x50)?;
                let id = self.next()?;
                visitor.visit_u64(id as u64)
            }
            0x44 | 0x53 | 0x80 => self.deserialize_u64(visitor),
            0xa3 | 0xb3 => self.deserialize_bytes(visitor),
            _ => Err(Error::InvalidData),
        }
    }

    fn deserialize_ignored_any<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_any(visitor)
    }
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

type Result<T> = std::result::Result<T, Error>;
