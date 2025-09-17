use crate::types::*;
#[allow(unused)]
use crate::utils::num_range::*;
use clap::{ArgAction, ArgGroup, Parser, Subcommand};

#[cfg(feature = "flate2")]
fn parse_compression_level(level: &str) -> Result<u32, String> {
    let lower = level.to_ascii_lowercase();
    if lower == "none" {
        return Ok(0);
    } else if lower == "best" {
        return Ok(9);
    } else if lower == "default" {
        return Ok(6);
    } else if lower == "fast" {
        return Ok(1);
    }
    number_range(level, 0, 9)
}

#[cfg(feature = "mozjpeg")]
fn parse_jpeg_quality(quality: &str) -> Result<u8, String> {
    let lower = quality.to_ascii_lowercase();
    if lower == "best" {
        return Ok(100);
    }
    number_range(quality, 0, 100)
}

#[cfg(feature = "zstd")]
fn parse_zstd_compression_level(level: &str) -> Result<i32, String> {
    let lower = level.to_ascii_lowercase();
    if lower == "default" {
        return Ok(3);
    } else if lower == "best" {
        return Ok(22);
    }
    number_range(level, 0, 22)
}

#[cfg(feature = "webp")]
fn parse_webp_quality(quality: &str) -> Result<u8, String> {
    let lower = quality.to_ascii_lowercase();
    if lower == "best" {
        return Ok(100);
    }
    number_range(quality, 0, 100)
}

#[cfg(feature = "audio-flac")]
fn parse_flac_compression_level(level: &str) -> Result<u32, String> {
    let lower = level.to_ascii_lowercase();
    if lower == "fast" {
        return Ok(0);
    } else if lower == "best" {
        return Ok(8);
    } else if lower == "default" {
        return Ok(5);
    }
    number_range(level, 0, 8)
}

#[cfg(feature = "image-jxl")]
fn parse_jxl_distance(s: &str) -> Result<f32, String> {
    let lower = s.to_ascii_lowercase();
    if lower == "lossless" {
        return Ok(0.0);
    } else if lower == "visually-lossless" {
        return Ok(1.0);
    }
    number_range(s, 0.0, 25.0)
}

/// Tools for export and import scripts
#[derive(Parser, Debug)]
#[clap(
    group = ArgGroup::new("encodingg").multiple(false),
    group = ArgGroup::new("output_encodingg").multiple(false),
    group = ArgGroup::new("archive_encodingg").multiple(false),
    group = ArgGroup::new("artemis_indentg").multiple(false),
    group = ArgGroup::new("ex_hibit_rld_xor_keyg").multiple(false),
    group = ArgGroup::new("ex_hibit_rld_def_xor_keyg").multiple(false),
    group = ArgGroup::new("webp_qualityg").multiple(false),
    group = ArgGroup::new("cat_system_int_encrypt_passwordg").multiple(false),
    group = ArgGroup::new("kirikiri_chat_jsong").multiple(false),
)]
#[command(
    version,
    about,
    long_about = "Tools for export and import scripts\nhttps://github.com/lifegpc/msg-tool"
)]
pub struct Arg {
    #[arg(short = 't', long, value_enum, global = true)]
    /// Script type
    pub script_type: Option<ScriptType>,
    #[arg(short = 'T', long, value_enum, global = true)]
    /// Output script type
    pub output_type: Option<OutputScriptType>,
    #[arg(short = 'n', long, global = true)]
    /// Disable extra extension when locating/export output script
    pub output_no_extra_ext: bool,
    #[cfg(feature = "image")]
    #[arg(short = 'i', long, value_enum, global = true)]
    /// Output image type
    pub image_type: Option<ImageOutputType>,
    #[arg(short = 'e', long, value_enum, global = true, group = "encodingg")]
    /// Script encoding
    pub encoding: Option<TextEncoding>,
    #[cfg(windows)]
    #[arg(short = 'c', long, value_enum, global = true, group = "encodingg")]
    /// Script code page
    pub code_page: Option<u32>,
    #[arg(
        short = 'E',
        long,
        value_enum,
        global = true,
        group = "output_encodingg"
    )]
    /// Output text encoding
    pub output_encoding: Option<TextEncoding>,
    #[cfg(windows)]
    #[arg(
        short = 'C',
        long,
        value_enum,
        global = true,
        group = "output_encodingg"
    )]
    /// Output code page
    pub output_code_page: Option<u32>,
    #[arg(
        short = 'a',
        long,
        value_enum,
        global = true,
        group = "archive_encodingg"
    )]
    /// Archive filename encoding
    pub archive_encoding: Option<TextEncoding>,
    #[cfg(windows)]
    #[arg(
        short = 'A',
        long,
        value_enum,
        global = true,
        group = "archive_encodingg"
    )]
    /// Archive code page
    pub archive_code_page: Option<u32>,
    #[cfg(feature = "circus")]
    #[arg(long, value_enum, global = true)]
    /// Circus Game
    pub circus_mes_type: Option<CircusMesType>,
    #[arg(short, long, action = ArgAction::SetTrue, global = true)]
    /// Search for script files in the directory recursively
    pub recursive: bool,
    #[arg(global = true, action = ArgAction::SetTrue, short, long)]
    /// Print backtrace on error
    pub backtrace: bool,
    #[cfg(feature = "escude-arc")]
    #[arg(long, action = ArgAction::SetTrue, global = true)]
    /// Whether to use fake compression for Escude archive
    pub escude_fake_compress: bool,
    #[cfg(feature = "escude")]
    #[arg(long, global = true)]
    /// The path to the Escude enum script file (enum_scr.bin)
    pub escude_enum_scr: Option<String>,
    #[cfg(feature = "bgi")]
    #[arg(long, action = ArgAction::SetTrue, global = true)]
    /// Duplicate same strings when importing into BGI scripts.
    /// Enable this will cause BGI scripts to become very large.
    pub bgi_import_duplicate: bool,
    #[cfg(feature = "bgi")]
    #[arg(long, action = ArgAction::SetTrue, global = true, visible_alias = "bgi-no-append")]
    /// Disable appending new strings to the end of BGI scripts.
    /// Disable may cause BGI scripts broken.
    pub bgi_disable_append: bool,
    #[cfg(all(feature = "bgi-arc", feature = "bgi-img"))]
    #[arg(long, global = true)]
    /// Detect all files in BGI archive as SysGrp Images. By default, only files which name is `sysgrp.arc` will enabled this.
    pub bgi_is_sysgrp_arc: Option<bool>,
    #[cfg(feature = "bgi-img")]
    #[arg(long, global = true)]
    /// Whether to create scrambled SysGrp images. When in import mode, the default value depends on the original image.
    /// When in creation mode, it is not enabled by default.
    pub bgi_img_scramble: Option<bool>,
    #[cfg(feature = "bgi-img")]
    #[arg(long, global = true, default_value_t = crate::types::get_default_threads())]
    /// Workers count for decode BGI compressed images v2 in parallel. Default is half of CPU cores.
    /// Set this to 1 to disable parallel decoding. 0 means same as 1.
    pub bgi_img_workers: usize,
    #[cfg(feature = "cat-system-arc")]
    #[arg(long, global = true, group = "cat_system_int_encrypt_passwordg")]
    /// CatSystem2 engine int archive password
    pub cat_system_int_encrypt_password: Option<String>,
    #[cfg(feature = "cat-system-arc")]
    #[arg(long, global = true, group = "cat_system_int_encrypt_passwordg")]
    /// The path to the CatSystem2 engine executable file. Used to get the int archive password.
    pub cat_system_int_exe: Option<String>,
    #[cfg(feature = "cat-system-img")]
    #[arg(long, global = true, action = ArgAction::SetTrue)]
    /// Draw CatSystem2 image on canvas (if canvas width and height are specified in file)
    pub cat_system_image_canvas: bool,
    #[cfg(feature = "kirikiri")]
    #[arg(long, global = true)]
    /// Kirikiri language index in script. If not specified, the first language will be used.
    pub kirikiri_language_index: Option<usize>,
    #[cfg(feature = "kirikiri")]
    #[arg(long, global = true)]
    /// Export chat message to extra json file. (for Kirikiri SCN script.)
    /// For example, CIRCUS's comu message. Yuzusoft's phone chat message.
    pub kirikiri_export_chat: bool,
    #[cfg(feature = "kirikiri")]
    #[arg(long, global = true)]
    /// Kirikiri chat message key. For example, CIRCUS's key is "comumode". Yuzusoft's key is "phonechat".
    /// If not specified, "comumode" will be used.
    pub kirikiri_chat_key: Option<String>,
    #[cfg(feature = "kirikiri")]
    #[arg(long, global = true, group = "kirikiri_chat_jsong")]
    /// Kirikiri chat message translation file. (Map<String, String>, key is original text, value is translated text.)
    pub kirikiri_chat_json: Option<String>,
    #[cfg(feature = "kirikiri")]
    #[arg(long, global = true, group = "kirikiri_chat_jsong")]
    /// Kirikiri chat message translation directory. All json files in this directory will be merged. (Only m3t files are supported.)
    pub kirikiri_chat_dir: Option<String>,
    #[cfg(feature = "kirikiri")]
    #[arg(long, global = true, value_delimiter = ',')]
    /// Kirikiri language list. First language code is code for language index 1.
    pub kirikiri_languages: Option<Vec<String>>,
    #[cfg(feature = "kirikiri")]
    #[arg(long, global = true, action = ArgAction::SetTrue, visible_alias = "kr-title")]
    /// Whether to handle title in Kirikiri SCN script.
    pub kirikiri_title: bool,
    #[cfg(feature = "kirikiri")]
    #[arg(long, global = true, action = ArgAction::SetTrue, visible_alias = "kr-no-empty-lines", visible_alias = "kirikiri-no-empty-lines")]
    /// Remove empty lines in Kirikiri KS script.
    pub kirikiri_remove_empty_lines: bool,
    #[cfg(feature = "kirikiri")]
    #[arg(
        long,
        global = true,
        value_delimiter = ',',
        default_value = "nm,set_title,speaker,Talk,talk,cn,name,名前"
    )]
    /// Kirikiri name commands, used to extract names from ks script.
    pub kirikiri_name_commands: Vec<String>,
    #[cfg(feature = "kirikiri")]
    #[arg(
        long,
        global = true,
        value_delimiter = ',',
        default_value = "sel01,sel02,sel03,sel04,AddSelect,ruby,exlink,e_xlink"
    )]
    /// Kirikiri message commands, used to extract more message from ks script.
    pub kirikiri_message_commands: Vec<String>,
    #[cfg(feature = "image")]
    #[arg(short = 'f', long, global = true)]
    /// Output multiple image as `<basename>_<name>.<ext>` instead of `<basename>/<name>.<ext>`
    pub image_output_flat: bool,
    #[cfg(feature = "bgi-arc")]
    #[arg(long, global = true, action = ArgAction::SetTrue)]
    /// Whether to compress files in BGI archive when packing BGI archive.
    pub bgi_compress_file: bool,
    #[cfg(feature = "bgi-arc")]
    #[arg(long, global = true, default_value_t = 3, value_parser = crate::scripts::bgi::archive::dsc::parse_min_length)]
    /// Minimum length of match size for DSC compression. Possible values are 2-256.
    pub bgi_compress_min_len: usize,
    #[cfg(feature = "emote-img")]
    #[arg(long, global = true)]
    /// Whether to overlay PIMG images. (By default, true if all layers are not group layers.)
    pub emote_pimg_overlay: Option<bool>,
    #[cfg(feature = "artemis-arc")]
    #[arg(long, global = true)]
    /// Disable Artemis archive (.pfs) XOR encryption when packing.
    pub artemis_arc_disable_xor: bool,
    #[cfg(feature = "artemis")]
    #[arg(long, global = true, group = "artemis_indentg")]
    /// Artemis script indent size, used to format Artemis script.
    /// Default is 4 spaces.
    pub artemis_indent: Option<usize>,
    #[cfg(feature = "artemis")]
    #[arg(long, global = true, action = ArgAction::SetTrue, group = "artemis_indentg")]
    /// Disable Artemis script indent, used to format Artemis script.
    pub artemis_no_indent: bool,
    #[cfg(feature = "artemis")]
    #[arg(long, global = true, default_value_t = 100)]
    /// Max line width in Artemis script, used to format Artemis script.
    pub artemis_max_line_width: usize,
    #[cfg(feature = "artemis")]
    #[arg(long, global = true)]
    /// Specify the language of Artemis AST script.
    /// If not specified, the first language will be used.
    pub artemis_ast_lang: Option<String>,
    #[cfg(feature = "artemis")]
    #[arg(long, global = true, action = ArgAction::SetTrue)]
    /// Do not format lua code in Artemis ASB script(.asb/.iet) when exporting.
    pub artemis_asb_no_format_lua: bool,
    // Default value is from tagFilters in macro.iet
    #[cfg(feature = "artemis-panmimisoft")]
    #[arg(
        long,
        global = true,
        value_delimiter = ',',
        default_value = "背景,イベントCG,遅延背景,遅延背景予約,背景予約,遅延イベントCG,遅延イベントCG予約,イベントCG予約,遅延ポップアップ,遅延bgm_in,遅延bgm_out,遅延se_in,遅延se_out,遅延bgs_in,遅延bgs_out,立ち絵face非連動,セーブサムネイル置換終了,シネスコ,ポップアップ"
    )]
    /// Artemis Engine blacklist tag names for TXT script.
    /// This is used to ignore these tags when finding names in Artemis TXT script (ぱんみみそふと).
    pub artemis_panmimisoft_txt_blacklist_names: Vec<String>,
    #[cfg(feature = "artemis-panmimisoft")]
    #[arg(long, global = true)]
    /// Specify the language of Artemis TXT (ぱんみみそふと) script.
    /// If not specified, the first language will be used.
    pub artemis_panmimisoft_txt_lang: Option<String>,
    #[cfg(feature = "artemis-panmimisoft")]
    #[arg(long, global = true)]
    /// The path to the tag.ini file, which contains the tags to be ignored when finding names in Artemis TXT script (ぱんみみそふと).
    pub artemis_panmimisoft_txt_tag_ini: Option<String>,
    #[cfg(feature = "cat-system")]
    #[arg(long, global = true)]
    /// CatSystem2 CSTL script language, used to extract messages from CSTL script.
    /// If not specified, the first language will be used.
    pub cat_system_cstl_lang: Option<String>,
    #[cfg(feature = "flate2")]
    #[arg(short = 'z', long, global = true, value_name = "LEVEL", value_parser = parse_compression_level, default_value_t = 6)]
    /// Zlib compression level. 0 means no compression, 9 means best compression.
    pub zlib_compression_level: u32,
    #[cfg(feature = "image")]
    #[arg(short = 'g', long, global = true, value_enum, default_value_t = PngCompressionLevel::Fast)]
    /// PNG compression level.
    pub png_compression_level: PngCompressionLevel,
    #[cfg(feature = "circus-img")]
    #[arg(long, global = true, action = ArgAction::SetTrue)]
    /// Keep original BPP when importing Circus CRX images.
    pub circus_crx_keep_original_bpp: bool,
    #[cfg(feature = "circus-img")]
    #[arg(long, global = true, action = ArgAction::SetTrue)]
    /// Use zstd compression for Circus CRX images. (CIRCUS Engine don't support this. Hook is required.)
    pub circus_crx_zstd: bool,
    #[cfg(feature = "zstd")]
    #[arg(short = 'Z', long, global = true, value_name = "LEVEL", value_parser = parse_zstd_compression_level, default_value_t = 3)]
    /// Zstd compression level. 0 means default compression level (3), 22 means best compression.
    pub zstd_compression_level: i32,
    #[cfg(feature = "circus-img")]
    #[arg(long, global = true, value_enum, default_value_t = crate::scripts::circus::image::crx::CircusCrxMode::Auto)]
    /// Circus CRX image row type mode
    pub circus_crx_mode: crate::scripts::circus::image::crx::CircusCrxMode,
    #[cfg(feature = "circus-img")]
    #[arg(long, global = true, action = ArgAction::SetTrue)]
    /// Draw Circus CRX images on canvas (if canvas width and height are specified in file)
    pub circus_crx_canvas: bool,
    #[arg(short = 'F', long, global = true, action = ArgAction::SetTrue)]
    /// Force all files in archive to be treated as script files.
    pub force_script: bool,
    #[cfg(feature = "ex-hibit")]
    #[arg(
        long,
        global = true,
        value_name = "HEX",
        group = "ex_hibit_rld_xor_keyg"
    )]
    /// ExHibit xor key for rld script, in hexadecimal format. (e.g. `12345678`)
    /// Use https://github.com/ZQF-ReVN/RxExHIBIT to find the key.
    pub ex_hibit_rld_xor_key: Option<String>,
    #[cfg(feature = "ex-hibit")]
    #[arg(
        long,
        global = true,
        value_name = "PATH",
        group = "ex_hibit_rld_xor_keyg"
    )]
    /// ExHibit rld xor key file, which contains the xor key in hexadecimal format. (e.g. `0x12345678`)
    pub ex_hibit_rld_xor_key_file: Option<String>,
    #[cfg(feature = "ex-hibit")]
    #[arg(
        long,
        global = true,
        value_name = "HEX",
        group = "ex_hibit_rld_def_xor_keyg"
    )]
    /// ExHibit rld def.rld xor key, in hexadecimal format. (e.g. `12345678`)
    pub ex_hibit_rld_def_xor_key: Option<String>,
    #[cfg(feature = "ex-hibit")]
    #[arg(
        long,
        global = true,
        value_name = "PATH",
        group = "ex_hibit_rld_def_xor_keyg"
    )]
    /// ExHibit rld def.rld xor key file, which contains the xor key in hexadecimal format. (e.g. `0x12345678`)
    pub ex_hibit_rld_def_xor_key_file: Option<String>,
    #[cfg(feature = "ex-hibit")]
    #[arg(long, global = true, value_name = "PATH")]
    /// Path to the ExHibit rld keys file, which contains the keys in BINARY format.
    /// Use https://github.com/ZQF-ReVN/RxExHIBIT to get this file.
    pub ex_hibit_rld_keys: Option<String>,
    #[cfg(feature = "ex-hibit")]
    #[arg(long, global = true, value_name = "PATH")]
    /// Path to the ExHibit rld def keys file, which contains the keys in BINARY format.
    pub ex_hibit_rld_def_keys: Option<String>,
    #[cfg(feature = "mozjpeg")]
    #[arg(short = 'j', long, global = true, default_value_t = 80, value_parser = parse_jpeg_quality)]
    /// JPEG quality for output images, 0-100. 100 means best quality.
    pub jpeg_quality: u8,
    #[cfg(feature = "webp")]
    #[arg(short = 'w', long, global = true, group = "webp_qualityg")]
    /// Use WebP lossless compression for output images.
    pub webp_lossless: bool,
    #[cfg(feature = "webp")]
    #[arg(short = 'W', long, global = true, value_name = "QUALITY", group = "webp_qualityg", value_parser = parse_webp_quality, default_value_t = 80)]
    /// WebP quality for output images, 0-100. 100 means best quality.
    pub webp_quality: u8,
    #[arg(long, global = true)]
    /// Try use YAML format instead of JSON when custom exporting.
    /// By default, this is based on output type. But can be overridden by this option.
    pub custom_yaml: Option<bool>,
    #[cfg(feature = "entis-gls")]
    #[arg(long, global = true)]
    /// Entis GLS srcxml script language, used to extract messages from srcxml script.
    /// If not specified, the first language will be used.
    pub entis_gls_srcxml_lang: Option<String>,
    #[cfg(feature = "will-plus")]
    #[arg(long, global = true)]
    /// Disable disassembly for WillPlus ws2 script.
    /// Use another parser to parse the script.
    /// Should only be used when the default parser not works well.
    pub will_plus_ws2_no_disasm: bool,
    #[cfg(feature = "lossless-audio")]
    #[arg(short = 'l', long, global = true, value_enum, default_value_t = LosslessAudioFormat::Wav)]
    /// Audio format for output lossless audio files.
    pub lossless_audio_fmt: LosslessAudioFormat,
    #[cfg(feature = "audio-flac")]
    #[arg(short = 'L', long, global = true, default_value_t = 5, value_parser = parse_flac_compression_level)]
    /// FLAC compression level for output FLAC audio files. 0 means fastest compression, 8 means best compression.
    pub flac_compression_level: u32,
    #[arg(long, global = true)]
    /// Add a mark to the end of each message for LLM translation.
    /// Only works on m3t format.
    pub llm_trans_mark: Option<String>,
    #[cfg(feature = "favorite")]
    #[arg(long, global = true, action = ArgAction::SetTrue)]
    /// Do not filter ascii strings in Favorite HCB script.
    pub favorite_hcb_no_filter_ascii: bool,
    #[cfg(feature = "image-jxl")]
    #[arg(long, global = true, action = ArgAction::SetTrue, visible_alias = "jxl-no-lossless")]
    /// Disable JXL lossless compression for output images
    pub jxl_lossy: bool,
    #[cfg(feature = "image-jxl")]
    #[arg(long, global = true, default_value_t = 1.0, value_parser = parse_jxl_distance)]
    /// JXL distance for output images. 0 means mathematically lossless compression. 1.0 means visually lossless compression.
    /// Allowed range is 0.0-25.0. Recommended range is 0.5-3.0. Default value is 1
    pub jxl_distance: f32,
    #[cfg(feature = "image-jxl")]
    #[arg(long, global = true, default_value_t = 1, visible_alias = "jxl-jobs")]
    /// Workers count for encode JXL images in parallel. Default is 1.
    /// Set this to 1 to disable parallel encoding. 0 means same as 1
    pub jxl_workers: usize,
    #[cfg(feature = "image")]
    #[arg(short = 'J', long, global = true, default_value_t = crate::types::get_default_threads(), visible_alias = "img-jobs", visible_alias = "img-workers", visible_alias = "image-jobs")]
    /// Workers count for encode images in parallel. Default is half of CPU cores.
    /// Set this to 1 to disable parallel encoding. 0 means same as 1.
    pub image_workers: usize,
    #[cfg(feature = "jieba")]
    #[arg(long, global = true)]
    /// Path to custom jieba dictionary
    pub jieba_dict: Option<String>,
    #[cfg(feature = "emote-img")]
    #[arg(long, global = true, action = ArgAction::SetTrue, visible_alias = "psb-no-tlg")]
    /// Do not process TLG images in PSB files.
    pub psb_no_process_tlg: bool,
    #[cfg(feature = "softpal-img")]
    #[arg(long, global = true, visible_alias = "pgd-co")]
    /// Whether to use compression for Softpal Pgd images.
    /// WARN: Compress may cause image broken.
    pub pgd_compress: bool,
    #[command(subcommand)]
    /// Command
    pub command: Command,
}

#[derive(Parser, Debug)]
#[clap(group = ArgGroup::new("patched_encodingg").multiple(false), group = ArgGroup::new("patched_archive_encodingg").multiple(false))]
pub struct ImportArgs {
    /// Input script file or directory
    pub input: String,
    /// Text file or directory
    pub output: String,
    /// Patched script file or directory
    pub patched: String,
    #[arg(short = 'p', long, group = "patched_encodingg")]
    /// Patched script encoding
    pub patched_encoding: Option<TextEncoding>,
    #[cfg(windows)]
    #[arg(short = 'P', long, group = "patched_encodingg")]
    /// Patched script code page
    pub patched_code_page: Option<u32>,
    #[arg(
        long,
        value_enum,
        group = "patched_archive_encodingg",
        visible_alias = "pa"
    )]
    /// Patched archive filename encoding
    pub patched_archive_encoding: Option<TextEncoding>,
    #[cfg(windows)]
    #[arg(
        long,
        value_enum,
        group = "patched_archive_encodingg",
        visible_alias = "PA"
    )]
    /// Patched archive code page
    pub patched_archive_code_page: Option<u32>,
    #[arg(long)]
    /// Patched script format type
    pub patched_format: Option<FormatType>,
    #[arg(long)]
    /// Fixed length of one line in patched script (for fixed format)
    pub patched_fixed_length: Option<usize>,
    #[arg(long, action = ArgAction::SetTrue)]
    /// Keep original line breaks in patched script (for fixed format)
    pub patched_keep_original: bool,
    #[arg(long, action = ArgAction::SetTrue)]
    /// Break words in patched script (for fixed format)
    pub patched_break_words: bool,
    #[arg(long, action = ArgAction::SetTrue)]
    /// Insert fullwidth space at the start of line in patched script (for fixed format)
    pub patched_insert_fullwidth_space_at_line_start: bool,
    #[arg(long, action = ArgAction::SetTrue)]
    /// If a line break occurs in the middle of some symbols, bring the sentence to next line (for fixed format)
    pub patched_break_with_sentence: bool,
    #[cfg(feature = "jieba")]
    #[arg(long, action = ArgAction::SetTrue)]
    /// Whether to disable break Chinese words at the end of the line.
    pub patched_no_break_chinese_words: bool,
    #[arg(long)]
    /// Name table file
    pub name_csv: Option<String>,
    #[arg(long)]
    /// Replacement table file
    pub replacement_json: Option<String>,
    #[arg(long, action = ArgAction::SetTrue)]
    pub warn_when_output_file_not_found: bool,
}

#[derive(Subcommand, Debug)]
/// Commands
pub enum Command {
    /// Extract from script
    Export {
        /// Input script file or directory
        input: String,
        /// Output file or directory
        output: Option<String>,
    },
    /// Import to script
    Import(ImportArgs),
    /// Pack files to archive
    Pack {
        /// Input directory
        input: String,
        /// Output archive file
        output: Option<String>,
    },
    /// Unpack archive to directory
    Unpack {
        /// Input archive file
        input: String,
        /// Output directory
        output: Option<String>,
    },
    /// Create a new script file
    Create {
        /// Input script
        input: String,
        /// Output script file
        output: Option<String>,
    },
}

pub fn parse_args() -> Arg {
    Arg::parse()
}

#[cfg(feature = "ex-hibit")]
pub fn load_ex_hibit_rld_xor_key(arg: &Arg) -> anyhow::Result<Option<u32>> {
    if let Some(key) = &arg.ex_hibit_rld_xor_key {
        if key.starts_with("0x") {
            return Ok(Some(u32::from_str_radix(&key[2..], 16)?));
        } else {
            return Ok(Some(u32::from_str_radix(key, 16)?));
        }
    }
    if let Some(file) = &arg.ex_hibit_rld_xor_key_file {
        let key = std::fs::read_to_string(file)?.trim().to_string();
        if key.starts_with("0x") {
            return Ok(Some(u32::from_str_radix(&key[2..], 16)?));
        } else {
            return Ok(Some(u32::from_str_radix(&key, 16)?));
        }
    }
    Ok(None)
}

#[cfg(feature = "ex-hibit")]
pub fn load_ex_hibit_rld_def_xor_key(arg: &crate::args::Arg) -> anyhow::Result<Option<u32>> {
    if let Some(key) = &arg.ex_hibit_rld_def_xor_key {
        if key.starts_with("0x") {
            return Ok(Some(u32::from_str_radix(&key[2..], 16)?));
        } else {
            return Ok(Some(u32::from_str_radix(key, 16)?));
        }
    }
    if let Some(file) = &arg.ex_hibit_rld_def_xor_key_file {
        let key = std::fs::read_to_string(file)?.trim().to_string();
        if key.starts_with("0x") {
            return Ok(Some(u32::from_str_radix(&key[2..], 16)?));
        } else {
            return Ok(Some(u32::from_str_radix(&key, 16)?));
        }
    }
    Ok(None)
}

#[cfg(feature = "cat-system-arc")]
pub fn get_cat_system_int_encrypt_password(arg: &Arg) -> anyhow::Result<Option<String>> {
    if let Some(exe) = &arg.cat_system_int_exe {
        return Ok(Some(
            crate::scripts::cat_system::archive::int::get_password_from_exe(exe)?,
        ));
    }
    if let Some(password) = &arg.cat_system_int_encrypt_password {
        return Ok(Some(password.clone()));
    }
    Ok(None)
}

#[cfg(feature = "artemis-panmimisoft")]
pub fn get_artemis_panmimisoft_txt_blacklist_names(
    arg: &Arg,
) -> anyhow::Result<std::collections::HashSet<String>> {
    match &arg.artemis_panmimisoft_txt_tag_ini {
        Some(path) => {
            let mut set = crate::scripts::artemis::panmimisoft::txt::read_tags_from_ini(path)?;
            for name in &arg.artemis_panmimisoft_txt_blacklist_names {
                set.insert(name.clone());
            }
            Ok(set)
        }
        None => Ok(arg
            .artemis_panmimisoft_txt_blacklist_names
            .iter()
            .cloned()
            .collect()),
    }
}

#[cfg(feature = "kirikiri")]
pub fn load_kirikiri_chat_json(
    arg: &Arg,
) -> anyhow::Result<Option<std::sync::Arc<std::collections::HashMap<String, String>>>> {
    if let Some(path) = &arg.kirikiri_chat_json {
        return Ok(Some(crate::scripts::kirikiri::read_kirikiri_comu_json(
            path,
        )?));
    }
    if let Some(dir) = &arg.kirikiri_chat_dir {
        let mut outt = arg.output_type.unwrap_or(OutputScriptType::M3t);
        if !matches!(
            outt,
            OutputScriptType::M3t | OutputScriptType::M3ta | OutputScriptType::M3tTxt
        ) {
            outt = OutputScriptType::M3t;
        }
        let files = crate::utils::files::find_ext_files(dir, arg.recursive, &[outt.as_ref()])?;
        if !files.is_empty() {
            let mut map = std::collections::HashMap::new();
            for file in files {
                let f = crate::utils::files::read_file(&file)?;
                let data = crate::utils::encoding::decode_to_string(
                    crate::get_output_encoding(arg),
                    &f,
                    true,
                )?;
                let m3t = crate::output_scripts::m3t::M3tParser::new(
                    &data,
                    arg.llm_trans_mark.as_ref().map(|s| s.as_str()),
                )
                .parse_as_map()?;
                for (k, v) in m3t {
                    map.insert(k, v);
                }
            }
            return Ok(Some(std::sync::Arc::new(map)));
        }
    }
    Ok(None)
}
