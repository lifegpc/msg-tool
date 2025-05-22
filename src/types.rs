use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Copy, Clone, Serialize, Deserialize, Debug)]
#[serde(untagged, rename_all = "camelCase")]
/// Text Encoding
pub enum Encoding {
    /// Automatically detect encoding
    Auto,
    /// UTF-8 encoding
    Utf8,
    /// Shift-JIS encoding
    Cp932,
    /// GB2312 encoding
    Gb2312,
    /// Code page encoding (Windows only)
    #[cfg(windows)]
    CodePage(u32),
}

impl Default for Encoding {
    fn default() -> Self {
        Encoding::Utf8
    }
}

impl Encoding {
    pub fn is_jis(&self) -> bool {
        match self {
            Self::Cp932 => true,
            #[cfg(windows)]
            Self::CodePage(code_page) => *code_page == 932,
            _ => false,
        }
    }
}

#[derive(Clone, Copy, Debug, ValueEnum, PartialEq, Eq, PartialOrd, Ord)]
/// Text Encoding
pub enum TextEncoding {
    /// Use script's default encoding
    Default,
    /// Automatically detect encoding
    Auto,
    /// UTF-8 encoding
    Utf8,
    /// Shift-JIS encoding
    Cp932,
    /// GB2312 encoding
    Gb2312,
}

#[derive(Clone, Copy, Debug, ValueEnum, PartialEq, Eq, PartialOrd, Ord)]
/// Script type
pub enum OutputScriptType {
    /// Text script
    M3t,
    /// JSON which can be used for GalTransl
    Json,
}

impl AsRef<str> for OutputScriptType {
    fn as_ref(&self) -> &str {
        match self {
            OutputScriptType::M3t => "m3t",
            OutputScriptType::Json => "json",
        }
    }
}

#[derive(Clone, Copy, Debug, ValueEnum, PartialEq, Eq, PartialOrd, Ord)]
pub enum CircusMesType {
    /// fortissimo//Akkord:Bsusvier
    Ffexa,
    /// fortissimo EXS//Akkord:nächsten Phase
    Ffexs,
    /// Eternal Fantasy
    Ef,
    /// D.C.〜ダ・カーポ〜　温泉編
    Dcos,
    /// ことり Love Ex P
    Ktlep,
    /// D.C.WhiteSeason
    Dcws,
    /// D.C. Summer Vacation
    Dcsv,
    /// Ｄ．Ｃ．Ｐ．Ｃ．(Vista)
    Dcpc,
    /// D.C.〜ダ・カーポ〜　MEMORIES DISC
    Dcmems,
    /// D.C. Dream X’mas
    Dcdx,
    /// D.C.A.S. 〜ダ・カーポ〜アフターシーズンズ
    Dcas,
    /// D.C.II 春風のアルティメットバトル！
    Dcbs,
    /// D.C.II Fall in Love
    Dc2fl,
    /// D.C.II 春風のアルティメットバトル！
    Dc2bs,
    /// D.C.II Dearest Marriage
    Dc2dm,
    /// D.C.II 〜featuring　Yun2〜
    Dc2fy,
    /// D.C.II C.C. 月島小恋のらぶらぶバスルーム
    Dc2cckko,
    /// D.C.II C.C. 音姫先生のどきどき特別授業
    Dc2ccotm,
    /// D.C.II Spring Celebration
    Dc2sc,
    /// D.C.II To You
    Dc2ty,
    /// D.C.II P.C.
    Dc2pc,
    /// D.C.III RX-rated
    Dc3rx,
    /// D.C.III P.P.～ダ・カーポIII プラチナパートナー～
    Dc3pp,
    /// D.C.III WithYou
    Dc3wy,
    /// D.C.III DreamDays
    Dc3dd,
    /// D.C.4 ～ダ・カーポ4～
    Dc4,
    /// D.C.4 Plus Harmony 〜ダ・カーポ4〜 プラスハーモニー
    Dc4ph,
    /// D.S. -Dal Segno-
    Ds,
    /// D.S.i.F. -Dal Segno- in Future
    Dsif,
    /// てんぷれ！
    Tmpl,
    /// 百花百狼/Hyakka Hyakurou
    Nightshade,
}

impl AsRef<str> for CircusMesType {
    fn as_ref(&self) -> &str {
        match self {
            CircusMesType::Ffexa => "ffexa",
            CircusMesType::Ffexs => "ffexs",
            CircusMesType::Ef => "ef",
            CircusMesType::Dcos => "dcos",
            CircusMesType::Ktlep => "ktlep",
            CircusMesType::Dcws => "dcws",
            CircusMesType::Dcsv => "dcsv",
            CircusMesType::Dcpc => "dcpc",
            CircusMesType::Dcmems => "dcmems",
            CircusMesType::Dcdx => "dcdx",
            CircusMesType::Dcas => "dcas",
            CircusMesType::Dcbs => "dcbs",
            CircusMesType::Dc2fl => "dc2fl",
            CircusMesType::Dc2bs => "dc2bs",
            CircusMesType::Dc2dm => "dc2dm",
            CircusMesType::Dc2fy => "dc2fy",
            CircusMesType::Dc2cckko => "dc2cckko",
            CircusMesType::Dc2ccotm => "dc2ccotm",
            CircusMesType::Dc2sc => "dc2sc",
            CircusMesType::Dc2ty => "dc2ty",
            CircusMesType::Dc2pc => "dc2pc",
            CircusMesType::Dc3rx => "dc3rx",
            CircusMesType::Dc3pp => "dc3pp",
            CircusMesType::Dc3wy => "dc3wy",
            CircusMesType::Dc3dd => "dc3dd",
            CircusMesType::Dc4 => "dc4",
            CircusMesType::Dc4ph => "dc4ph",
            CircusMesType::Ds => "ds",
            CircusMesType::Dsif => "dsif",
            CircusMesType::Tmpl => "tmpl",
            CircusMesType::Nightshade => "nightshade",
        }
    }
}

pub struct ExtraConfig {
    pub circus_mes_type: Option<CircusMesType>,
}

#[derive(Clone, Copy, Debug, ValueEnum, PartialEq, Eq, PartialOrd, Ord)]
/// Script type
pub enum ScriptType {
    /// Circus MES script
    Circus,
    #[value(alias("ethornell"))]
    /// Buriko General Interpreter/Ethornell Script
    BGI,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Message {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub message: String,
}

impl Message {
    pub fn new(message: String, name: Option<String>) -> Self {
        Message { message, name }
    }
}

pub enum ScriptResult {
    Ok,
    Ignored,
}

#[derive(Clone, Copy, Debug, ValueEnum, PartialEq, Eq, PartialOrd, Ord)]
/// Format type
pub enum FormatType {
    /// Wrap line with fixed length
    Fixed,
    /// Do not wrap line
    None,
}

/// Format options
pub enum FormatOptions {
    /// Wrap line with fixed length
    Fixed {
        /// Fixed length
        length: usize,
        /// Whether to keep original line breaks
        keep_original: bool,
    },
    /// Do not wrap line
    None,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NameTableCell {
    #[serde(rename = "JP_Name")]
    pub jp_name: String,
    #[serde(rename = "CN_Name")]
    pub cn_name: String,
    #[serde(rename = "Count")]
    pub count: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReplacementTable {
    #[serde(flatten)]
    pub map: HashMap<String, String>,
}
