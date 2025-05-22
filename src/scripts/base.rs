use crate::types::*;
use anyhow::Result;

pub trait ScriptBuilder {
    fn default_encoding(&self) -> Encoding;

    fn default_patched_encoding(&self) -> Encoding {
        self.default_encoding()
    }

    fn build_script(
        &self,
        filename: &str,
        encoding: Encoding,
        config: &ExtraConfig,
    ) -> Result<Box<dyn Script>>;

    fn extensions(&self) -> &'static [&'static str];

    fn script_type(&self) -> &'static ScriptType;
}

pub trait Script: std::fmt::Debug {
    fn default_output_script_type(&self) -> OutputScriptType;

    fn default_format_type(&self) -> FormatOptions;

    fn extract_messages(&self) -> Result<Vec<Message>>;

    fn import_messages(
        &self,
        messages: Vec<Message>,
        filename: &str,
        encoding: Encoding,
        replacement: Option<&ReplacementTable>,
    ) -> Result<()>;
}
