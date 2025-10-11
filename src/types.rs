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
    /// UTF-16 Little Endian encoding
    Utf16LE,
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

    /// Returns true if the encoding is UTF-16LE.
    pub fn is_utf16le(&self) -> bool {
        match self {
            Self::Utf16LE => true,
            #[cfg(windows)]
            Self::CodePage(code_page) => *code_page == 1200,
            _ => false,
        }
    }

    /// Returns true if the encoding is UTF8.
    pub fn is_utf8(&self) -> bool {
        match self {
            Self::Utf8 => true,
            #[cfg(windows)]
            Self::CodePage(code_page) => *code_page == 65001,
            _ => false,
        }
    }

    /// Returns the charset name
    pub fn charset(&self) -> Option<&'static str> {
        match self {
            Self::Auto => None,
            Self::Utf8 => Some("UTF-8"),
            Self::Cp932 => Some("shift_jis"),
            Self::Gb2312 => Some("gbk"),
            Self::Utf16LE => Some("utf-16le"),
            #[cfg(windows)]
            Self::CodePage(code_page) => match *code_page {
                932 => Some("shift_jis"),
                65001 => Some("utf-8"),
                1200 => Some("utf-16le"),
                936 => Some("gbk"),
                _ => None,
            },
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
    /// Same as M3t, buf different extension
    M3ta,
    /// Same as M3t, buf different extension
    M3tTxt,
    /// JSON which can be used for GalTransl
    Json,
    /// YAML (same as JSON, but with YAML syntax)
    Yaml,
    /// Gettext .pot file
    Pot,
    /// Gettext .po file
    Po,
    /// Custom output
    Custom,
}

impl OutputScriptType {
    /// Returns true if the script type is custom.
    pub fn is_custom(&self) -> bool {
        matches!(self, OutputScriptType::Custom)
    }

    /// Returns true if the script type is M3t/M3ta/M3tTxt.
    pub fn is_m3t(&self) -> bool {
        matches!(
            self,
            OutputScriptType::M3t | OutputScriptType::M3ta | OutputScriptType::M3tTxt
        )
    }
}

impl AsRef<str> for OutputScriptType {
    /// Returns the extension for the script type.
    fn as_ref(&self) -> &str {
        match self {
            OutputScriptType::M3t => "m3t",
            OutputScriptType::M3ta => "m3ta",
            OutputScriptType::M3tTxt => "txt",
            OutputScriptType::Json => "json",
            OutputScriptType::Yaml => "yaml",
            OutputScriptType::Pot => "pot",
            OutputScriptType::Po => "po",
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
#[derive(Debug, Clone, msg_tool_macro::Default)]
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
    /// Export chat message to extra json file. (for Kirikiri SCN script.)
    /// For example, CIRCUS's comu message. Yuzusoft's phone chat message.
    pub kirikiri_export_chat: bool,
    #[cfg(feature = "kirikiri")]
    /// Kirikiri chat message key. For example, CIRCUS's key is "comumode". Yuzusoft's key is "phonechat".
    /// If not specified, "comumode" will be used.
    pub kirikiri_chat_key: Option<String>,
    #[cfg(feature = "kirikiri")]
    /// Kirikiri chat message translation. The outter object's key is filename(`global` is a special key).
    /// The inner object: key is original text, value is (translated text, original text count).
    pub kirikiri_chat_json:
        Option<std::sync::Arc<HashMap<String, HashMap<String, (String, usize)>>>>,
    #[cfg(feature = "kirikiri")]
    /// Kirikiri language list. First language code is code for language index 1.
    pub kirikiri_languages: Option<std::sync::Arc<Vec<String>>>,
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
    #[default(3)]
    /// Minimum length of match size for DSC compression. Possible values are 2-256.
    pub bgi_compress_min_len: usize,
    #[cfg(feature = "emote-img")]
    /// Whether to overlay PIMG images. (By default, true if all layers are not group layers.)
    pub emote_pimg_overlay: Option<bool>,
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
    #[default(100)]
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
    #[default(6)]
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
    #[default(3)]
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
    #[default(80)]
    /// JPEG quality for output images, 0-100. 100 means best quality.
    pub jpeg_quality: u8,
    #[cfg(feature = "webp")]
    /// Use WebP lossless compression for output images.
    pub webp_lossless: bool,
    #[cfg(feature = "webp")]
    #[default(80)]
    /// WebP quality for output images, 0-100. 100 means best quality.
    pub webp_quality: u8,
    #[cfg(feature = "circus-img")]
    /// Draw Circus CRX images on canvas (if canvas width and height are specified in file)
    pub circus_crx_canvas: bool,
    /// Try use YAML format instead of JSON when custom exporting.
    pub custom_yaml: bool,
    #[cfg(feature = "entis-gls")]
    /// Entis GLS srcxml script language, used to extract messages from srcxml script.
    /// If not specified, the first language will be used.
    pub entis_gls_srcxml_lang: Option<String>,
    #[cfg(feature = "will-plus")]
    /// Disable disassembly for WillPlus ws2 script.
    /// Use another parser to parse the script.
    /// Should only be used when the default parser not works well.
    pub will_plus_ws2_no_disasm: bool,
    #[cfg(feature = "artemis-panmimisoft")]
    /// Artemis Engine blacklist tag names for TXT script.
    /// This is used to ignore these tags when finding names in Artemis TXT (ぱんみみそふと) script.
    pub artemis_panmimisoft_txt_blacklist_names: std::sync::Arc<std::collections::HashSet<String>>,
    #[cfg(feature = "artemis-panmimisoft")]
    /// Specify the language of Artemis TXT (ぱんみみそふと) script.
    /// If not specified, the first language will be used.
    pub artemis_panmimisoft_txt_lang: Option<String>,
    #[cfg(feature = "lossless-audio")]
    /// Audio format for output lossless audio files.
    pub lossless_audio_fmt: LosslessAudioFormat,
    #[cfg(feature = "audio-flac")]
    #[default(5)]
    /// FLAC compression level for output FLAC audio files. 0 means fastest compression, 8 means best compression. Default level is 5.
    pub flac_compression_level: u32,
    #[cfg(feature = "artemis")]
    #[default(true)]
    /// Format lua code in Artemis ASB script(.asb/.iet) when exporting.
    pub artemis_asb_format_lua: bool,
    #[cfg(feature = "kirikiri")]
    /// Whether to handle title in Kirikiri SCN script.
    pub kirikiri_title: bool,
    #[cfg(feature = "favorite")]
    #[default(true)]
    /// Whether to filter ascii strings in Favorite HCB script.
    pub favorite_hcb_filter_ascii: bool,
    #[cfg(feature = "bgi-img")]
    #[default(get_default_threads())]
    /// Workers count for decode BGI compressed images v2 in parallel. Default is half of CPU cores.
    /// Set this to 1 to disable parallel decoding. 0 means same as 1.
    pub bgi_img_workers: usize,
    #[cfg(feature = "image-jxl")]
    #[default(true)]
    /// Use JXL lossless compression for output images. Enabled by default.
    pub jxl_lossless: bool,
    #[cfg(feature = "image-jxl")]
    #[default(1.0)]
    /// JXL distance for output images. 0 means mathematically lossless compression. 1.0 means visually lossless compression.
    /// Allowed range is 0.0-25.0. Recommended range is 0.5-3.0. Default value is 1.0.
    pub jxl_distance: f32,
    #[cfg(feature = "image-jxl")]
    #[default(1)]
    /// Workers count for encode JXL images in parallel. Default is 1.
    /// Set this to 1 to disable parallel encoding. 0 means same as 1
    pub jxl_workers: usize,
    #[cfg(feature = "emote-img")]
    #[default(true)]
    /// Process tlg images.
    pub psb_process_tlg: bool,
    #[cfg(feature = "softpal-img")]
    #[default(true)]
    /// Whether to use fake compression for Softpal Pgd images. Enabled by default.
    /// WARN: Compress may cause image broken.
    pub pgd_fake_compress: bool,
    #[cfg(feature = "softpal")]
    /// Whether to add message index to Softpal src script when exporting.
    pub softpal_add_message_index: bool,
    #[cfg(feature = "kirikiri")]
    #[default(true)]
    /// Enable multi-language support for Kirikiri chat messages. Default is true.
    /// Note: This requires [Self::kirikiri_language_index] and [Self::kirikiri_languages] to be set correctly.
    pub kirikiri_chat_multilang: bool,
    #[cfg(feature = "kirikiri-arc")]
    #[default(true)]
    /// Decrypt SimpleCrypt files in Kirikiri XP3 archive when extracting. Default is true.
    pub xp3_simple_crypt: bool,
    #[cfg(feature = "kirikiri-arc")]
    #[default(true)]
    /// Decompress mdf files in Kirikiri XP3 archive when extracting. Default is true.
    pub xp3_mdf_decompress: bool,
    #[cfg(feature = "kirikiri-arc")]
    /// Configuration for Kirikiri XP3 segmenter when creating XP3 archive.
    pub xp3_segmenter: crate::scripts::kirikiri::archive::xp3::SegmenterConfig,
    #[cfg(feature = "kirikiri-arc")]
    #[default(true)]
    /// Compress files in Kirikiri XP3 archive when creating. Default is true.
    pub xp3_compress_files: bool,
    #[cfg(feature = "kirikiri-arc")]
    #[default(true)]
    /// Compress index in Kirikiri XP3 archive when creating. Default is true.
    pub xp3_compress_index: bool,
    #[cfg(feature = "kirikiri-arc")]
    #[default(num_cpus::get())]
    /// Workers count for compress files in Kirikiri XP3 archive when creating in parallel. Default is CPU cores count.
    pub xp3_compress_workers: usize,
    #[cfg(feature = "kirikiri-arc")]
    /// Use zstd compression for files in Kirikiri XP3 archive when creating. (Warning: Kirikiri engine don't support this. Hook is required.)
    pub xp3_zstd: bool,
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
    #[cfg(feature = "artemis")]
    /// Artemis Engine TXT (General) script
    ArtemisTxt,
    #[cfg(feature = "artemis-panmimisoft")]
    /// Artemis Engine TXT (ぱんみみそふと) script
    ArtemisPanmimisoftTxt,
    #[cfg(feature = "artemis-arc")]
    #[value(alias("pfs"))]
    /// Artemis archive (pfs)
    ArtemisArc,
    #[cfg(feature = "artemis-arc")]
    #[value(alias("pf2"))]
    /// Artemis archive (pf2) (.pfs)
    ArtemisPf2,
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
    #[cfg(feature = "emote-img")]
    #[value(alias("psb"))]
    /// Emote PSB (basic handle)
    EmotePsb,
    #[cfg(feature = "emote-img")]
    #[value(alias("pimg"))]
    /// Emote PIMG image
    EmotePimg,
    #[cfg(feature = "emote-img")]
    #[value(alias("dref"))]
    /// Emote DREF(DPAK-referenced) image
    EmoteDref,
    #[cfg(feature = "entis-gls")]
    /// Entis GLS srcxml Script
    EntisGls,
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
    #[cfg(feature = "ex-hibit-arc")]
    /// ExHibit GRP archive
    ExHibitGrp,
    #[cfg(feature = "favorite")]
    /// Favorite hcb script
    Favorite,
    #[cfg(feature = "hexen-haus")]
    /// HexenHaus bin script
    HexenHaus,
    #[cfg(feature = "hexen-haus-arc")]
    /// HexenHaus Arcc archive
    HexenHausArcc,
    #[cfg(feature = "hexen-haus-arc")]
    /// HexenHaus Audio archive
    HexenHausOdio,
    #[cfg(feature = "hexen-haus-arc")]
    /// HexenHaus WAG archive
    HexenHausWag,
    #[cfg(feature = "hexen-haus-img")]
    /// HexenHaus PNG image
    HexenHausPng,
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
    #[cfg(feature = "kirikiri-arc")]
    #[value(alias = "kr-xp3", alias = "xp3")]
    /// Kirikiri XP3 archive
    KirikiriXp3,
    #[cfg(feature = "kirikiri-img")]
    #[value(alias("kr-tlg"))]
    /// Kirikiri TLG image
    KirikiriTlg,
    #[cfg(feature = "kirikiri")]
    #[value(alias("kr-mdf"))]
    /// Kirikiri MDF (zlib compressed) file
    KirikiriMdf,
    #[cfg(feature = "kirikiri")]
    #[value(alias("kr-tjs2"))]
    /// Kirikiri compiled TJS2 script
    KirikiriTjs2,
    #[cfg(feature = "kirikiri")]
    #[value(alias("kr-tjs-ns0"))]
    /// Kirikiri TJS NS0 binary encoded script
    KirikiriTjsNs0,
    #[cfg(feature = "silky")]
    /// Silky Engine Mes script
    Silky,
    #[cfg(feature = "silky")]
    /// Silky Engine Map script
    SilkyMap,
    #[cfg(feature = "softpal")]
    /// Softpal src script
    Softpal,
    #[cfg(feature = "softpal-arc")]
    /// Softpal Pac archive
    SoftpalPac,
    #[cfg(feature = "softpal-arc")]
    /// Softpal Pac/AMUSE archive
    SoftpalPacAmuse,
    #[cfg(feature = "softpal-img")]
    #[value(alias = "pgd-ge", alias = "pgd")]
    /// Softpal Pgd Ge image
    SoftpalPgdGe,
    #[cfg(feature = "softpal-img")]
    #[value(alias = "softpal-pgd2", alias = "pgd3", alias = "pgd2")]
    /// Softpal Pgd Differential image
    SoftpalPgd3,
    #[cfg(feature = "will-plus")]
    #[value(alias("adv-hd-ws2"))]
    /// WillPlus ws2 script
    WillPlusWs2,
    #[cfg(feature = "will-plus-img")]
    #[value(alias("adv-hd-wip"))]
    /// WillPlus WIP Image
    WillPlusWip,
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
    /// Operation not completed.
    /// This will not count in statistics.
    Uncount,
}

#[derive(Clone, Copy, Debug, ValueEnum, PartialEq, Eq, PartialOrd, Ord)]
/// Format type (for CLI)
pub enum FormatType {
    /// Wrap line with fixed length
    Fixed,
    /// Do not wrap line
    None,
}

#[derive(Clone)]
/// Format options
pub enum FormatOptions {
    /// Wrap line with fixed length
    Fixed {
        /// Fixed length
        length: usize,
        /// Whether to keep original line breaks
        keep_original: bool,
        /// Whether to break words(ASCII only) at the end of the line
        break_words: bool,
        /// Whether to insert a full-width space after a line break when a sentence starts with a full-width quotation mark.
        insert_fullwidth_space_at_line_start: bool,
        /// If a line break occurs in the middle of some symbols, bring the sentence to next line
        break_with_sentence: bool,
        #[cfg(feature = "jieba")]
        /// Whether to break Chinese words at the end of the line.
        break_chinese_words: bool,
        #[cfg(feature = "jieba")]
        /// Path to custom jieba dictionary
        jieba_dict: Option<String>,
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
    #[cfg(feature = "image-jxl")]
    /// JPEG XL image
    Jxl,
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
            #[cfg(feature = "image-jxl")]
            "jxl" => Ok(ImageOutputType::Jxl),
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
            #[cfg(feature = "image-jxl")]
            ImageOutputType::Jxl => "jxl",
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
    #[value(alias = "n")]
    /// No compression whatsoever. Fastest, but results in large files.
    NoCompression,
    #[value(alias = "d")]
    /// Default level (usually balanced)
    Default,
    /// Extremely fast but light compression.
    ///
    /// Note: When used in streaming mode, this compression level can actually result in files
    /// *larger* than would be produced by `NoCompression` on incompressible data because
    /// it doesn't do any buffering of the output stream to detect whether the data is being compressed or not.
    Fastest,
    #[value(alias = "f")]
    /// Fast minimal compression
    Fast,
    #[value(alias = "b")]
    /// Higher compression level. Same as high
    Best,
    #[value(alias = "h")]
    /// Spend much more time to produce a slightly smaller file than with `Balanced`.
    High,
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
            PngCompressionLevel::NoCompression => png::Compression::NoCompression,
            PngCompressionLevel::Fastest => png::Compression::Fastest,
            PngCompressionLevel::Default => png::Compression::Balanced,
            PngCompressionLevel::Fast => png::Compression::Fast,
            PngCompressionLevel::Best => png::Compression::High,
            PngCompressionLevel::High => png::Compression::High,
        }
    }
}

#[cfg(feature = "lossless-audio")]
#[derive(Clone, Copy, Debug, ValueEnum, PartialEq, Eq, PartialOrd, Ord)]
/// Lossless audio format
pub enum LosslessAudioFormat {
    /// Wav
    Wav,
    #[cfg(feature = "audio-flac")]
    /// FLAC Format
    Flac,
}

#[cfg(feature = "lossless-audio")]
impl Default for LosslessAudioFormat {
    fn default() -> Self {
        LosslessAudioFormat::Wav
    }
}

#[cfg(feature = "lossless-audio")]
impl TryFrom<&str> for LosslessAudioFormat {
    type Error = anyhow::Error;
    /// Try to convert a extension string to an `LosslessAudioFormat`.
    /// Extensions are case-insensitive.
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value.to_ascii_lowercase().as_str() {
            "wav" => Ok(LosslessAudioFormat::Wav),
            #[cfg(feature = "audio-flac")]
            "flac" => Ok(LosslessAudioFormat::Flac),
            _ => Err(anyhow::anyhow!(
                "Unsupported lossless audio format: {}",
                value
            )),
        }
    }
}

#[cfg(feature = "lossless-audio")]
impl TryFrom<&std::path::Path> for LosslessAudioFormat {
    type Error = anyhow::Error;

    fn try_from(value: &std::path::Path) -> Result<Self, Self::Error> {
        if let Some(ext) = value.extension() {
            Self::try_from(ext.to_string_lossy().as_ref())
        } else {
            Err(anyhow::anyhow!("No extension found in path"))
        }
    }
}

#[cfg(feature = "lossless-audio")]
impl AsRef<str> for LosslessAudioFormat {
    /// Returns the extension for the lossless audio format.
    fn as_ref(&self) -> &str {
        match self {
            LosslessAudioFormat::Wav => "wav",
            #[cfg(feature = "audio-flac")]
            LosslessAudioFormat::Flac => "flac",
        }
    }
}

#[cfg(feature = "utils-threadpool")]
#[allow(unused)]
pub(crate) fn get_default_threads() -> usize {
    num_cpus::get().max(2) / 2
}
