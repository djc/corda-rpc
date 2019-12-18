use serde::{ser, Serialize};

use crate::Error;

// By convention, the public API of a Serde serializer is one or more `to_abc`
// functions such as `to_string`, `to_bytes`, or `to_writer` depending on what
// Rust types the serializer is able to produce as output.
//
// This basic serializer supports only `to_string`.
pub fn into_bytes<T>(value: &T, output: &mut Vec<u8>) -> Result<()>
where
    T: Serialize,
{
    let mut serializer = Serializer {
        output,
        offsets: vec![],
    };
    value.serialize(&mut serializer)?;
    Ok(())
}

pub struct Serializer<'a> {
    output: &'a mut Vec<u8>,
    offsets: Vec<usize>,
}

impl ser::Serializer for &'_ mut Serializer<'_> {
    type Ok = ();
    type Error = Error;

    type SerializeSeq = Self;
    type SerializeTuple = Self;
    type SerializeTupleStruct = Self;
    type SerializeTupleVariant = Self;
    type SerializeMap = Self;
    type SerializeStruct = Self;
    type SerializeStructVariant = Self;

    fn serialize_bool(self, v: bool) -> Result<()> {
        self.output.push(if v { 0x41 } else { 0x42 });
        Ok(())
    }

    fn serialize_u8(self, v: u8) -> Result<()> {
        self.output.push(0x50);
        self.output.push(v);
        Ok(())
    }

    fn serialize_u16(self, v: u16) -> Result<()> {
        self.output.push(0x60);
        self.output.extend_from_slice(&v.to_be_bytes()[..]);
        Ok(())
    }

    fn serialize_u32(self, v: u32) -> Result<()> {
        if v == 0 {
            self.output.push(0x43);
        } else if v < 256 {
            self.output.push(0x52);
            self.output.push(v as u8);
        } else {
            self.output.push(0x70);
            self.output.extend_from_slice(&v.to_be_bytes()[..]);
        }
        Ok(())
    }

    fn serialize_u64(self, v: u64) -> Result<()> {
        if v == 0 {
            self.output.push(0x44);
        } else if v < 256 {
            self.output.push(0x53);
            self.output.push(v as u8);
        } else {
            self.output.push(0x80);
            self.output.extend_from_slice(&v.to_be_bytes()[..]);
        }
        Ok(())
    }

    fn serialize_i8(self, v: i8) -> Result<()> {
        self.output.push(0x51);
        self.output.push(v as u8);
        Ok(())
    }

    fn serialize_i16(self, v: i16) -> Result<()> {
        self.output.push(0x61);
        self.output.extend_from_slice(&v.to_be_bytes()[..]);
        Ok(())
    }

    fn serialize_i32(self, v: i32) -> Result<()> {
        if v < 256 {
            self.output.push(0x54);
            self.output.push(v as u8);
        } else {
            self.output.push(0x71);
            self.output.extend_from_slice(&v.to_be_bytes()[..]);
        }
        Ok(())
    }

    fn serialize_i64(self, v: i64) -> Result<()> {
        if v < 256 {
            self.output.push(0x55);
            self.output.push(v as u8);
        } else {
            self.output.push(0x81);
            self.output.extend_from_slice(&v.to_be_bytes()[..]);
        }
        Ok(())
    }

    fn serialize_f32(self, v: f32) -> Result<()> {
        self.output.push(0x72);
        self.output
            .extend_from_slice(&v.to_bits().to_be_bytes()[..]);
        Ok(())
    }

    fn serialize_f64(self, v: f64) -> Result<()> {
        self.output.push(0x82);
        self.output
            .extend_from_slice(&v.to_bits().to_be_bytes()[..]);
        Ok(())
    }

    fn serialize_char(self, v: char) -> Result<()> {
        self.output.push(0x73);
        self.output.extend_from_slice(&(v as u32).to_be_bytes()[..]);
        Ok(())
    }

    fn serialize_str(self, v: &str) -> Result<()> {
        if v.len() < 256 {
            self.output.push(0xa1);
            self.output.push(v.len() as u8);
            self.output.extend_from_slice(v.as_bytes());
        } else if v.len() < std::u32::MAX as usize {
            self.output.push(0xa1);
            self.output
                .extend_from_slice(&(v.len() as u32).to_be_bytes()[..]);
            self.output.extend_from_slice(v.as_bytes());
        } else {
            return Err(Error::InvalidData);
        }
        Ok(())
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<()> {
        if v.len() < 256 {
            self.output.push(0xa0);
            self.output.push(v.len() as u8);
            self.output.extend_from_slice(v);
        } else if v.len() < std::u32::MAX as usize {
            self.output.push(0xb0);
            self.output
                .extend_from_slice(&(v.len() as u32).to_be_bytes()[..]);
            self.output.extend_from_slice(v);
        } else {
            return Err(Error::InvalidData);
        }
        Ok(())
    }

    fn serialize_none(self) -> Result<()> {
        self.serialize_unit()
    }

    fn serialize_some<T>(self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<()> {
        self.output.push(0x40);
        Ok(())
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<()> {
        self.serialize_unit()
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<()> {
        let v = variant.as_bytes();
        if v.len() < 256 {
            self.output.push(0xa3);
            self.output.push(v.len() as u8);
            self.output.extend_from_slice(v);
        } else if v.len() < std::u32::MAX as usize {
            self.output.push(0xb3);
            self.output
                .extend_from_slice(&(v.len() as u32).to_be_bytes()[..]);
            self.output.extend_from_slice(v);
        } else {
            return Err(Error::InvalidData);
        }
        Ok(())
    }

    fn serialize_newtype_struct<T>(self, _name: &'static str, _value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        unimplemented!()
    }

    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        value: &T,
    ) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut *self)?;
        Ok(())
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq> {
        Ok(self)
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple> {
        unimplemented!()
    }

    // Tuple structs look just like sequences in JSON.
    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct> {
        unimplemented!()
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        unimplemented!()
    }

    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap> {
        // Map format with 4-byte length
        self.output.push(0xd1);
        self.offsets.push(self.output.len());
        self.output.extend_from_slice(&[0, 0, 0, 0]);
        let len = (len.unwrap() * 2) as u32;
        self.output.extend_from_slice(&len.to_be_bytes());
        Ok(self)
    }

    fn serialize_struct(self, name: &'static str, len: usize) -> Result<Self::SerializeStruct> {
        let bytes = name.as_bytes();
        assert!(bytes.len() < 256);
        assert!(len < 256);

        // Descriptor in 1-byte length string format
        self.output.push(0x00);
        self.output.push(0xa3);
        self.output.push(bytes.len() as u8);
        self.output.extend_from_slice(bytes);
        self.output.push(0xd0);

        // Variable-width type header in 4-byte length format
        self.offsets.push(self.output.len());
        self.output.extend_from_slice(&[0, 0, 0, 0]);
        self.output.extend_from_slice(&(len as u32).to_be_bytes());
        Ok(self)
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant> {
        unimplemented!()
    }
}

impl ser::SerializeSeq for &'_ mut Serializer<'_> {
    // Must match the `Ok` type of the serializer.
    type Ok = ();
    // Must match the `Error` type of the serializer.
    type Error = Error;

    // Serialize a single element of the sequence.
    fn serialize_element<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)
    }

    // Close the sequence.
    fn end(self) -> Result<()> {
        Ok(())
    }
}

impl ser::SerializeTuple for &'_ mut Serializer<'_> {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T>(&mut self, _value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        unimplemented!()
    }

    fn end(self) -> Result<()> {
        Ok(())
    }
}

impl ser::SerializeTupleStruct for &'_ mut Serializer<'_> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, _value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        unimplemented!()
    }

    fn end(self) -> Result<()> {
        Ok(())
    }
}

impl ser::SerializeTupleVariant for &'_ mut Serializer<'_> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, _value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        unimplemented!()
    }

    fn end(self) -> Result<()> {
        Ok(())
    }
}

impl ser::SerializeMap for &'_ mut Serializer<'_> {
    type Ok = ();
    type Error = Error;

    fn serialize_key<T>(&mut self, key: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        key.serialize(&mut **self)
    }

    fn serialize_value<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<()> {
        let offset = self.offsets.pop().unwrap();
        let len = (self.output.len() - offset - 4) as u32;
        let dst = &mut self.output[offset..offset + 4];
        dst.copy_from_slice(&len.to_be_bytes());
        Ok(())
    }
}

impl ser::SerializeStruct for &'_ mut Serializer<'_> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, _key: &'static str, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<()> {
        let offset = self.offsets.pop().unwrap();
        let len = (self.output.len() - offset - 4) as u32;
        let dst = &mut self.output[offset..offset + 4];
        dst.copy_from_slice(&len.to_be_bytes());
        Ok(())
    }
}

impl ser::SerializeStructVariant for &'_ mut Serializer<'_> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, _key: &'static str, _value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        unimplemented!()
    }

    fn end(self) -> Result<()> {
        Ok(())
    }
}

type Result<T> = std::result::Result<T, Error>;
