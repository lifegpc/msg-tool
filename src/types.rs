//! Basic types
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
    /// Returns true if the encoding is Shift-JIS (CP932).
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
/// Text Encoding (for CLI)
pub enum TextEncoding {
    /// Use script's default encoding
    Default,
    /// Automatically detect encoding
    Auto,
    /// UTF-8 encoding
    Utf8,
    #[value(alias("jis"))]
    /// Shift-JIS encoding
    Cp932,
    #[value(alias("gbk"))]
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
    /// YAML (same as JSON, but with YAML syntax)
    Yaml,
    /// Custom output
    Custom,
}

impl OutputScriptType {
    /// Returns true if the script type is custom.
    pub fn is_custom(&self) -> bool {
        matches!(self, OutputScriptType::Custom)
    }
}

impl AsRef<str> for OutputScriptType {
    /// Returns the extension for the script type.
    fn as_ref(&self) -> &str {
        match self {
            OutputScriptType::M3t => "m3t",
            OutputScriptType::Json => "json",
            OutputScriptType::Yaml => "yaml",
            OutputScriptType::Custom => "",
        }
    }
}

#[cfg(feature = "circus")]
#[derive(Clone, Copy, Debug, ValueEnum, PartialEq, Eq, PartialOrd, Ord)]
/// Circus MES game
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

#[cfg(feature = "circus")]
impl AsRef<str> for CircusMesType {
    /// Returns the name.
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

/// Extra configuration options for the script.
#[derive(Debug, Clone, Default)]
pub struct ExtraConfig {
    #[cfg(feature = "circus")]
    /// Circus Game for circus MES script.
    pub circus_mes_type: Option<CircusMesType>,
    #[cfg(feature = "escude-arc")]
    /// Whether to use fake compression for Escude archive
    pub escude_fake_compress: bool,
    #[cfg(feature = "escude")]
    /// The path to the Escude enum script file (enum_scr.bin)
    pub escude_enum_scr: Option<String>,
    #[cfg(feature = "bgi")]
    /// Duplicate same strings when importing into BGI scripts.
    /// Enable this will cause BGI scripts to become very large.
    pub bgi_import_duplicate: bool,
    #[cfg(feature = "bgi")]
    /// Disable appending new strings to the end of BGI scripts.
    /// Disable may cause BGI scripts broken.
    pub bgi_disable_append: bool,
    #[cfg(feature = "image")]
    /// Output image type
    pub image_type: Option<ImageOutputType>,
    #[cfg(all(feature = "bgi-arc", feature = "bgi-img"))]
    /// Detect all files in BGI archive as SysGrp Images. By default, only files which name is `sysgrp.arc` will enabled this.
    pub bgi_is_sysgrp_arc: Option<bool>,
    #[cfg(feature = "bgi-img")]
    /// Whether to create scrambled SysGrp images. When in import mode, the default value depends on the original image.
    /// When in creation mode, it is not enabled by default.
    pub bgi_img_scramble: Option<bool>,
    #[cfg(feature = "cat-system-arc")]
    /// CatSystem2 engine int archive password
    pub cat_system_int_encrypt_password: Option<String>,
    #[cfg(feature = "cat-system-img")]
    /// Draw CatSystem2 image on canvas (if canvas width and height are specified in file)
    pub cat_system_image_canvas: bool,
    #[cfg(feature = "kirikiri")]
    /// Kirikiri language index in script. If not specified, the first language will be used.
    pub kirikiri_language_index: Option<usize>,
    #[cfg(feature = "kirikiri")]
    /// Export COMU message to extra json file. (for Kirikiri SCN script.)
    /// Only CIRCUS's game have COMU message.
    pub kirikiri_export_comumode: bool,
    #[cfg(feature = "kirikiri")]
    /// Kirikiri COMU message translation. key is original text, value is translated text.
    pub kirikiri_comumode_json: Option<std::sync::Arc<HashMap<String, String>>>,
    #[cfg(feature = "kirikiri")]
    /// Remove empty lines in Kirikiri KS script.
    pub kirikiri_remove_empty_lines: bool,
    #[cfg(feature = "kirikiri")]
    /// Kirikiri name commands, used to extract names from ks script.
    pub kirikiri_name_commands: std::sync::Arc<std::collections::HashSet<String>>,
    #[cfg(feature = "kirikiri")]
    /// Kirikiri message commands, used to extract more message from ks script.
    pub kirikiri_message_commands: std::sync::Arc<std::collections::HashSet<String>>,
    #[cfg(feature = "bgi-arc")]
    /// Whether to compress files in BGI archive when packing BGI archive.
    pub bgi_compress_file: bool,
    #[cfg(feature = "bgi-arc")]
    /// Minimum length of match size for DSC compression. Possible values are 2-256.
    pub bgi_compress_min_len: usize,
    #[cfg(feature = "kirikiri-img")]
    /// Whether to overlay PIMG images. (By default, true if all layers are not group layers.)
    pub kirikiri_pimg_overlay: Option<bool>,
    #[cfg(feature = "artemis-arc")]
    /// Disable Artemis archive (.pfs) XOR encryption when packing.
    pub artemis_arc_disable_xor: bool,
    #[cfg(feature = "artemis")]
    /// Artemis script indent size, used to format Artemis script.
    /// Default is 4 spaces.
    pub artemis_indent: Option<usize>,
    #[cfg(feature = "artemis")]
    /// Disable Artemis script indent, used to format Artemis script.
    pub artemis_no_indent: bool,
    #[cfg(feature = "artemis")]
    /// Max line width in Artemis script, used to format Artemis script.
    pub artemis_max_line_width: usize,
    #[cfg(feature = "artemis")]
    /// Specify the language of Artemis AST script.
    /// If not specified, the first language will be used.
    pub artemis_ast_lang: Option<String>,
    #[cfg(feature = "cat-system")]
    /// CatSystem2 CSTL script language, used to extract messages from CSTL script.
    /// If not specified, the first language will be used.
    pub cat_system_cstl_lang: Option<String>,
    #[cfg(feature = "flate2")]
    /// Zlib compression level. 0 means no compression, 9 means best compression.
    pub zlib_compression_level: u32,
    #[cfg(feature = "image")]
    /// PNG compression level.
    pub png_compression_level: PngCompressionLevel,
    #[cfg(feature = "circus-img")]
    /// Keep original BPP when importing Circus CRX images.
    pub circus_crx_keep_original_bpp: bool,
    #[cfg(feature = "circus-img")]
    /// Use zstd compression for Circus CRX images. (CIRCUS Engine don't support this. Hook is required.)
    pub circus_crx_zstd: bool,
    #[cfg(feature = "zstd")]
    /// Zstd compression level. 0 means default compression level (3), 22 means best compression.
    pub zstd_compression_level: i32,
    #[cfg(feature = "circus-img")]
    /// Circus CRX image row type mode
    pub circus_crx_mode: crate::scripts::circus::image::crx::CircusCrxMode,
    #[cfg(feature = "ex-hibit")]
    /// ExHibit xor key for rld script.
    /// Use [ReExHIBIT](https://github.com/ZQF-ReVN/RxExHIBIT) to find the key.
    pub ex_hibit_rld_xor_key: Option<u32>,
    #[cfg(feature = "ex-hibit")]
    /// ExHibit def.rld xor key.
    pub ex_hibit_rld_def_xor_key: Option<u32>,
    #[cfg(feature = "ex-hibit")]
    /// ExHibit rld xor keys.
    pub ex_hibit_rld_keys: Option<Box<[u32; 0x100]>>,
    #[cfg(feature = "ex-hibit")]
    /// ExHibit def.rld xor keys.
    pub ex_hibit_rld_def_keys: Option<Box<[u32; 0x100]>>,
    #[cfg(feature = "mozjpeg")]
    /// JPEG quality for output images, 0-100. 100 means best quality.
    pub jpeg_quality: u8,
    #[cfg(feature = "webp")]
    /// Use WebP lossless compression for output images.
    pub webp_lossless: bool,
    #[cfg(feature = "webp")]
    /// WebP quality for output images, 0-100. 100 means best quality.
    pub webp_quality: u8,
    #[cfg(feature = "circus-img")]
    /// Draw Circus CRX images on canvas (if canvas width and height are specified in file)
    pub circus_crx_canvas: bool,
    /// Try use YAML format instead of JSON when custom exporting.
    pub custom_yaml: bool,
}

#[derive(Clone, Copy, Debug, ValueEnum, PartialEq, Eq, PartialOrd, Ord)]
/// Script type
pub enum ScriptType {
    #[cfg(feature = "artemis")]
    /// Artemis Engine AST script
    Artemis,
    #[cfg(feature = "artemis")]
    /// Artemis Engine ASB script
    ArtemisAsb,
    #[cfg(feature = "artemis-arc")]
    #[value(alias("pfs"))]
    /// Artemis archive (pfs)
    ArtemisArc,
    #[cfg(feature = "bgi")]
    #[value(alias("ethornell"))]
    /// Buriko General Interpreter/Ethornell Script
    BGI,
    #[cfg(feature = "bgi")]
    #[value(alias("ethornell-bsi"))]
    /// Buriko General Interpreter/Ethornell bsi script (._bsi)
    BGIBsi,
    #[cfg(feature = "bgi")]
    #[value(alias("ethornell-bp"))]
    /// Buriko General Interpreter/Ethornell bp script (._bp)
    BGIBp,
    #[cfg(feature = "bgi-arc")]
    #[value(alias = "ethornell-arc-v1")]
    /// Buriko General Interpreter/Ethornell archive v1
    BGIArcV1,
    #[cfg(feature = "bgi-arc")]
    #[value(alias = "ethornell-arc-v2", alias = "bgi-arc", alias = "ethornell-arc")]
    /// Buriko General Interpreter/Ethornell archive v2
    BGIArcV2,
    #[cfg(feature = "bgi-arc")]
    #[value(alias("ethornell-dsc"))]
    /// Buriko General Interpreter/Ethornell compressed file (DSC)
    BGIDsc,
    #[cfg(feature = "bgi-audio")]
    #[value(alias("ethornell-audio"))]
    /// Buriko General Interpreter/Ethornell audio file (Ogg/Vorbis)
    BGIAudio,
    #[cfg(feature = "bgi-img")]
    #[value(alias("ethornell-img"))]
    /// Buriko General Interpreter/Ethornell image (Image files in sysgrp.arc)
    BGIImg,
    #[cfg(feature = "bgi-img")]
    #[value(alias("ethornell-cbg"))]
    /// Buriko General Interpreter/Ethornell Compressed Background image (CBG)
    BGICbg,
    #[cfg(feature = "cat-system")]
    /// CatSystem2 engine scene script
    CatSystem,
    #[cfg(feature = "cat-system")]
    /// CatSystem2 engine CSTL script
    CatSystemCstl,
    #[cfg(feature = "cat-system-arc")]
    /// CatSystem2 engine archive
    CatSystemInt,
    #[cfg(feature = "cat-system-img")]
    /// CatSystem2 engine image
    CatSystemHg3,
    #[cfg(feature = "circus")]
    /// Circus MES script
    Circus,
    #[cfg(feature = "circus-arc")]
    /// Circus Image archive
    CircusCrm,
    #[cfg(feature = "circus-arc")]
    /// Circus DAT archive
    CircusDat,
    #[cfg(feature = "circus-arc")]
    /// Circus PCK archive
    CircusPck,
    #[cfg(feature = "circus-audio")]
    /// Circus PCM audio
    CircusPcm,
    #[cfg(feature = "circus-img")]
    /// Circus CRX Image
    CircusCrx,
    #[cfg(feature = "circus-img")]
    /// Circus Differential Image
    CircusCrxd,
    #[cfg(feature = "escude-arc")]
    /// Escude bin archive
    EscudeArc,
    #[cfg(feature = "escude")]
    /// Escude bin script
    Escude,
    #[cfg(feature = "escude")]
    /// Escude list script
    EscudeList,
    #[cfg(feature = "ex-hibit")]
    /// ExHibit rld script
    ExHibit,
    #[cfg(feature = "hexen-haus")]
    /// HexenHaus bin script
    HexenHaus,
    #[cfg(feature = "kirikiri")]
    #[value(alias("kr-scn"))]
    /// Kirikiri SCN script
    KirikiriScn,
    #[cfg(feature = "kirikiri")]
    #[value(alias("kr-simple-crypt"))]
    /// Kirikiri SimpleCrypt's text file
    KirikiriSimpleCrypt,
    #[cfg(feature = "kirikiri")]
    #[value(alias = "kr", alias = "kr-ks", alias = "kirikiri-ks")]
    /// Kirikiri script
    Kirikiri,
    #[cfg(feature = "kirikiri-img")]
    #[value(alias("kr-tlg"))]
    /// Kirikiri TLG image
    KirikiriTlg,
    #[cfg(feature = "kirikiri-img")]
    #[value(alias("kr-pimg"))]
    /// Kirikiri PIMG image
    KirikiriPimg,
    #[cfg(feature = "kirikiri-img")]
    #[value(alias("kr-dref"))]
    /// Kirikiri DREF(DPAK-referenced) image
    KirikiriDref,
    #[cfg(feature = "kirikiri")]
    #[value(alias("kr-mdf"))]
    /// Kirikiri MDF (zlib compressed) file
    KirikiriMdf,
    #[cfg(feature = "will-plus")]
    /// WillPlus ws2 script
    WillPlusWs2,
    #[cfg(feature = "yaneurao-itufuru")]
    #[value(alias("itufuru"))]
    /// Yaneurao Itufuru script
    YaneuraoItufuru,
    #[cfg(feature = "yaneurao-itufuru")]
    #[value(alias("itufuru-arc"))]
    /// Yaneurao Itufuru script archive
    YaneuraoItufuruArc,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
/// Message structure for scripts
pub struct Message {
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Optional name for the message, used in some scripts.
    pub name: Option<String>,
    /// The actual message content.
    pub message: String,
}

impl Message {
    /// Creates a new `Message` instance.
    pub fn new(message: String, name: Option<String>) -> Self {
        Message { message, name }
    }
}

/// Result of script operation.
pub enum ScriptResult {
    /// Operation completed successfully.
    Ok,
    /// Operation completed without any changes.
    /// For example, no messages found in the script.
    Ignored,
}

#[derive(Clone, Copy, Debug, ValueEnum, PartialEq, Eq, PartialOrd, Ord)]
/// Format type (for CLI)
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
/// Name table cell
pub struct NameTableCell {
    #[serde(rename = "JP_Name")]
    /// Original name
    pub jp_name: String,
    #[serde(rename = "CN_Name")]
    /// Translated name
    pub cn_name: String,
    #[serde(rename = "Count")]
    /// Number of times this name appears in the script
    pub count: usize,
}

#[derive(Debug, Serialize, Deserialize)]
/// Replacement table for string replacements
pub struct ReplacementTable {
    #[serde(flatten)]
    /// Map of original strings to their replacements
    pub map: HashMap<String, String>,
}

#[cfg(feature = "image")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
/// Image color type
pub enum ImageColorType {
    /// Grayscale image
    Grayscale,
    /// RGB image
    Rgb,
    /// RGBA image
    Rgba,
    /// BGR image
    Bgr,
    /// BGRA image
    Bgra,
}

#[cfg(feature = "image")]
impl ImageColorType {
    /// Returns the number of bytes per pixel for the color type and depth.
    pub fn bpp(&self, depth: u8) -> u16 {
        match self {
            ImageColorType::Grayscale => depth as u16,
            ImageColorType::Rgb => depth as u16 * 3,
            ImageColorType::Rgba => depth as u16 * 4,
            ImageColorType::Bgr => depth as u16 * 3,
            ImageColorType::Bgra => depth as u16 * 4,
        }
    }
}

#[cfg(feature = "image")]
#[derive(Clone, Copy, Debug, ValueEnum, PartialEq, Eq, PartialOrd, Ord)]
/// Image output type
pub enum ImageOutputType {
    /// PNG image
    Png,
    #[cfg(feature = "image-jpg")]
    /// JPEG image
    Jpg,
    #[cfg(feature = "image-webp")]
    /// WebP image
    Webp,
}

#[cfg(feature = "image")]
impl TryFrom<&str> for ImageOutputType {
    type Error = anyhow::Error;

    /// Try to convert a extension string to an `ImageOutputType`.
    /// Extensions are case-insensitive.
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value.to_ascii_lowercase().as_str() {
            "png" => Ok(ImageOutputType::Png),
            #[cfg(feature = "image-jpg")]
            "jpg" => Ok(ImageOutputType::Jpg),
            #[cfg(feature = "image-jpg")]
            "jpeg" => Ok(ImageOutputType::Jpg),
            #[cfg(feature = "image-webp")]
            "webp" => Ok(ImageOutputType::Webp),
            _ => Err(anyhow::anyhow!("Unsupported image output type: {}", value)),
        }
    }
}

#[cfg(feature = "image")]
impl TryFrom<&std::path::Path> for ImageOutputType {
    type Error = anyhow::Error;

    fn try_from(value: &std::path::Path) -> Result<Self, Self::Error> {
        if let Some(ext) = value.extension() {
            Self::try_from(ext.to_string_lossy().as_ref())
        } else {
            Err(anyhow::anyhow!("No extension found in path"))
        }
    }
}

#[cfg(feature = "image")]
impl AsRef<str> for ImageOutputType {
    /// Returns the extension for the image output type.
    fn as_ref(&self) -> &str {
        match self {
            ImageOutputType::Png => "png",
            #[cfg(feature = "image-jpg")]
            ImageOutputType::Jpg => "jpg",
            #[cfg(feature = "image-webp")]
            ImageOutputType::Webp => "webp",
        }
    }
}

#[cfg(feature = "image")]
#[derive(Clone, Debug)]
/// Image data
pub struct ImageData {
    /// Image width in pixels
    pub width: u32,
    /// Image height in pixels
    pub height: u32,
    /// Image color type
    pub color_type: ImageColorType,
    /// Image depth in bits per channel
    pub depth: u8,
    /// Image data
    pub data: Vec<u8>,
}

#[cfg(feature = "image")]
#[derive(Clone, Debug)]
/// Image data with name
pub struct ImageDataWithName {
    /// Image name
    pub name: String,
    /// Image data
    pub data: ImageData,
}

#[derive(Clone, Copy, Debug, ValueEnum, PartialEq, Eq, PartialOrd, Ord)]
/// BOM type
pub enum BomType {
    /// No BOM
    None,
    /// UTF-8 BOM
    Utf8,
    /// UTF-16 Little Endian BOM
    Utf16LE,
    /// UTF-16 Big Endian BOM
    Utf16BE,
}

impl BomType {
    /// Returns the byte sequence for the BOM type.
    pub fn as_bytes(&self) -> &'static [u8] {
        match self {
            BomType::None => &[],
            BomType::Utf8 => b"\xEF\xBB\xBF",
            BomType::Utf16LE => b"\xFF\xFE",
            BomType::Utf16BE => b"\xFE\xFF",
        }
    }
}

#[cfg(feature = "image")]
#[derive(Clone, Copy, Debug, ValueEnum, PartialEq, Eq, PartialOrd, Ord)]
/// PNG compression level
pub enum PngCompressionLevel {
    #[value(alias = "d")]
    /// Default level
    Default,
    #[value(alias = "f")]
    /// Fast minimal compression
    Fast,
    #[value(alias = "b")]
    /// Higher compression level
    ///
    /// Best in this context isn't actually the highest possible level
    /// the encoder can do, but is meant to emulate the `Best` setting in the `Flate2`
    /// library.
    Best,
}

#[cfg(feature = "image")]
impl Default for PngCompressionLevel {
    fn default() -> Self {
        PngCompressionLevel::Default
    }
}

#[cfg(feature = "image")]
impl PngCompressionLevel {
    /// Converts the [PngCompressionLevel] to a [png::Compression] enum.
    pub fn to_compression(&self) -> png::Compression {
        match self {
            PngCompressionLevel::Default => png::Compression::Default,
            PngCompressionLevel::Fast => png::Compression::Fast,
            PngCompressionLevel::Best => png::Compression::Best,
        }
    }
}
