pub mod args;
pub mod ext;
pub mod format;
pub mod output_scripts;
pub mod scripts;
pub mod types;
pub mod utils;

use scripts::base::ArchiveContent;

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

fn get_archived_encoding(
    arg: &args::Arg,
    builder: &Box<dyn scripts::ScriptBuilder + Send + Sync>,
    encoding: types::Encoding,
) -> types::Encoding {
    match &arg.archive_encoding {
        Some(enc) => {
            return match enc {
                &types::TextEncoding::Default => builder
                    .default_archive_encoding()
                    .unwrap_or_else(|| builder.default_encoding()),
                &types::TextEncoding::Auto => types::Encoding::Auto,
                &types::TextEncoding::Cp932 => types::Encoding::Cp932,
                &types::TextEncoding::Utf8 => types::Encoding::Utf8,
                &types::TextEncoding::Gb2312 => types::Encoding::Gb2312,
            };
        }
        None => {}
    }
    #[cfg(windows)]
    match &arg.archive_code_page {
        Some(code_page) => {
            return types::Encoding::CodePage(*code_page);
        }
        None => {}
    }
    builder.default_archive_encoding().unwrap_or(encoding)
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
                    let archive_encoding = get_archived_encoding(arg, builder, encoding);
                    return Ok((
                        builder.build_script_from_file(
                            filename,
                            encoding,
                            archive_encoding,
                            config,
                        )?,
                        builder,
                    ));
                }
            }
        }
        _ => {}
    }
    let mut exts_builder = Vec::new();
    for builder in scripts::BUILDER.iter() {
        let exts = builder.extensions();
        for ext in exts {
            if filename.to_lowercase().ends_with(ext) {
                exts_builder.push(builder);
                break;
            }
        }
    }
    let exts_builder = if exts_builder.is_empty() {
        scripts::BUILDER.iter().collect::<Vec<_>>()
    } else {
        exts_builder
    };
    if exts_builder.len() == 1 {
        let builder = exts_builder.first().unwrap();
        let encoding = get_encoding(arg, builder);
        let archive_encoding = get_archived_encoding(arg, builder, encoding);
        return Ok((
            builder.build_script_from_file(filename, encoding, archive_encoding, config)?,
            builder,
        ));
    }
    let mut buf = [0u8; 1024];
    let mut size = 0;
    if filename != "-" {
        let mut f = std::fs::File::open(filename)?;
        size = std::io::Read::read(&mut f, &mut buf)?;
    }
    let mut scores = Vec::new();
    for builder in exts_builder.iter() {
        if let Some(score) = builder.is_this_format(filename, &buf, size) {
            scores.push((score, builder));
        }
    }
    if scores.is_empty() {
        return Err(anyhow::anyhow!("Unsupported script type"));
    }
    let max_score = scores.iter().map(|s| s.0).max().unwrap();
    let mut best_builders = Vec::new();
    for (score, builder) in scores.iter() {
        if *score == max_score {
            best_builders.push(builder);
        }
    }
    if best_builders.len() == 1 {
        let builder = best_builders.first().unwrap();
        let encoding = get_encoding(arg, builder);
        let archive_encoding = get_archived_encoding(arg, builder, encoding);
        return Ok((
            builder.build_script_from_file(filename, encoding, archive_encoding, config)?,
            builder,
        ));
    }
    if best_builders.len() > 1 {
        eprintln!(
            "Multiple script types found for {}: {:?}",
            filename, best_builders
        );
        return Err(anyhow::anyhow!("Multiple script types found"));
    }
    Err(anyhow::anyhow!("Unsupported script type"))
}

pub fn parse_script_from_archive(
    file: &Box<dyn ArchiveContent>,
    arg: &args::Arg,
    config: &types::ExtraConfig,
) -> anyhow::Result<(
    Box<dyn scripts::Script>,
    &'static Box<dyn scripts::ScriptBuilder + Send + Sync>,
)> {
    let mut exts_builder = Vec::new();
    for builder in scripts::BUILDER.iter() {
        let exts = builder.extensions();
        for ext in exts {
            if file.name().to_lowercase().ends_with(ext) {
                exts_builder.push(builder);
                break;
            }
        }
    }
    let exts_builder = if exts_builder.is_empty() {
        scripts::BUILDER.iter().collect::<Vec<_>>()
    } else {
        exts_builder
    };
    if exts_builder.len() == 1 {
        let builder = exts_builder.first().unwrap();
        let encoding = get_encoding(arg, builder);
        let archive_encoding = get_archived_encoding(arg, builder, encoding);
        return Ok((
            builder.build_script(
                file.data().to_vec(),
                file.name(),
                encoding,
                archive_encoding,
                config,
            )?,
            builder,
        ));
    }
    let mut scores = Vec::new();
    for builder in exts_builder.iter() {
        if let Some(score) = builder.is_this_format(file.name(), file.data(), file.data().len()) {
            scores.push((score, builder));
        }
    }
    if scores.is_empty() {
        return Err(anyhow::anyhow!("Unsupported script type"));
    }
    let max_score = scores.iter().map(|s| s.0).max().unwrap();
    let mut best_builders = Vec::new();
    for (score, builder) in scores.iter() {
        if *score == max_score {
            best_builders.push(builder);
        }
    }
    if best_builders.len() == 1 {
        let builder = best_builders.first().unwrap();
        let encoding = get_encoding(arg, builder);
        let archive_encoding = get_archived_encoding(arg, builder, encoding);
        return Ok((
            builder.build_script(
                file.data().to_vec(),
                file.name(),
                encoding,
                archive_encoding,
                config,
            )?,
            builder,
        ));
    }
    if best_builders.len() > 1 {
        eprintln!(
            "Multiple script types found for {}: {:?}",
            file.name(),
            best_builders
        );
        return Err(anyhow::anyhow!("Multiple script types found"));
    }
    Err(anyhow::anyhow!("Unsupported script type"))
}

pub fn export_script(
    filename: &str,
    arg: &args::Arg,
    config: &types::ExtraConfig,
    output: &Option<String>,
    is_dir: bool,
) -> anyhow::Result<types::ScriptResult> {
    eprintln!("Exporting {}", filename);
    let mut script = parse_script(filename, arg, config)?.0;
    if script.is_archive() {
        let odir = match output.as_ref() {
            Some(output) => {
                let mut pb = std::path::PathBuf::from(output);
                let filename = std::path::PathBuf::from(filename);
                if let Some(fname) = filename.file_name() {
                    pb.push(fname);
                }
                pb.to_string_lossy().into_owned()
            }
            None => {
                let mut pb = std::path::PathBuf::from(filename);
                pb.set_extension("");
                pb.to_string_lossy().into_owned()
            }
        };
        if !std::fs::exists(&odir)? {
            std::fs::create_dir_all(&odir)?;
        }
        for f in script.iter_archive()? {
            let f = f?;
            if f.is_script() {
                let (script_file, _) = parse_script_from_archive(&f, arg, config)?;
                let mut of = match &arg.output_type {
                    Some(t) => t.clone(),
                    None => script_file.default_output_script_type(),
                };
                if !script_file.is_output_supported(of) {
                    of = script_file.default_output_script_type();
                }
                let mes = if of.is_custom() {
                    Vec::new()
                } else {
                    match script_file.extract_messages() {
                        Ok(mes) => mes,
                        Err(e) => {
                            eprintln!("Error extracting messages from {}: {}", f.name(), e);
                            COUNTER.inc_error();
                            if arg.backtrace {
                                eprintln!("Backtrace: {}", e.backtrace());
                            }
                            continue;
                        }
                    }
                };
                if !of.is_custom() && mes.is_empty() {
                    eprintln!("No messages found in {}", f.name());
                    COUNTER.inc(types::ScriptResult::Ignored);
                    continue;
                }
                let mut out_path = std::path::PathBuf::from(&odir).join(f.name());
                out_path.set_extension(if of.is_custom() {
                    script_file.custom_output_extension()
                } else {
                    of.as_ref()
                });
                match utils::files::make_sure_dir_exists(&out_path) {
                    Ok(_) => {}
                    Err(e) => {
                        eprintln!(
                            "Error creating parent directory for {}: {}",
                            out_path.display(),
                            e
                        );
                        COUNTER.inc_error();
                        continue;
                    }
                }
                match of {
                    types::OutputScriptType::Json => {
                        let enc = get_output_encoding(arg);
                        let s = match serde_json::to_string_pretty(&mes) {
                            Ok(s) => s,
                            Err(e) => {
                                eprintln!("Error serializing messages to JSON: {}", e);
                                COUNTER.inc_error();
                                continue;
                            }
                        };
                        let b = match utils::encoding::encode_string(enc, &s, false) {
                            Ok(b) => b,
                            Err(e) => {
                                eprintln!("Error encoding string: {}", e);
                                COUNTER.inc_error();
                                continue;
                            }
                        };
                        let mut f = match utils::files::write_file(&out_path) {
                            Ok(f) => f,
                            Err(e) => {
                                eprintln!("Error writing file {}: {}", out_path.display(), e);
                                COUNTER.inc_error();
                                continue;
                            }
                        };
                        match f.write_all(&b) {
                            Ok(_) => {}
                            Err(e) => {
                                eprintln!("Error writing to file {}: {}", out_path.display(), e);
                                COUNTER.inc_error();
                                continue;
                            }
                        }
                    }
                    types::OutputScriptType::M3t => {
                        let enc = get_output_encoding(arg);
                        let s = output_scripts::m3t::M3tDumper::dump(&mes);
                        let b = match utils::encoding::encode_string(enc, &s, false) {
                            Ok(b) => b,
                            Err(e) => {
                                eprintln!("Error encoding string: {}", e);
                                COUNTER.inc_error();
                                continue;
                            }
                        };
                        let mut f = match utils::files::write_file(&out_path) {
                            Ok(f) => f,
                            Err(e) => {
                                eprintln!("Error writing file {}: {}", out_path.display(), e);
                                COUNTER.inc_error();
                                continue;
                            }
                        };
                        match f.write_all(&b) {
                            Ok(_) => {}
                            Err(e) => {
                                eprintln!("Error writing to file {}: {}", out_path.display(), e);
                                COUNTER.inc_error();
                                continue;
                            }
                        }
                    }
                    types::OutputScriptType::Custom => {
                        let enc = get_output_encoding(arg);
                        if let Err(e) = script_file.custom_export(&out_path, enc) {
                            eprintln!("Error exporting custom script: {}", e);
                            COUNTER.inc_error();
                            continue;
                        }
                    }
                }
            } else {
                let out_path = std::path::PathBuf::from(&odir).join(f.name());
                match utils::files::make_sure_dir_exists(&out_path) {
                    Ok(_) => {}
                    Err(e) => {
                        eprintln!(
                            "Error creating parent directory for {}: {}",
                            out_path.display(),
                            e
                        );
                        COUNTER.inc_error();
                        continue;
                    }
                }
                match utils::files::write_file(&out_path) {
                    Ok(mut fi) => match fi.write_all(f.data()) {
                        Ok(_) => {}
                        Err(e) => {
                            eprintln!("Error writing to file {}: {}", out_path.display(), e);
                            COUNTER.inc_error();
                            continue;
                        }
                    },
                    Err(e) => {
                        eprintln!("Error writing file {}: {}", out_path.display(), e);
                        COUNTER.inc_error();
                        continue;
                    }
                }
            }
            COUNTER.inc(types::ScriptResult::Ok);
        }
        return Ok(types::ScriptResult::Ok);
    }
    let mut of = match &arg.output_type {
        Some(t) => t.clone(),
        None => script.default_output_script_type(),
    };
    if !script.is_output_supported(of) {
        of = script.default_output_script_type();
    }
    let mes = if of.is_custom() {
        Vec::new()
    } else {
        script.extract_messages()?
    };
    if !of.is_custom() && mes.is_empty() {
        eprintln!("No messages found");
        return Ok(types::ScriptResult::Ignored);
    }
    let ext = if of.is_custom() {
        script.custom_output_extension()
    } else {
        of.as_ref()
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
                    pb.set_extension(ext);
                    pb.to_string_lossy().into_owned()
                } else {
                    output.clone()
                }
            }
            None => {
                let mut pb = std::path::PathBuf::from(filename);
                pb.set_extension(ext);
                pb.to_string_lossy().into_owned()
            }
        }
    };
    match of {
        types::OutputScriptType::Json => {
            let enc = get_output_encoding(arg);
            let s = serde_json::to_string_pretty(&mes)?;
            let b = utils::encoding::encode_string(enc, &s, false)?;
            let mut f = utils::files::write_file(&f)?;
            f.write_all(&b)?;
        }
        types::OutputScriptType::M3t => {
            let enc = get_output_encoding(arg);
            let s = output_scripts::m3t::M3tDumper::dump(&mes);
            let b = utils::encoding::encode_string(enc, &s, false)?;
            let mut f = utils::files::write_file(&f)?;
            f.write_all(&b)?;
        }
        types::OutputScriptType::Custom => {
            let enc = get_output_encoding(arg);
            println!("f: {}", f);
            script.custom_export(f.as_ref(), enc)?;
        }
    }
    Ok(types::ScriptResult::Ok)
}

pub fn import_script(
    filename: &str,
    arg: &args::Arg,
    config: &types::ExtraConfig,
    imp_cfg: &args::ImportArgs,
    is_dir: bool,
    name_csv: Option<&std::collections::HashMap<String, String>>,
    repl: Option<&types::ReplacementTable>,
) -> anyhow::Result<types::ScriptResult> {
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
        return Ok(types::ScriptResult::Ignored);
    }
    let mut mes = match of {
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
        _ => {
            eprintln!("Unsupported output script type for import: {:?}", of);
            return Ok(types::ScriptResult::Ignored);
        }
    };
    if mes.is_empty() {
        eprintln!("No messages found");
        return Ok(types::ScriptResult::Ignored);
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
    let fmt = match imp_cfg.patched_format {
        Some(fmt) => match fmt {
            types::FormatType::Fixed => types::FormatOptions::Fixed {
                length: imp_cfg.patched_fixed_length.unwrap_or(32),
                keep_original: imp_cfg.patched_keep_original,
            },
            types::FormatType::None => types::FormatOptions::None,
        },
        None => script.default_format_type(),
    };
    match name_csv {
        Some(name_table) => {
            utils::name_replacement::replace_message(&mut mes, name_table);
        }
        None => {}
    }
    format::fmt_message(&mut mes, fmt, *builder.script_type());

    script.import_messages(mes, &patched_f, encoding, repl)?;
    Ok(types::ScriptResult::Ok)
}

lazy_static::lazy_static! {
    static ref COUNTER: utils::counter::Counter = utils::counter::Counter::new();
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
                    Ok(s) => {
                        COUNTER.inc(s);
                    }
                    Err(e) => {
                        COUNTER.inc_error();
                        eprintln!("Error exporting {}: {}", script, e);
                        if arg.backtrace {
                            eprintln!("Backtrace: {}", e.backtrace());
                        }
                    }
                }
            }
        }
        args::Command::Import(args) => {
            let name_csv = match &args.name_csv {
                Some(name_csv) => {
                    let name_table = utils::name_replacement::read_csv(name_csv).unwrap();
                    Some(name_table)
                }
                None => None,
            };
            let repl = match &args.replacement_json {
                Some(replacement_json) => {
                    let b = utils::files::read_file(replacement_json).unwrap();
                    let s = String::from_utf8(b).unwrap();
                    let table = serde_json::from_str::<types::ReplacementTable>(&s).unwrap();
                    Some(table)
                }
                None => None,
            };
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
                let re = import_script(
                    &script,
                    &arg,
                    &cfg,
                    args,
                    is_dir,
                    name_csv.as_ref(),
                    repl.as_ref(),
                );
                match re {
                    Ok(s) => {
                        COUNTER.inc(s);
                    }
                    Err(e) => {
                        COUNTER.inc_error();
                        eprintln!("Error exporting {}: {}", script, e);
                        if arg.backtrace {
                            eprintln!("Backtrace: {}", e.backtrace());
                        }
                    }
                }
            }
        }
    }
    eprintln!("{}", std::ops::Deref::deref(&COUNTER));
}
