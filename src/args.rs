use crate::types::*;
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
    clap_num::number_range(level, 0, 9)
}

#[cfg(feature = "mozjpeg")]
fn parse_jpeg_quality(quality: &str) -> Result<u8, String> {
    let lower = quality.to_ascii_lowercase();
    if lower == "best" {
        return Ok(100);
    }
    clap_num::number_range(quality, 0, 100)
}

#[cfg(feature = "zstd")]
fn parse_zstd_compression_level(level: &str) -> Result<i32, String> {
    let lower = level.to_ascii_lowercase();
    if lower == "default" {
        return Ok(3);
    } else if lower == "best" {
        return Ok(22);
    }
    clap_num::number_range(level, 0, 22)
}

#[cfg(feature = "webp")]
fn parse_webp_quality(quality: &str) -> Result<u8, String> {
    let lower = quality.to_ascii_lowercase();
    if lower == "best" {
        return Ok(100);
    }
    clap_num::number_range(quality, 0, 100)
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
    #[arg(long, action = ArgAction::SetTrue, global = true, alias = "bgi-no-append")]
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
    #[cfg(feature = "cat-system-arc")]
    #[arg(long, global = true)]
    /// CatSystem2 engine int archive password
    pub cat_system_int_encrypt_password: Option<String>,
    #[cfg(feature = "cat-system-img")]
    #[arg(long, global = true, action = ArgAction::SetTrue)]
    /// Draw CatSystem2 image on canvas (if canvas width and height are specified in file)
    pub cat_system_image_canvas: bool,
    #[cfg(feature = "kirikiri")]
    #[arg(long, global = true)]
    /// Kirikiri language index in script. If not specified, the first language will be used.
    pub kirikiri_language_index: Option<usize>,
    #[cfg(feature = "kirikiri")]
    #[arg(long, global = true, action = ArgAction::SetTrue)]
    /// Export COMU message to extra json file. (for Kirikiri SCN script.)
    /// Only CIRCUS's game have COMU message.
    pub kirikiri_export_comumode: bool,
    #[cfg(feature = "kirikiri")]
    #[arg(long, global = true)]
    /// Kirikiri COMU message translation file. (Map<String, String>, key is original text, value is translated text.)
    pub kirikiri_comumode_json: Option<String>,
    #[cfg(feature = "kirikiri")]
    #[arg(long, global = true, action = ArgAction::SetTrue, alias = "kr-no-empty-lines", alias = "kirikiri-no-empty-lines")]
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
    #[cfg(feature = "kirikiri-img")]
    #[arg(long, global = true)]
    /// Whether to overlay PIMG images. (By default, true if all layers are not group layers.)
    pub kirikiri_pimg_overlay: Option<bool>,
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
    #[arg(long, value_enum, group = "patched_archive_encodingg", alias = "pa")]
    /// Patched archive filename encoding
    pub patched_archive_encoding: Option<TextEncoding>,
    #[cfg(windows)]
    #[arg(long, value_enum, group = "patched_archive_encodingg", alias = "PA")]
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
