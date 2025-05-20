use crate::types::*;
use clap::{ArgAction, ArgGroup, Parser, Subcommand};

/// Tools for export and import scripts
#[derive(Parser, Debug)]
#[clap(group = ArgGroup::new("encodingg").multiple(false), group = ArgGroup::new("output_encodingg").multiple(false))]
#[command(version, about, long_about = None)]
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
    #[arg(long, value_enum, global = true)]
    /// Circus Game
    pub circus_mes_type: Option<CircusMesType>,
    #[arg(short, long, action = ArgAction::SetTrue, global = true)]
    /// Search for script files in the directory recursively
    pub recursive: bool,
    #[arg(global = true, action = ArgAction::SetTrue, short, long)]
    /// Print backtrace on error
    pub backtrace: bool,
    #[command(subcommand)]
    /// Command
    pub command: Command,
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
}

pub fn parse_args() -> Arg {
    Arg::parse()
}
