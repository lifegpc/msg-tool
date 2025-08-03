use crate::types::Encoding;
use anyhow::Result;
use msg_tool_macro::struct_unpack_impl_for_num;
use std::io::{Read, Seek, Write};

pub trait StructUnpack: Sized {
    fn unpack<R: Read + Seek>(reader: R, big: bool, encoding: Encoding) -> Result<Self>;
}

pub trait StructPack: Sized {
    fn pack<W: Write>(&self, writer: &mut W, big: bool, encoding: Encoding) -> Result<()>;
}

impl<T: StructPack> StructPack for Vec<T> {
    fn pack<W: Write>(&self, writer: &mut W, big: bool, encoding: Encoding) -> Result<()> {
        for item in self {
            item.pack(writer, big, encoding)?;
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
    fn unpack<R: Read + Seek>(mut reader: R, _big: bool, _encoding: Encoding) -> Result<Self> {
        let mut buf = [0u8; 1];
        reader.read_exact(&mut buf)?;
        Ok(buf[0] != 0)
    }
}

impl StructPack for bool {
    fn pack<W: Write>(&self, writer: &mut W, _big: bool, _encoding: Encoding) -> Result<()> {
        writer.write_all(&[if *self { 1 } else { 0 }])?;
        Ok(())
    }
}

impl<T: StructPack> StructPack for Option<T> {
    fn pack<W: Write>(&self, writer: &mut W, big: bool, encoding: Encoding) -> Result<()> {
        if let Some(value) = self {
            value.pack(writer, big, encoding)?;
        }
        Ok(())
    }
}

impl<T: StructUnpack> StructUnpack for Option<T> {
    fn unpack<R: Read + Seek>(reader: R, big: bool, encoding: Encoding) -> Result<Self> {
        let value = T::unpack(reader, big, encoding)?;
        Ok(Some(value))
    }
}
