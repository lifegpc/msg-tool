use crate::ext::io::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::encoding::{decode_to_string, encode_string};
use anyhow::Result;
use emote_psb::types::PsbValue;
use emote_psb::types::collection::PsbObject;
use emote_psb::{PsbReader, PsbWriter, VirtualPsb};
use json::JsonValue;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::io::{Read, Seek, Write};
use std::path::Path;

trait JsonExt {
    fn is_valid_str(&self) -> bool;
}

impl JsonExt for JsonValue {
    fn is_valid_str(&self) -> bool {
        self.is_string() || self.is_null()
    }
}

#[derive(Debug)]
pub struct ScnScriptBuilder {}

impl ScnScriptBuilder {
    pub fn new() -> Self {
        Self {}
    }
}

impl ScriptBuilder for ScnScriptBuilder {
    fn default_encoding(&self) -> Encoding {
        Encoding::Utf8
    }

    fn build_script(
        &self,
        buf: Vec<u8>,
        filename: &str,
        _encoding: Encoding,
        _archive_encoding: Encoding,
        config: &ExtraConfig,
    ) -> Result<Box<dyn Script>> {
        Ok(Box::new(ScnScript::new(
            MemReader::new(buf),
            filename,
            config,
        )?))
    }

    fn build_script_from_file(
        &self,
        filename: &str,
        _encoding: Encoding,
        _archive_encoding: Encoding,
        config: &ExtraConfig,
    ) -> Result<Box<dyn Script>> {
        if filename == "-" {
            let data = crate::utils::files::read_file(filename)?;
            Ok(Box::new(ScnScript::new(
                MemReader::new(data),
                filename,
                config,
            )?))
        } else {
            let f = std::fs::File::open(filename)?;
            let reader = std::io::BufReader::new(f);
            Ok(Box::new(ScnScript::new(reader, filename, config)?))
        }
    }

    fn build_script_from_reader(
        &self,
        reader: Box<dyn ReadSeek>,
        filename: &str,
        _encoding: Encoding,
        _archive_encoding: Encoding,
        config: &ExtraConfig,
    ) -> Result<Box<dyn Script>> {
        Ok(Box::new(ScnScript::new(reader, filename, config)?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["scn"]
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::KirikiriScn
    }

    fn is_this_format(&self, filename: &str, buf: &[u8], buf_len: usize) -> Option<u8> {
        if Path::new(filename)
            .file_name()
            .map(|name| {
                name.to_ascii_lowercase()
                    .to_string_lossy()
                    .ends_with(".ks.scn")
            })
            .unwrap_or(false)
            && buf_len >= 4
            && buf.starts_with(b"PSB\0")
        {
            return Some(255);
        }
        None
    }
}

#[derive(Debug)]
pub struct ScnScript {
    psb: VirtualPsb,
    language_index: usize,
    export_comumode: bool,
    filename: String,
}

impl ScnScript {
    pub fn new<R: Read + Seek>(reader: R, filename: &str, config: &ExtraConfig) -> Result<Self> {
        let mut psb = PsbReader::open_psb(reader)
            .map_err(|e| anyhow::anyhow!("Failed to open PSB from {}: {:?}", filename, e))?;
        let psb = psb
            .load()
            .map_err(|e| anyhow::anyhow!("Failed to load PSB from {}: {:?}", filename, e))?;
        Ok(Self {
            psb,
            language_index: config.kirikiri_language_index.unwrap_or(0),
            export_comumode: config.kirikiri_export_comumode,
            filename: filename.to_string(),
        })
    }
}

#[derive(Debug, Serialize)]
pub struct PsbDataRef<'a> {
    pub version: u16,
    pub encryption: u16,
    pub root: &'a emote_psb::types::collection::PsbObject,
}

impl<'a> PsbDataRef<'a> {
    pub fn new(psb: &'a VirtualPsb) -> Self {
        let header = psb.header();
        Self {
            version: header.version,
            encryption: header.encryption,
            root: psb.root(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct PsbData {
    pub version: u16,
    pub encryption: u16,
    pub root: emote_psb::types::collection::PsbObject,
}

impl PsbData {
    pub fn header(&self) -> emote_psb::header::PsbHeader {
        emote_psb::header::PsbHeader {
            version: self.version,
            encryption: self.encryption,
        }
    }
}

impl Script for ScnScript {
    fn default_output_script_type(&self) -> OutputScriptType {
        OutputScriptType::Json
    }

    fn default_format_type(&self) -> FormatOptions {
        FormatOptions::None
    }

    fn is_output_supported(&self, _: OutputScriptType) -> bool {
        true
    }

    fn extract_messages(&self) -> Result<Vec<Message>> {
        let mut messages = Vec::new();
        let root = self.psb.root();
        let scenes = root
            .get_value("scenes".into())
            .ok_or(anyhow::anyhow!("scenes not found"))?;
        let scenes = match scenes {
            PsbValue::List(list) => list,
            _ => return Err(anyhow::anyhow!("scenes is not a list")),
        };
        let mut comu = if self.export_comumode {
            Some(ExportComuMes::new())
        } else {
            None
        };
        for (i, oscene) in scenes.iter().enumerate() {
            let scene = match oscene {
                PsbValue::Object(obj) => obj,
                _ => return Err(anyhow::anyhow!("scene at index {} is not an object", i)),
            };
            if let Some(PsbValue::List(texts)) = scene.get_value("texts".into()) {
                for (j, text) in texts.iter().enumerate() {
                    if let PsbValue::List(text) = text {
                        let values = text.values();
                        if values.len() <= 1 {
                            continue; // Skip if there are not enough values
                        }
                        let name = &values[0];
                        let name = match name {
                            PsbValue::String(s) => Some(s),
                            PsbValue::Null => None,
                            PsbValue::None => None,
                            _ => return Err(anyhow::anyhow!("name is not a string or null")),
                        };
                        let mut display_name;
                        let mut message;
                        if matches!(values[1], PsbValue::List(_)) {
                            display_name = None;
                            message = &values[1];
                        } else {
                            if values.len() <= 2 {
                                continue; // Skip if there is no message
                            }
                            display_name = match &values[1] {
                                PsbValue::String(s) => Some(s),
                                PsbValue::Null => None,
                                PsbValue::None => None,
                                _ => {
                                    return Err(anyhow::anyhow!(
                                        "display name is not a string or null at {i},{j}"
                                    ));
                                }
                            };
                            message = &values[2];
                        }
                        if matches!(message, PsbValue::List(_)) {
                            let tmp = message;
                            if let PsbValue::List(list) = tmp {
                                if list.len() > self.language_index {
                                    if let PsbValue::List(data) =
                                        &list.values()[self.language_index]
                                    {
                                        if data.len() >= 2 {
                                            let data = data.values();
                                            display_name = match &data[0] {
                                                PsbValue::String(s) => Some(s),
                                                PsbValue::Null => None,
                                                PsbValue::None => None,
                                                _ => {
                                                    return Err(anyhow::anyhow!(
                                                        "display name is not a string or null at {i},{j}"
                                                    ));
                                                }
                                            };
                                            message = &data[1];
                                        }
                                    }
                                }
                            }
                        }
                        if let PsbValue::String(message) = message {
                            match name {
                                Some(name) => {
                                    let name = match display_name {
                                        Some(name) => name.string(),
                                        None => name.string(),
                                    };
                                    let message = message.string();
                                    messages.push(Message {
                                        name: Some(name.to_string()),
                                        message: message.replace("\\n", "\n"),
                                    });
                                }
                                None => {
                                    let message = message.string();
                                    messages.push(Message {
                                        name: None,
                                        message: message.replace("\\n", "\n"),
                                    });
                                }
                            }
                        }
                    }
                }
            }
            if let Some(PsbValue::List(selects)) = scene.get_value("selects".into()) {
                for select in selects.iter() {
                    if let PsbValue::Object(select) = select {
                        let mut text = None;
                        if let Some(PsbValue::List(language)) = select.get_value("language".into())
                        {
                            if language.len() > self.language_index {
                                let v = &language.values()[self.language_index];
                                if let PsbValue::Object(v) = v {
                                    text = match v.get_value("text".into()) {
                                        Some(PsbValue::String(s)) => Some(s),
                                        Some(PsbValue::Null) => None,
                                        Some(PsbValue::None) => None,
                                        None => None,
                                        _ => {
                                            return Err(anyhow::anyhow!(
                                                "select text is not a string or null"
                                            ));
                                        }
                                    }
                                }
                            }
                        }
                        if text.is_none() {
                            text = match select.get_value("text".into()) {
                                Some(PsbValue::String(s)) => Some(s),
                                Some(PsbValue::Null) => None,
                                Some(PsbValue::None) => None,
                                None => None,
                                _ => {
                                    return Err(anyhow::anyhow!(
                                        "select text is not a string or null"
                                    ));
                                }
                            };
                        }
                        if let Some(text) = text {
                            let text = text.string();
                            messages.push(Message {
                                name: None,
                                message: text.replace("\\n", "\n"),
                            });
                        }
                    }
                }
            }
            comu.as_mut().map(|c| c.export(&oscene));
        }
        if let Some(comu) = comu {
            if !comu.messages.is_empty() {
                let mut pb = std::path::PathBuf::from(&self.filename);
                let filename = pb
                    .file_stem()
                    .map(|s| s.to_string_lossy())
                    .unwrap_or(std::borrow::Cow::from("comumode"));
                pb.set_file_name(format!("{}_comumode.json", filename));
                match std::fs::File::create(&pb) {
                    Ok(mut f) => {
                        let messages: Vec<String> = comu.messages.into_iter().collect();
                        if let Err(e) = serde_json::to_writer_pretty(&mut f, &messages) {
                            eprintln!("Failed to write COMU messages to {}: {:?}", pb.display(), e);
                            crate::COUNTER.inc_warning();
                        }
                    }
                    Err(e) => {
                        eprintln!(
                            "Failed to create COMU messages file {}: {:?}",
                            pb.display(),
                            e
                        );
                        crate::COUNTER.inc_warning();
                    }
                }
            }
        }
        Ok(messages)
    }

    fn import_messages<'a>(
        &'a self,
        messages: Vec<Message>,
        file: Box<dyn WriteSeek + 'a>,
        _encoding: Encoding,
        replacement: Option<&'a ReplacementTable>,
    ) -> Result<()> {
        let mut mes = messages.iter();
        let mut cur_mes = mes.next();
        // We use json library to process the PSB data, because emote-psb does not support update data.
        let t = serde_json::to_string(&self.psb.root())?;
        let mut root = json::parse(&t)?;
        let scenes = &mut root["scenes"];
        if !scenes.is_array() {
            return Err(anyhow::anyhow!("scenes is not an array"));
        }
        for (i, scene) in scenes.members_mut().enumerate() {
            if !scene.is_object() {
                return Err(anyhow::anyhow!("scene at {} is not an object", i));
            }
            for text in scene["texts"].members_mut() {
                if text.is_array() {
                    if text.len() <= 1 {
                        continue; // Skip if there are not enough values
                    }
                    if cur_mes.is_none() {
                        cur_mes = mes.next();
                    }
                    if !text[0].is_valid_str() {
                        return Err(anyhow::anyhow!("name is not a string or null"));
                    }
                    let has_name = text[0].is_string();
                    let mut has_display_name;
                    if text[1].is_array() {
                        if text[1].is_string() {
                            let m = match cur_mes.take() {
                                Some(m) => m,
                                None => {
                                    return Err(anyhow::anyhow!("No enough messages."));
                                }
                            };
                            if has_name {
                                if let Some(name) = &m.name {
                                    let mut name = name.clone();
                                    if let Some(replacement) = replacement {
                                        for (key, value) in replacement.map.iter() {
                                            name = name.replace(key, value);
                                        }
                                    }
                                    text[0] = json::JsonValue::String(name);
                                } else {
                                    return Err(anyhow::anyhow!("Name is missing for message."));
                                }
                            }
                            let mut message = m.message.clone();
                            if let Some(replacement) = replacement {
                                for (key, value) in replacement.map.iter() {
                                    message = message.replace(key, value);
                                }
                            }
                            text[1] = json::JsonValue::String(message.replace("\n", "\\n"));
                        } else if text[1].is_array() {
                            if text[1].len() > self.language_index
                                && text[1][self.language_index].is_array()
                                && text[1][self.language_index].len() >= 2
                            {
                                if !text[1][self.language_index][0].is_valid_str() {
                                    return Err(anyhow::anyhow!(
                                        "display name is not a string or null"
                                    ));
                                }
                                has_display_name = text[1][self.language_index][0].is_string();
                                if text[1][self.language_index][1].is_string() {
                                    let m = match cur_mes.take() {
                                        Some(m) => m,
                                        None => {
                                            return Err(anyhow::anyhow!("No enough messages."));
                                        }
                                    };
                                    if has_name {
                                        if let Some(name) = &m.name {
                                            let mut name = name.clone();
                                            if let Some(replacement) = replacement {
                                                for (key, value) in replacement.map.iter() {
                                                    name = name.replace(key, value);
                                                }
                                            }
                                            if has_display_name {
                                                text[1][self.language_index][0] =
                                                    json::JsonValue::String(name);
                                            } else {
                                                text[0] = json::JsonValue::String(name);
                                            }
                                        } else {
                                            return Err(anyhow::anyhow!(
                                                "Name is missing for message."
                                            ));
                                        }
                                    }
                                    let mut message = m.message.clone();
                                    if let Some(replacement) = replacement {
                                        for (key, value) in replacement.map.iter() {
                                            message = message.replace(key, value);
                                        }
                                    }
                                    text[1][self.language_index][1] =
                                        json::JsonValue::String(message.replace("\n", "\\n"));
                                }
                            }
                        }
                    } else {
                        if text.len() <= 2 {
                            continue; // Skip if there is no message
                        }
                        if !text[1].is_valid_str() {
                            return Err(anyhow::anyhow!("display name is not a string or null"));
                        }
                        has_display_name = text[1].is_string();
                        if text[2].is_string() {
                            let m = match cur_mes.take() {
                                Some(m) => m,
                                None => {
                                    return Err(anyhow::anyhow!("No enough messages."));
                                }
                            };
                            if has_name {
                                if let Some(name) = &m.name {
                                    let mut name = name.clone();
                                    if let Some(replacement) = replacement {
                                        for (key, value) in replacement.map.iter() {
                                            name = name.replace(key, value);
                                        }
                                    }
                                    if has_display_name {
                                        text[1] = json::JsonValue::String(name);
                                    } else {
                                        text[0] = json::JsonValue::String(name);
                                    }
                                } else {
                                    return Err(anyhow::anyhow!("Name is missing for message."));
                                }
                            }
                            let mut message = m.message.clone();
                            if let Some(replacement) = replacement {
                                for (key, value) in replacement.map.iter() {
                                    message = message.replace(key, value);
                                }
                            }
                            text[2] = json::JsonValue::String(message.replace("\n", "\\n"));
                        } else if text[2].is_array() {
                            if text[2].len() > self.language_index
                                && text[2][self.language_index].is_array()
                                && text[2][self.language_index].len() >= 2
                            {
                                if !text[2][self.language_index][0].is_valid_str() {
                                    return Err(anyhow::anyhow!(
                                        "display name is not a string or null"
                                    ));
                                }
                                has_display_name = text[2][self.language_index][0].is_string();
                                if text[2][self.language_index][1].is_string() {
                                    let m = match cur_mes.take() {
                                        Some(m) => m,
                                        None => {
                                            return Err(anyhow::anyhow!("No enough messages."));
                                        }
                                    };
                                    if has_name {
                                        if let Some(name) = &m.name {
                                            let mut name = name.clone();
                                            if let Some(replacement) = replacement {
                                                for (key, value) in replacement.map.iter() {
                                                    name = name.replace(key, value);
                                                }
                                            }
                                            if has_display_name {
                                                text[2][self.language_index][0] =
                                                    json::JsonValue::String(name);
                                            } else {
                                                text[0] = json::JsonValue::String(name);
                                            }
                                        } else {
                                            return Err(anyhow::anyhow!(
                                                "Name is missing for message."
                                            ));
                                        }
                                    }
                                    let mut message = m.message.clone();
                                    if let Some(replacement) = replacement {
                                        for (key, value) in replacement.map.iter() {
                                            message = message.replace(key, value);
                                        }
                                    }
                                    text[2][self.language_index][1] =
                                        json::JsonValue::String(message.replace("\n", "\\n"));
                                }
                            }
                        }
                    }
                }
            }
            // #TODO: selects and comumode
        }
        if cur_mes.is_some() || mes.next().is_some() {
            return Err(anyhow::anyhow!("Some messages were not processed."));
        }
        let s = json::stringify(root);
        let obj = serde_json::from_str::<PsbObject>(&s)?;
        let oheader = self.psb.header();
        let header = emote_psb::header::PsbHeader {
            version: oheader.version,
            encryption: oheader.encryption,
        };
        let psb = VirtualPsb::new(header, Vec::new(), Vec::new(), obj);
        let writer = PsbWriter::new(psb, file);
        writer
            .finish()
            .map_err(|e| anyhow::anyhow!("Failed to write PSB: {:?}", e))?;
        Ok(())
    }

    fn custom_output_extension(&self) -> &'static str {
        "json"
    }

    fn custom_export(&self, filename: &Path, encoding: Encoding) -> Result<()> {
        if !self.psb.resources().is_empty() {
            eprintln!(
                "Warning: The PSB contains resources, which may not be fully represented in the JSON output."
            );
            crate::COUNTER.inc_warning();
        }
        if !self.psb.extra().is_empty() {
            eprintln!(
                "Warning: The PSB contains extra data, which may not be fully represented in the JSON output."
            );
            crate::COUNTER.inc_warning();
        }
        let psb_data = PsbDataRef::new(&self.psb);
        let str = serde_json::to_string_pretty(&psb_data)?;
        let s = encode_string(encoding, &str, false)?;
        let mut f = std::fs::File::create(filename)?;
        f.write_all(&s)?;
        Ok(())
    }

    fn custom_import<'a>(
        &'a self,
        custom_filename: &'a str,
        file: Box<dyn WriteSeek + 'a>,
        encoding: Encoding,
        _output_encoding: Encoding,
    ) -> Result<()> {
        let data = crate::utils::files::read_file(custom_filename)?;
        let s = decode_to_string(encoding, &data)?;
        let psb_data: PsbData = serde_json::from_str(&s)?;
        let psb = VirtualPsb::new(psb_data.header(), Vec::new(), Vec::new(), psb_data.root);
        let writer = PsbWriter::new(psb, file);
        writer
            .finish()
            .map_err(|e| anyhow::anyhow!("Failed to write PSB: {:?}", e))?;
        Ok(())
    }
}

#[derive(Debug)]
struct ExportComuMes {
    pub messages: HashSet<String>,
}

impl ExportComuMes {
    pub fn new() -> Self {
        Self {
            messages: HashSet::new(),
        }
    }

    pub fn export(&mut self, value: &PsbValue) {
        match value {
            PsbValue::Object(obj) => {
                for (k, v) in obj.iter() {
                    if k == "comumode" {
                        if let PsbValue::List(list) = v {
                            for item in list.iter() {
                                if let PsbValue::Object(obj) = item {
                                    if let Some(PsbValue::String(s)) = obj.get_value("text".into())
                                    {
                                        self.messages.insert(s.string().replace("\\n", "\n"));
                                    }
                                }
                            }
                        }
                    } else {
                        self.export(v);
                    }
                }
            }
            PsbValue::List(list) => {
                let list = list.values();
                if list.len() > 1 {
                    if let PsbValue::String(s) = &list[0] {
                        if s.string() == "comumode" {
                            for i in 1..list.len() {
                                if let PsbValue::String(s) = &list[i - 1] {
                                    if s.string() == "text" {
                                        if let PsbValue::String(text) = &list[i] {
                                            self.messages
                                                .insert(text.string().replace("\\n", "\n"));
                                        }
                                    }
                                }
                            }
                            return;
                        }
                    }
                }
                for item in list {
                    self.export(item);
                }
            }
            _ => {}
        }
    }
}
