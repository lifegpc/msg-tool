use crate::types::*;
use clap::{ArgAction, ArgGroup, Parser, Subcommand};

/// Tools for export and import scripts
#[derive(Parser, Debug)]
#[clap(group = ArgGroup::new("encodingg").multiple(false), group = ArgGroup::new("output_encodingg").multiple(false), group = ArgGroup::new("archive_encodingg").multiple(false))]
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
