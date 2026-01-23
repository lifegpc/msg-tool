//! Favorite HCB script (.hcb)
use super::disasm::*;
use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::encoding::*;
use anyhow::Result;

#[derive(Debug)]
/// Favorite HCB script builder
pub struct HcbScriptBuilder {}

impl HcbScriptBuilder {
    /// Create a new HcbScriptBuilder
    pub fn new() -> Self {
        Self {}
    }
}

impl ScriptBuilder for HcbScriptBuilder {
    fn default_encoding(&self) -> Encoding {
        Encoding::Cp932
    }

    fn build_script(
        &self,
        buf: Vec<u8>,
        _filename: &str,
        encoding: Encoding,
        _archive_encoding: Encoding,
        config: &ExtraConfig,
        _archive: Option<&Box<dyn Script>>,
    ) -> Result<Box<dyn Script>> {
        Ok(Box::new(HcbScript::new(buf, encoding, config)?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["hcb"]
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::Favorite
    }
}

#[derive(Debug)]
pub struct HcbScript {
    data: Data,
    reader: MemReader,
    custom_yaml: bool,
    filter_ascii: bool,
    encoding: Encoding,
}

impl HcbScript {
    pub fn new(buf: Vec<u8>, encoding: Encoding, config: &ExtraConfig) -> Result<Self> {
        let reader = MemReader::new(buf);
        let data = Data::disasm(reader.to_ref(), encoding)?;
        Ok(Self {
            data,
            reader,
            custom_yaml: config.custom_yaml,
            filter_ascii: config.favorite_hcb_filter_ascii,
            encoding,
        })
    }
}

impl Script for HcbScript {
    fn default_output_script_type(&self) -> OutputScriptType {
        OutputScriptType::Json
    }

    fn default_format_type(&self) -> FormatOptions {
        FormatOptions::None
    }

    fn is_output_supported(&self, _: OutputScriptType) -> bool {
        true
    }

    fn custom_output_extension<'a>(&'a self) -> &'a str {
        if self.custom_yaml { "yaml" } else { "json" }
    }

    fn extract_messages(&self) -> Result<Vec<Message>> {
        let messages = self
            .data
            .extract_messages(self.filter_ascii)
            .into_iter()
            .map(|(speaker, message)| Message::new(message, speaker))
            .collect();
        Ok(messages)
    }

    fn import_messages<'a>(
        &'a self,
        messages: Vec<Message>,
        file: Box<dyn WriteSeek + 'a>,
        _filename: &str,
        encoding: Encoding,
        replacement: Option<&'a ReplacementTable>,
    ) -> Result<()> {
        let mut mess = messages.iter();
        let mut mes = mess.next();
        let mut patcher =
            BinaryPatcher::new(self.reader.to_ref(), file, |pos| Ok(pos), |pos| Ok(pos))?;
        let mut need_pacth_addresses = Vec::new();
        for funcs in [&self.data.functions, &self.data.main_script] {
            for func in funcs {
                let mut cur_pos = func.pos + 1;
                if matches!(func.opcode, 0x02 | 0x06 | 0x07) {
                    need_pacth_addresses.push(cur_pos);
                }
                for operand in &func.operands {
                    if let Operand::S(s) = operand {
                        if self.filter_ascii && s.chars().all(|c| c.is_ascii()) {
                            continue;
                        }
                        let m = match mes {
                            Some(m) => m,
                            None => {
                                return Err(anyhow::anyhow!(
                                    "Not enough messages to import. Missing message: {}",
                                    s
                                ));
                            }
                        };
                        let mut message = m.message.clone();
                        if let Some(table) = replacement {
                            for (k, v) in &table.map {
                                message = message.replace(k, v);
                            }
                        }
                        patcher.copy_up_to(cur_pos)?;
                        let ori_len = operand.len(self.encoding)? as u64;
                        let mut s = encode_string(encoding, &message, true)?;
                        s.push(0); // null-terminated
                        let len = s.len();
                        if len > 255 {
                            return Err(anyhow::anyhow!(
                                "Message too long to import (max 255 bytes): {}",
                                message
                            ));
                        }
                        patcher.replace_bytes_with_write(ori_len, |writer| {
                            writer.write_u8(len as u8)?;
                            writer.write_all(&s)?;
                            Ok(())
                        })?;
                        mes = mess.next();
                    }
                    cur_pos += operand.len(self.encoding)? as u64;
                }
            }
        }
        patcher.copy_up_to(self.reader.data.len() as u64)?;
        for addr in need_pacth_addresses {
            patcher.patch_u32_address(addr)?;
        }
        Ok(())
    }

    fn custom_export(&self, filename: &std::path::Path, encoding: Encoding) -> Result<()> {
        let s = if self.custom_yaml {
            serde_yaml_ng::to_string(&self.data)?
        } else {
            serde_json::to_string_pretty(&self.data)?
        };
        let e = encode_string(encoding, &s, false)?;
        let mut writer = crate::utils::files::write_file(filename)?;
        writer.write_all(&e)?;
        Ok(())
    }
}
