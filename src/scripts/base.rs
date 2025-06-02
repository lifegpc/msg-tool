use crate::types::*;
use anyhow::Result;

pub trait ScriptBuilder: std::fmt::Debug {
    fn default_encoding(&self) -> Encoding;

    fn default_patched_encoding(&self) -> Encoding {
        self.default_encoding()
    }

    fn build_script(
        &self,
        buf: Vec<u8>,
        encoding: Encoding,
        config: &ExtraConfig,
    ) -> Result<Box<dyn Script>>;

    fn extensions(&self) -> &'static [&'static str];

    fn is_this_format(&self, _filename: &str, _buf: &[u8], _buf_len: usize) -> Option<u8> {
        None
    }

    fn script_type(&self) -> &'static ScriptType;
}

pub trait ArchiveContent {
    fn name(&self) -> &str;
    fn data(&self) -> &[u8];
    fn is_script(&self) -> bool;
}

pub trait Script: std::fmt::Debug {
    fn default_output_script_type(&self) -> OutputScriptType;

    fn default_format_type(&self) -> FormatOptions;

    fn extract_messages(&self) -> Result<Vec<Message>> {
        if !self.is_archive() {
            return Err(anyhow::anyhow!(
                "This script type does not support extracting messages."
            ));
        }
        Ok(vec![])
    }

    fn import_messages(
        &self,
        _messages: Vec<Message>,
        _filename: &str,
        _encoding: Encoding,
        _replacement: Option<&ReplacementTable>,
    ) -> Result<()> {
        if !self.is_archive() {
            return Err(anyhow::anyhow!(
                "This script type does not support importing messages."
            ));
        }
        Ok(())
    }

    fn is_archive(&self) -> bool {
        false
    }

    fn iter_archive<'a>(
        &'a self,
    ) -> Result<Box<dyn Iterator<Item = Result<Box<dyn ArchiveContent>>> + 'a>> {
        Ok(Box::new(std::iter::empty()))
    }
}
