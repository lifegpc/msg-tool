use crate::types::*;
use anyhow::Result;

pub trait ScriptBuilder {
    fn default_encoding(&self) -> Encoding;

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

    fn extract_messages(&self) -> Result<Vec<Message>>;
}
