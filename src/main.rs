pub mod args;
pub mod output_scripts;
pub mod scripts;
pub mod types;
pub mod utils;

fn get_encoding(
    arg: &args::Arg,
    builder: &Box<dyn scripts::ScriptBuilder + Send + Sync>,
) -> types::Encoding {
    match &arg.encoding {
        Some(enc) => {
            return match enc {
                &types::TextEncoding::Default => builder.default_encoding(),
                &types::TextEncoding::Auto => types::Encoding::Auto,
                &types::TextEncoding::Cp932 => types::Encoding::Cp932,
                &types::TextEncoding::Utf8 => types::Encoding::Utf8,
                &types::TextEncoding::Gb2312 => types::Encoding::Gb2312,
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
                &types::TextEncoding::Default => types::Encoding::Utf8,
                &types::TextEncoding::Auto => types::Encoding::Utf8,
                &types::TextEncoding::Cp932 => types::Encoding::Cp932,
                &types::TextEncoding::Utf8 => types::Encoding::Utf8,
                &types::TextEncoding::Gb2312 => types::Encoding::Gb2312,
            };
        }
        None => {}
    }
    #[cfg(windows)]
    match &arg.output_code_page {
        Some(code_page) => {
            return types::Encoding::CodePage(*code_page);
        }
        None => {}
    }
    types::Encoding::Utf8
}

fn get_patched_encoding(
    arg: &args::ImportArgs,
    builder: &Box<dyn scripts::ScriptBuilder + Send + Sync>,
) -> types::Encoding {
    match &arg.patched_encoding {
        Some(enc) => {
            return match enc {
                &types::TextEncoding::Default => builder.default_patched_encoding(),
                &types::TextEncoding::Auto => types::Encoding::Utf8,
                &types::TextEncoding::Cp932 => types::Encoding::Cp932,
                &types::TextEncoding::Utf8 => types::Encoding::Utf8,
                &types::TextEncoding::Gb2312 => types::Encoding::Gb2312,
            };
        }
        None => {}
    }
    #[cfg(windows)]
    match &arg.patched_code_page {
        Some(code_page) => {
            return types::Encoding::CodePage(*code_page);
        }
        None => {}
    }
    builder.default_patched_encoding()
}

pub fn parse_script(
    filename: &str,
    arg: &args::Arg,
    config: &types::ExtraConfig,
) -> anyhow::Result<(
    Box<dyn scripts::Script>,
    &'static Box<dyn scripts::ScriptBuilder + Send + Sync>,
)> {
    match &arg.script_type {
        Some(typ) => {
            for builder in scripts::BUILDER.iter() {
                if typ == builder.script_type() {
                    let encoding = get_encoding(arg, builder);
                    return Ok((builder.build_script(filename, encoding, config)?, builder));
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
                return Ok((builder.build_script(filename, encoding, config)?, builder));
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
    let script = parse_script(filename, arg, config)?.0;
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
        types::OutputScriptType::M3t => {
            let enc = get_output_encoding(arg);
            let s = output_scripts::m3t::M3tDumper::dump(&mes);
            let b = utils::encoding::encode_string(enc, &s)?;
            let mut f = utils::files::write_file(&f)?;
            f.write_all(&b)?;
        }
    }
    Ok(())
}

pub fn import_script(
    filename: &str,
    arg: &args::Arg,
    config: &types::ExtraConfig,
    imp_cfg: &args::ImportArgs,
    is_dir: bool,
) -> anyhow::Result<()> {
    eprintln!("Importing {}", filename);
    let (script, builder) = parse_script(filename, arg, config)?;
    let of = match &arg.output_type {
        Some(t) => t.clone(),
        None => script.default_output_script_type(),
    };
    let out_f = if is_dir {
        let f = std::path::PathBuf::from(filename);
        let mut pb = std::path::PathBuf::from(&imp_cfg.output);
        if let Some(fname) = f.file_name() {
            pb.push(fname);
        }
        pb.set_extension(of.as_ref());
        pb.to_string_lossy().into_owned()
    } else {
        imp_cfg.output.clone()
    };
    if !std::fs::exists(&out_f).unwrap_or(false) {
        eprintln!("Output file does not exist");
        return Ok(());
    }
    let mes = match of {
        types::OutputScriptType::Json => {
            let enc = get_output_encoding(arg);
            let b = utils::files::read_file(&out_f)?;
            let s = utils::encoding::decode_to_string(enc, &b)?;
            serde_json::from_str::<Vec<types::Message>>(&s)?
        }
        types::OutputScriptType::M3t => {
            let enc = get_output_encoding(arg);
            let b = utils::files::read_file(&out_f)?;
            let s = utils::encoding::decode_to_string(enc, &b)?;
            let mut parser = output_scripts::m3t::M3tParser::new(&s);
            parser.parse()?
        }
    };
    if mes.is_empty() {
        eprintln!("No messages found");
        return Ok(());
    }
    let encoding = get_patched_encoding(imp_cfg, builder);
    let patched_f = if is_dir {
        let f = std::path::PathBuf::from(filename);
        let mut pb = std::path::PathBuf::from(&imp_cfg.patched);
        if let Some(fname) = f.file_name() {
            pb.push(fname);
        }
        pb.set_extension(builder.extensions().first().unwrap_or(&""));
        pb.to_string_lossy().into_owned()
    } else {
        imp_cfg.patched.clone()
    };
    script.import_messages(mes, &patched_f, encoding)?;
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
                    None => {}
                }
            }
            for script in scripts.iter() {
                let re = export_script(&script, &arg, &cfg, output, is_dir);
                match re {
                    Ok(_) => {}
                    Err(e) => {
                        eprintln!("Error exporting {}: {}", script, e);
                        if arg.backtrace {
                            eprintln!("Backtrace: {}", e.backtrace());
                        }
                    }
                }
            }
        }
        args::Command::Import(args) => {
            let (scripts, is_dir) =
                utils::files::collect_files(&args.input, arg.recursive).unwrap();
            if is_dir {
                let pb = std::path::Path::new(&args.patched);
                if pb.exists() {
                    if !pb.is_dir() {
                        eprintln!("Patched path is not a directory");
                        return;
                    }
                } else {
                    std::fs::create_dir_all(pb).unwrap();
                }
            }
            for script in scripts.iter() {
                let re = import_script(&script, &arg, &cfg, args, is_dir);
                match re {
                    Ok(_) => {}
                    Err(e) => {
                        eprintln!("Error exporting {}: {}", script, e);
                        if arg.backtrace {
                            eprintln!("Backtrace: {}", e.backtrace());
                        }
                    }
                }
            }
        }
    }
}
