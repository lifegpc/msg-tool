pub mod args;
pub mod scripts;
pub mod types;
pub mod utils;

fn get_encoding(arg: &args::Arg, builder: &Box<dyn scripts::ScriptBuilder + Send + Sync>) -> types::Encoding {
    match &arg.encoding {
        Some(enc) => {
            return match enc {
                &types::TextEncoding::Default => {
                    builder.default_encoding()
                }
                &types::TextEncoding::Auto => {
                    types::Encoding::Auto
                }
                &types::TextEncoding::Cp932 => {
                    types::Encoding::Cp932
                }
                &types::TextEncoding::Utf8 => {
                    types::Encoding::Utf8
                }
                &types::TextEncoding::Gb2312 => {
                    types::Encoding::Gb2312
                }
            };
        }
        None => {}
    }
    #[cfg(windows)]
    match &arg.code_page {
        Some(code_page) => {
            return types::Encoding::CodePage(*code_page);
        }
        None => {}
    }
    builder.default_encoding()
}

fn get_output_encoding(arg: &args::Arg) -> types::Encoding {
    match &arg.output_encoding {
        Some(enc) => {
            return match enc {
                &types::TextEncoding::Default => {
                    types::Encoding::Utf8
                }
                &types::TextEncoding::Auto => {
                    types::Encoding::Utf8
                }
                &types::TextEncoding::Cp932 => {
                    types::Encoding::Cp932
                }
                &types::TextEncoding::Utf8 => {
                    types::Encoding::Utf8
                }
                &types::TextEncoding::Gb2312 => {
                    types::Encoding::Gb2312
                }
            };
        }
        None => {}
    }
    #[cfg(windows)]
    match &arg.code_page {
        Some(code_page) => {
            return types::Encoding::CodePage(*code_page);
        }
        None => {}
    }
    types::Encoding::Utf8
}

pub fn parse_script(filename: &str, arg: &args::Arg, config: &types::ExtraConfig) -> anyhow::Result<Box<dyn scripts::Script>> {
    match &arg.script_type {
        Some(typ) => {
            for builder in scripts::BUILDER.iter() {
                if typ == builder.script_type() {
                    let encoding = get_encoding(arg, builder);
                    return Ok(builder.build_script(filename, encoding, config)?);
                }
            }
        }
        _ => {}
    }
    for builder in scripts::BUILDER.iter() {
        let exts = builder.extensions();
        for ext in exts {
            if filename.to_lowercase().ends_with(ext) {
                let encoding = get_encoding(arg, builder);
                return Ok(builder.build_script(filename, encoding, config)?);
            }
        }
    }
    Err(anyhow::anyhow!("Unsupported script type"))
}

pub fn export_script(
    filename: &str,
    arg: &args::Arg,
    config: &types::ExtraConfig,
    output: &Option<String>,
    is_dir: bool,
) -> anyhow::Result<()> {
    eprintln!("Exporting {}", filename);
    let script = parse_script(filename, arg, config)?;
    // println!("{:?}", script);
    let mes = script.extract_messages()?;
    // for m in mes.iter() {
    //     println!("{:?}", m);
    // }
    if mes.is_empty() {
        eprintln!("No messages found");
        return Ok(());
    }
    let of = match &arg.output_type {
        Some(t) => t.clone(),
        None => script.default_output_script_type(),
    };
    let f = if filename == "-" {
        String::from("-")
    } else {
        match output.as_ref() {
            Some(output) => {
                if is_dir {
                    let f = std::path::PathBuf::from(filename);
                    let mut pb = std::path::PathBuf::from(output);
                    if let Some(fname) = f.file_name() {
                        pb.push(fname);
                    }
                    pb.set_extension(of.as_ref());
                    pb.to_string_lossy().into_owned()
                } else {
                    output.clone()
                }
            }
            None => {
                let mut pb = std::path::PathBuf::from(filename);
                pb.set_extension(of.as_ref());
                pb.to_string_lossy().into_owned()
            }
        }
    };
    match of {
        types::OutputScriptType::Json => {
            let enc = get_output_encoding(arg);
            let s = serde_json::to_string_pretty(&mes)?;
            let b = utils::encoding::encode_string(enc, &s)?;
            let mut f = utils::files::write_file(&f)?;
            f.write_all(&b)?;
        }
        _ => {}
    }
    Ok(())
}

fn main() {
    let arg = args::parse_args();
    if arg.backtrace {
        unsafe { std::env::set_var("RUST_LIB_BACKTRACE", "1") };
    }
    let cfg = types::ExtraConfig {
        circus_mes_type: arg.circus_mes_type.clone(),
    };
    match &arg.command {
        args::Command::Export { input, output } => {
            let (scripts, is_dir) = utils::files::collect_files(input, arg.recursive).unwrap();
            if is_dir {
                match &output {
                    Some(output) => {
                        let op = std::path::Path::new(output);
                        if op.exists() {
                            if !op.is_dir() {
                                eprintln!("Output path is not a directory");
                                return;
                            }
                        } else {
                            std::fs::create_dir_all(op).unwrap();
                        }
                    }
                    None => {
                        eprintln!("Output path is not specified");
                        return;
                    }
                }
            }
            for script in scripts.iter() {
                let re = export_script(&script, &arg, &cfg, output, is_dir);
                match re {
                    Ok(_) => {
                    }
                    Err(e) => {
                        eprintln!("Error exporting {}: {}", script, e);
                        if arg.backtrace {
                            eprintln!("Backtrace: {:?}", e.backtrace());
                        }
                    }
                }
            }
        }
    }
}
