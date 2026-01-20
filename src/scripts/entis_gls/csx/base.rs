use crate::scripts::base::*;
use crate::types::*;
use anyhow::Result;
use std::collections::HashMap;
use std::io::Write;

pub trait ECSImage: std::fmt::Debug {
    fn disasm<'a>(&self, writer: Box<dyn Write + 'a>) -> Result<()>;
    fn export(&self) -> Result<Vec<Message>>;
    fn export_multi(&self) -> Result<HashMap<String, Vec<Message>>>;
    fn export_all(&self) -> Result<Vec<String>>;
    fn import<'a>(
        &self,
        messages: Vec<Message>,
        file: Box<dyn WriteSeek + 'a>,
        replacement: Option<&'a ReplacementTable>,
    ) -> Result<()>;
    fn import_multi<'a>(
        &self,
        messages: HashMap<String, Vec<Message>>,
        file: Box<dyn WriteSeek + 'a>,
        replacement: Option<&'a ReplacementTable>,
    ) -> Result<()>;
    fn import_all<'a>(&self, messages: Vec<String>, file: Box<dyn WriteSeek + 'a>) -> Result<()>;
}
