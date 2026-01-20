//! Binary Struct Packing and Unpacking Utilities
use crate::types::Encoding;
use anyhow::Result;
use msg_tool_macro::struct_unpack_impl_for_num;
use std::any::Any;
use std::io::{Read, Seek, Write};

/// Trait for unpacking a struct from a binary stream.
pub trait StructUnpack: Sized {
    /// Unpacks a struct from a binary stream.
    ///
    /// * `reader` - The reader to read the binary data from.
    /// * `big` - Whether the data is in big-endian format.
    /// * `encoding` - The encoding to use for string fields.
    /// * `info` - Additional information that may be needed for unpacking.
    fn unpack<R: Read + Seek>(
        reader: &mut R,
        big: bool,
        encoding: Encoding,
        info: &Option<Box<dyn Any>>,
    ) -> Result<Self>;
}

/// Trait for packing a struct into a binary stream.
pub trait StructPack: Sized {
    /// Packs a struct into a binary stream.
    ///
    /// * `writer` - The writer to write the binary data to.
    /// * `big` - Whether to use big-endian format.
    /// * `encoding` - The encoding to use for string fields.
    /// * `info` - Additional information that may be needed for packing.
    fn pack<W: Write>(
        &self,
        writer: &mut W,
        big: bool,
        encoding: Encoding,
        info: &Option<Box<dyn Any>>,
    ) -> Result<()>;
}

impl<T: StructPack> StructPack for Vec<T> {
    fn pack<W: Write>(
        &self,
        writer: &mut W,
        big: bool,
        encoding: Encoding,
        info: &Option<Box<dyn Any>>,
    ) -> Result<()> {
        for item in self {
            item.pack(writer, big, encoding, info)?;
        }
        Ok(())
    }
}

struct_unpack_impl_for_num!(u8);
struct_unpack_impl_for_num!(u16);
struct_unpack_impl_for_num!(u32);
struct_unpack_impl_for_num!(u64);
struct_unpack_impl_for_num!(u128);
struct_unpack_impl_for_num!(i8);
struct_unpack_impl_for_num!(i16);
struct_unpack_impl_for_num!(i32);
struct_unpack_impl_for_num!(i64);
struct_unpack_impl_for_num!(i128);
struct_unpack_impl_for_num!(f32);
struct_unpack_impl_for_num!(f64);

impl StructUnpack for bool {
    fn unpack<R: Read + Seek>(
        reader: &mut R,
        _big: bool,
        _encoding: Encoding,
        _info: &Option<Box<dyn Any>>,
    ) -> Result<Self> {
        let mut buf = [0u8; 1];
        reader.read_exact(&mut buf)?;
        Ok(buf[0] != 0)
    }
}

impl StructPack for bool {
    fn pack<W: Write>(
        &self,
        writer: &mut W,
        _big: bool,
        _encoding: Encoding,
        _info: &Option<Box<dyn Any>>,
    ) -> Result<()> {
        writer.write_all(&[if *self { 1 } else { 0 }])?;
        Ok(())
    }
}

impl<T: StructPack> StructPack for Option<T> {
    fn pack<W: Write>(
        &self,
        writer: &mut W,
        big: bool,
        encoding: Encoding,
        info: &Option<Box<dyn Any>>,
    ) -> Result<()> {
        if let Some(value) = self {
            value.pack(writer, big, encoding, info)?;
        }
        Ok(())
    }
}

impl<T: StructUnpack> StructUnpack for Option<T> {
    fn unpack<R: Read + Seek>(
        reader: &mut R,
        big: bool,
        encoding: Encoding,
        info: &Option<Box<dyn Any>>,
    ) -> Result<Self> {
        let value = T::unpack(reader, big, encoding, info)?;
        Ok(Some(value))
    }
}

impl<const T: usize> StructPack for [u8; T] {
    fn pack<W: Write>(
        &self,
        writer: &mut W,
        _big: bool,
        _encoding: Encoding,
        _info: &Option<Box<dyn Any>>,
    ) -> Result<()> {
        writer.write_all(self)?;
        Ok(())
    }
}

impl<const T: usize> StructUnpack for [u8; T] {
    fn unpack<R: Read + Seek>(
        reader: &mut R,
        _big: bool,
        _encoding: Encoding,
        _info: &Option<Box<dyn Any>>,
    ) -> Result<Self> {
        let mut buf = [0u8; T];
        reader.read_exact(&mut buf)?;
        Ok(buf)
    }
}
