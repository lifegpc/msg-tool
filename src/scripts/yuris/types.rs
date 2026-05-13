use crate::ext::io::*;
use crate::types::*;
use crate::utils::encoding::*;
use crate::utils::struct_pack::*;
use anyhow::Result;
use msg_tool_macro::*;
use serde::{Deserialize, Serialize};
use std::io::{Read, Seek, Write};

#[derive(Debug, StructUnpack, StructPack, Deserialize, Serialize)]
pub struct ArgumentMeta {
    #[cstring]
    pub name: String,
    pub data: u16,
}

#[derive(Debug, StructPack, StructUnpack, Deserialize, Serialize)]
pub struct CodeMeta {
    #[cstring]
    pub name: String,
    #[pvec(u8)]
    pub arguments: Vec<ArgumentMeta>,
}
