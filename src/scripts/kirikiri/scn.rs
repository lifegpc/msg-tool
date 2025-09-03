//! Kirikiri Scene File (.scn)
use super::mdf::Mdf;
use crate::ext::io::*;
use crate::ext::psb::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::encoding::*;
use anyhow::Result;
use emote_psb::{PsbReader, PsbWriter};
use fancy_regex::Regex;
use std::collections::{HashMap, HashSet};
use std::io::{Read, Seek};
use std::path::Path;
use std::sync::Arc;

#[derive(Debug)]
/// Kirikiri Scene Script Builder
pub struct ScnScriptBuilder {}

impl ScnScriptBuilder {
    /// Creates a new instance of `ScnScriptBuilder`
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
        _archive: Option<&Box<dyn Script>>,
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
        _archive: Option<&Box<dyn Script>>,
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
        _archive: Option<&Box<dyn Script>>,
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
                    .ends_with(".scn")
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
/// Kirikiri Scene Script
pub struct ScnScript {
    psb: VirtualPsbFixed,
    language_index: usize,
    export_chat: bool,
    filename: String,
    chat_key: Option<String>,
    chat_json: Option<Arc<HashMap<String, String>>>,
    custom_yaml: bool,
}

impl ScnScript {
    /// Creates a new `ScnScript` from the given reader and filename
    ///
    /// * `reader` - The reader containing the PSB or MDF data
    /// * `filename` - The name of the file (used for error reporting and extension detection)
    /// * `config` - Extra configuration options
    pub fn new<R: Read + Seek>(
        mut reader: R,
        filename: &str,
        config: &ExtraConfig,
    ) -> Result<Self> {
        let mut header = [0u8; 4];
        reader.read_exact(&mut header)?;
        if &header == b"mdf\0" {
            let mut data = Vec::new();
            reader.read_to_end(&mut data)?;
            let decoded = Mdf::unpack(MemReaderRef::new(&data))?;
            return Self::new(MemReader::new(decoded), filename, config);
        }
        reader.rewind()?;
        let mut psb = PsbReader::open_psb(reader)
            .map_err(|e| anyhow::anyhow!("Failed to open PSB from {}: {:?}", filename, e))?;
        let psb = psb
            .load()
            .map_err(|e| anyhow::anyhow!("Failed to load PSB from {}: {:?}", filename, e))?;
        Ok(Self {
            psb: psb.to_psb_fixed(),
            language_index: config.kirikiri_language_index.unwrap_or(0),
            export_chat: config.kirikiri_export_chat,
            filename: filename.to_string(),
            chat_key: config.kirikiri_chat_key.clone(),
            chat_json: config.kirikiri_chat_json.clone(),
            custom_yaml: config.custom_yaml,
        })
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

    fn custom_output_extension<'a>(&'a self) -> &'a str {
        if self.custom_yaml { "yaml" } else { "json" }
    }

    fn extract_messages(&self) -> Result<Vec<Message>> {
        let mut messages = Vec::new();
        let root = self.psb.root();
        let scenes = root
            .get_value("scenes")
            .ok_or(anyhow::anyhow!("scenes not found"))?;
        let scenes = match scenes {
            PsbValueFixed::List(list) => list,
            _ => return Err(anyhow::anyhow!("scenes is not a list")),
        };
        let mut comu = if self.export_chat {
            Some(ExportMes::new(
                self.chat_key.clone().unwrap_or("comumode".to_string()),
            ))
        } else {
            None
        };
        for (i, oscene) in scenes.iter().enumerate() {
            let scene = match oscene {
                PsbValueFixed::Object(obj) => obj,
                _ => return Err(anyhow::anyhow!("scene at index {} is not an object", i)),
            };
            if let Some(PsbValueFixed::List(texts)) = scene.get_value("texts") {
                for (j, text) in texts.iter().enumerate() {
                    if let PsbValueFixed::List(text) = text {
                        let values = text.values();
                        if values.len() <= 1 {
                            continue; // Skip if there are not enough values
                        }
                        let name = &values[0];
                        let name = match name {
                            PsbValueFixed::String(s) => Some(s),
                            PsbValueFixed::Null => None,
                            _ => return Err(anyhow::anyhow!("name is not a string or null")),
                        };
                        let mut display_name;
                        let mut message;
                        if matches!(values[1], PsbValueFixed::List(_)) {
                            display_name = None;
                            message = &values[1];
                        } else {
                            if values.len() <= 2 {
                                continue; // Skip if there is no message
                            }
                            display_name = match &values[1] {
                                PsbValueFixed::String(s) => Some(s),
                                PsbValueFixed::Null => None,
                                _ => {
                                    return Err(anyhow::anyhow!(
                                        "display name is not a string or null at {i},{j}"
                                    ));
                                }
                            };
                            message = &values[2];
                        }
                        if matches!(message, PsbValueFixed::List(_)) {
                            let tmp = message;
                            if let PsbValueFixed::List(list) = tmp {
                                if list.len() > self.language_index {
                                    if let PsbValueFixed::List(data) =
                                        &list.values()[self.language_index]
                                    {
                                        if data.len() >= 2 {
                                            let data = data.values();
                                            display_name = match &data[0] {
                                                PsbValueFixed::String(s) => Some(s),
                                                PsbValueFixed::Null => None,
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
                        if let PsbValueFixed::String(message) = message {
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
            if let Some(PsbValueFixed::List(selects)) = scene.get_value("selects") {
                for select in selects.iter() {
                    if let PsbValueFixed::Object(select) = select {
                        let mut text = None;
                        if let Some(PsbValueFixed::List(language)) = select.get_value("language") {
                            if language.len() > self.language_index {
                                let v = &language.values()[self.language_index];
                                if let PsbValueFixed::Object(v) = v {
                                    text = match v.get_value("text") {
                                        Some(PsbValueFixed::String(s)) => Some(s),
                                        Some(PsbValueFixed::Null) => None,
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
                            text = match select.get_value("text") {
                                Some(PsbValueFixed::String(s)) => Some(s),
                                Some(PsbValueFixed::Null) => None,
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
                    .unwrap_or(std::borrow::Cow::from(comu.key.as_str()));
                pb.set_file_name(format!("{}_{}.json", filename, comu.key));
                match std::fs::File::create(&pb) {
                    Ok(mut f) => {
                        let messages: Vec<String> = comu.messages.into_iter().collect();
                        if let Err(e) = serde_json::to_writer_pretty(&mut f, &messages) {
                            eprintln!("Failed to write chat messages to {}: {:?}", pb.display(), e);
                            crate::COUNTER.inc_warning();
                        }
                    }
                    Err(e) => {
                        eprintln!(
                            "Failed to create chat messages file {}: {:?}",
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
        _filename: &str,
        _encoding: Encoding,
        replacement: Option<&'a ReplacementTable>,
    ) -> Result<()> {
        let mut mes = messages.iter();
        let mut cur_mes = mes.next();
        let mut psb = self.psb.clone();
        let root = psb.root_mut();
        let scenes = &mut root["scenes"];
        if !scenes.is_list() {
            return Err(anyhow::anyhow!("scenes is not an array"));
        }
        let comu = self.chat_json.as_ref().map(|json| {
            ImportMes::new(
                json,
                replacement,
                self.chat_key.clone().unwrap_or("comumode".to_string()),
            )
        });
        for (i, scene) in scenes.members_mut().enumerate() {
            if !scene.is_object() {
                return Err(anyhow::anyhow!("scene at {} is not an object", i));
            }
            if scene["texts"].is_list() {
                for (j, text) in scene["texts"].members_mut().enumerate() {
                    if text.is_list() {
                        if text.len() <= 1 {
                            continue; // Skip if there are not enough values
                        }
                        if cur_mes.is_none() {
                            cur_mes = mes.next();
                        }
                        if !text[0].is_string_or_null() {
                            return Err(anyhow::anyhow!("name is not a string or null"));
                        }
                        let has_name = text[0].is_string();
                        let mut has_display_name;
                        if text[1].is_list() {
                            if text[1].len() > self.language_index
                                && text[1][self.language_index].is_list()
                                && text[1][self.language_index].len() >= 2
                            {
                                if !text[1][self.language_index][0].is_string_or_null() {
                                    return Err(anyhow::anyhow!(
                                        "display name is not a string or null"
                                    ));
                                }
                                has_display_name = text[1][self.language_index][0].is_string();
                                if text[1][self.language_index][1].is_string() {
                                    let m = match cur_mes.take() {
                                        Some(m) => m,
                                        None => {
                                            return Err(anyhow::anyhow!(
                                                "No enough messages. (text {j} at scene {i})"
                                            ));
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
                                                text[1][self.language_index][0].set_string(name);
                                            } else {
                                                text[0].set_string(name);
                                            }
                                        } else {
                                            return Err(anyhow::anyhow!(
                                                "Name is missing for message. (text {j} at scene {i})"
                                            ));
                                        }
                                    }
                                    let mut message = m.message.clone();
                                    if let Some(replacement) = replacement {
                                        for (key, value) in replacement.map.iter() {
                                            message = message.replace(key, value);
                                        }
                                    }
                                    text[1][self.language_index][1]
                                        .set_string(message.replace("\n", "\\n"));
                                    // Modify save message if exists
                                    if text[1][self.language_index][3].is_string() {
                                        text[1][self.language_index][3]
                                            .set_string(get_save_message(&message, true));
                                    }
                                    if text[1][self.language_index][4].is_string() {
                                        text[1][self.language_index][4]
                                            .set_string(get_save_message(&message, false));
                                    }
                                }
                            }
                        } else {
                            if text.len() <= 2 {
                                continue; // Skip if there is no message
                            }
                            if !text[1].is_string_or_null() {
                                return Err(anyhow::anyhow!(
                                    "display name is not a string or null"
                                ));
                            }
                            has_display_name = text[1].is_string();
                            if text[2].is_string() {
                                let m = match cur_mes.take() {
                                    Some(m) => m,
                                    None => {
                                        return Err(anyhow::anyhow!(
                                            "No enough messages.(text {j} at scene {i})"
                                        ));
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
                                            text[1].set_string(name);
                                        } else {
                                            text[0].set_string(name);
                                        }
                                    } else {
                                        return Err(anyhow::anyhow!(
                                            "Name is missing for message.(text {j} at scene {i})"
                                        ));
                                    }
                                }
                                let mut message = m.message.clone();
                                if let Some(replacement) = replacement {
                                    for (key, value) in replacement.map.iter() {
                                        message = message.replace(key, value);
                                    }
                                }
                                text[2].set_string(message.replace("\n", "\\n"));
                            } else if text[2].is_list() {
                                if text[2].len() > self.language_index
                                    && text[2][self.language_index].is_list()
                                    && text[2][self.language_index].len() >= 2
                                {
                                    if !text[2][self.language_index][0].is_string_or_null() {
                                        return Err(anyhow::anyhow!(
                                            "display name is not a string or null"
                                        ));
                                    }
                                    has_display_name = text[2][self.language_index][0].is_string();
                                    if text[2][self.language_index][1].is_string() {
                                        let m = match cur_mes.take() {
                                            Some(m) => m,
                                            None => {
                                                return Err(anyhow::anyhow!(
                                                    "No enough messages.(text {j} at scene {i})"
                                                ));
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
                                                    text[2][self.language_index][0]
                                                        .set_string(name);
                                                } else {
                                                    text[0].set_string(name);
                                                }
                                            } else {
                                                return Err(anyhow::anyhow!(
                                                    "Name is missing for message.(text {j} at scene {i})"
                                                ));
                                            }
                                        }
                                        let mut message = m.message.clone();
                                        if let Some(replacement) = replacement {
                                            for (key, value) in replacement.map.iter() {
                                                message = message.replace(key, value);
                                            }
                                        }
                                        text[2][self.language_index][1]
                                            .set_string(message.replace("\n", "\\n"));
                                        // Modify save message if exists
                                        if text[2][self.language_index][3].is_string() {
                                            text[2][self.language_index][3]
                                                .set_string(get_save_message(&message, true));
                                        }
                                        if text[2][self.language_index][4].is_string() {
                                            text[2][self.language_index][4]
                                                .set_string(get_save_message(&message, false));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if scene["selects"].is_list() {
                for select in scene["selects"].members_mut() {
                    if select.is_object() {
                        if cur_mes.is_none() {
                            cur_mes = mes.next();
                        }
                        if select["language"].is_list()
                            && select["language"].len() > self.language_index
                            && select["language"][self.language_index].is_object()
                        {
                            let lang_obj = &mut select["language"][self.language_index];
                            if lang_obj["text"].is_string() {
                                let m = match cur_mes.take() {
                                    Some(m) => m,
                                    None => {
                                        return Err(anyhow::anyhow!("No enough messages."));
                                    }
                                };
                                let mut text = m.message.clone();
                                if let Some(replacement) = replacement {
                                    for (key, value) in replacement.map.iter() {
                                        text = text.replace(key, value);
                                    }
                                }
                                lang_obj["text"].set_string(text.replace("\n", "\\n"));
                                continue;
                            }
                        }
                        if select["text"].is_string() {
                            let m = match cur_mes.take() {
                                Some(m) => m,
                                None => {
                                    return Err(anyhow::anyhow!("No enough messages."));
                                }
                            };
                            let mut text = m.message.clone();
                            if let Some(replacement) = replacement {
                                for (key, value) in replacement.map.iter() {
                                    text = text.replace(key, value);
                                }
                            }
                            select["text"].set_string(text.replace("\n", "\\n"));
                        }
                    }
                }
            }
            comu.as_ref().map(|c| c.import(scene));
        }
        if cur_mes.is_some() || mes.next().is_some() {
            return Err(anyhow::anyhow!("Some messages were not processed."));
        }
        let psb = psb.to_psb(true);
        let writer = PsbWriter::new(psb, file);
        writer.finish().map_err(|e| {
            anyhow::anyhow!("Failed to write PSB to file {}: {:?}", self.filename, e)
        })?;
        Ok(())
    }

    fn custom_export(&self, filename: &Path, encoding: Encoding) -> Result<()> {
        let s = if self.custom_yaml {
            serde_yaml_ng::to_string(&self.psb)
                .map_err(|e| anyhow::anyhow!("Failed to serialize to YAML: {}", e))?
        } else {
            json::stringify_pretty(self.psb.to_json(), 2)
        };
        let mut f = crate::utils::files::write_file(filename)?;
        let b = encode_string(encoding, &s, false)?;
        f.write_all(&b)?;
        Ok(())
    }

    fn custom_import<'a>(
        &'a self,
        custom_filename: &'a str,
        file: Box<dyn WriteSeek + 'a>,
        _encoding: Encoding,
        output_encoding: Encoding,
    ) -> Result<()> {
        let data = crate::utils::files::read_file(custom_filename)?;
        let s = decode_to_string(output_encoding, &data, true)?;
        let psb = if self.custom_yaml {
            let data: VirtualPsbFixedData = serde_yaml_ng::from_str(&s)
                .map_err(|e| anyhow::anyhow!("Failed to deserialize YAML: {}", e))?;
            let mut psb = self.psb.clone();
            psb.set_data(data);
            psb.to_psb(true)
        } else {
            let json = json::parse(&s)?;
            let mut psb = self.psb.clone();
            psb.from_json(&json)?;
            psb.to_psb(true)
        };
        let writer = PsbWriter::new(psb, file);
        writer.finish().map_err(|e| {
            anyhow::anyhow!("Failed to write PSB to file {}: {:?}", self.filename, e)
        })?;
        Ok(())
    }
}

#[derive(Debug)]
struct ExportMes {
    pub messages: HashSet<String>,
    pub key: String,
}

impl ExportMes {
    pub fn new(key: String) -> Self {
        Self {
            messages: HashSet::new(),
            key: key,
        }
    }

    pub fn export(&mut self, value: &PsbValueFixed) {
        match value {
            PsbValueFixed::Object(obj) => {
                for (k, v) in obj.iter() {
                    if k == &self.key {
                        if let PsbValueFixed::List(list) = v {
                            for item in list.iter() {
                                if let PsbValueFixed::Object(obj) = item {
                                    if let Some(PsbValueFixed::String(s)) = obj.get_value("text") {
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
            PsbValueFixed::List(list) => {
                let list = list.values();
                if list.len() > 1 {
                    if let PsbValueFixed::String(s) = &list[0] {
                        if s.string() == &self.key {
                            for i in 1..list.len() {
                                if let PsbValueFixed::String(s) = &list[i - 1] {
                                    if s.string() == "text" {
                                        if let PsbValueFixed::String(text) = &list[i] {
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

#[derive(Debug)]
struct ImportMes<'a> {
    messages: &'a Arc<HashMap<String, String>>,
    replacement: Option<&'a ReplacementTable>,
    key: String,
}

impl<'a> ImportMes<'a> {
    pub fn new(
        messages: &'a Arc<HashMap<String, String>>,
        replacement: Option<&'a ReplacementTable>,
        key: String,
    ) -> Self {
        Self {
            messages,
            replacement,
            key: key,
        }
    }

    pub fn import(&self, value: &mut PsbValueFixed) {
        match value {
            PsbValueFixed::Object(obj) => {
                for (k, v) in obj.iter_mut() {
                    if k == &self.key {
                        for obj in v.members_mut() {
                            if let Some(text) = obj["text"].as_str() {
                                if let Some(replace_text) = self.messages.get(text) {
                                    let mut text = replace_text.clone();
                                    if let Some(replacement) = self.replacement {
                                        for (key, value) in replacement.map.iter() {
                                            text = text.replace(key, value);
                                        }
                                    }
                                    obj["text"].set_string(text.replace("\n", "\\n"));
                                } else {
                                    eprintln!(
                                        "Warning: chat message '{}' not found in translation table.",
                                        text
                                    );
                                    crate::COUNTER.inc_warning();
                                }
                            }
                        }
                    } else {
                        self.import(v);
                    }
                }
            }
            PsbValueFixed::List(list) => {
                if list.len() > 1 {
                    if list[0] == self.key {
                        for i in 1..list.len() {
                            if list[i - 1] == "text" {
                                if let Some(text) = list[i].as_str() {
                                    if let Some(replace_text) = self.messages.get(text) {
                                        let mut text = replace_text.clone();
                                        if let Some(replacement) = self.replacement {
                                            for (key, value) in replacement.map.iter() {
                                                text = text.replace(key, value);
                                            }
                                        }
                                        list[i].set_string(text.replace("\n", "\\n"));
                                    } else {
                                        eprintln!(
                                            "Warning: chat message '{}' not found in translation table.",
                                            text
                                        );
                                        crate::COUNTER.inc_warning();
                                    }
                                }
                            }
                        }
                        return;
                    }
                }
                for item in list.iter_mut() {
                    self.import(item);
                }
            }
            _ => {}
        }
    }
}

lazy_static::lazy_static! {
    static ref CONTROL: Regex = Regex::new("%[^;]*;").unwrap();
    static ref RUBY: Regex = Regex::new(r"\[([^\]]*)\](.?)").unwrap();
}

fn get_save_message(s: &str, in_ruby: bool) -> String {
    let mut s = s.replace("\n", "");
    s = CONTROL.replace_all(&s, "").to_string();
    s = RUBY
        .replace_all(&s, if in_ruby { "$1" } else { "$2" })
        .to_string();
    s
}

#[test]
fn test_get_save_message() {
    let s = "%n;Test\n[ruby]测[test\\]试%ok;[ok]";
    assert_eq!(get_save_message(s, true), "Testrubytest\\ok");
    assert_eq!(get_save_message(s, false), "Test测试");
}
