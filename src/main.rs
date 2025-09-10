#![cfg_attr(any(docsrs, feature = "unstable"), feature(doc_auto_cfg))]
pub mod args;
pub mod ext;
pub mod format;
pub mod output_scripts;
pub mod scripts;
pub mod types;
pub mod utils;

use ext::path::PathBufExt;
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

fn get_patched_archive_encoding(
    arg: &args::ImportArgs,
    builder: &Box<dyn scripts::ScriptBuilder + Send + Sync>,
    encoding: types::Encoding,
) -> types::Encoding {
    match &arg.patched_archive_encoding {
        Some(enc) => {
            return match enc {
                &types::TextEncoding::Default => {
                    builder.default_archive_encoding().unwrap_or(encoding)
                }
                &types::TextEncoding::Auto => types::Encoding::Utf8,
                &types::TextEncoding::Cp932 => types::Encoding::Cp932,
                &types::TextEncoding::Utf8 => types::Encoding::Utf8,
                &types::TextEncoding::Gb2312 => types::Encoding::Gb2312,
            };
        }
        None => {}
    }
    #[cfg(windows)]
    match &arg.patched_archive_code_page {
        Some(code_page) => {
            return types::Encoding::CodePage(*code_page);
        }
        None => {}
    }
    builder.default_archive_encoding().unwrap_or(encoding)
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
                            None,
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
            builder.build_script_from_file(filename, encoding, archive_encoding, config, None)?,
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
            builder.build_script_from_file(filename, encoding, archive_encoding, config, None)?,
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

pub fn parse_script_from_archive<'a>(
    file: &mut Box<dyn ArchiveContent + 'a>,
    arg: &args::Arg,
    config: &types::ExtraConfig,
    archive: &Box<dyn scripts::Script>,
) -> anyhow::Result<(
    Box<dyn scripts::Script>,
    &'static Box<dyn scripts::ScriptBuilder + Send + Sync>,
)> {
    match file.script_type() {
        Some(typ) => {
            for builder in scripts::BUILDER.iter() {
                if typ == builder.script_type() {
                    let encoding = get_encoding(arg, builder);
                    let archive_encoding = get_archived_encoding(arg, builder, encoding);
                    return Ok((
                        builder.build_script(
                            file.data()?,
                            file.name(),
                            encoding,
                            archive_encoding,
                            config,
                            Some(archive),
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
                file.data()?,
                file.name(),
                encoding,
                archive_encoding,
                config,
                Some(archive),
            )?,
            builder,
        ));
    }
    let buf = file.data()?;
    let mut scores = Vec::new();
    for builder in exts_builder.iter() {
        if let Some(score) = builder.is_this_format(file.name(), buf.as_slice(), buf.len()) {
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
                buf,
                file.name(),
                encoding,
                archive_encoding,
                config,
                Some(archive),
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
    root_dir: Option<&std::path::Path>,
) -> anyhow::Result<types::ScriptResult> {
    eprintln!("Exporting {}", filename);
    let script = parse_script(filename, arg, config)?.0;
    if script.is_archive() {
        let odir = match output.as_ref() {
            Some(output) => {
                let mut pb = std::path::PathBuf::from(output);
                let filename = std::path::PathBuf::from(filename);
                if let Some(root_dir) = root_dir {
                    let rpath = utils::files::relative_path(root_dir, &filename);
                    if let Some(parent) = rpath.parent() {
                        pb.push(parent);
                    }
                    if let Some(fname) = filename.file_name() {
                        pb.push(fname);
                    }
                }
                pb.set_extension("");
                if let Some(ext) = script.archive_output_ext() {
                    pb.set_extension(ext);
                }
                pb.to_string_lossy().into_owned()
            }
            None => {
                let mut pb = std::path::PathBuf::from(filename);
                pb.set_extension("");
                if let Some(ext) = script.archive_output_ext() {
                    pb.set_extension(ext);
                }
                pb.to_string_lossy().into_owned()
            }
        };
        if !std::fs::exists(&odir)? {
            std::fs::create_dir_all(&odir)?;
        }
        for (i, filename) in script.iter_archive_filename()?.enumerate() {
            let filename = match filename {
                Ok(f) => f,
                Err(e) => {
                    eprintln!("Error reading archive filename: {}", e);
                    COUNTER.inc_error();
                    if arg.backtrace {
                        eprintln!("Backtrace: {}", e.backtrace());
                    }
                    continue;
                }
            };
            let mut f = match script.open_file(i) {
                Ok(f) => f,
                Err(e) => {
                    eprintln!("Error opening file {}: {}", filename, e);
                    COUNTER.inc_error();
                    if arg.backtrace {
                        eprintln!("Backtrace: {}", e.backtrace());
                    }
                    continue;
                }
            };
            if arg.force_script || f.is_script() {
                let (script_file, _) = match parse_script_from_archive(&mut f, arg, config, &script)
                {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("Error parsing script '{}' from archive: {}", filename, e);
                        COUNTER.inc_error();
                        if arg.backtrace {
                            eprintln!("Backtrace: {}", e.backtrace());
                        }
                        continue;
                    }
                };
                #[cfg(feature = "image")]
                if script_file.is_image() {
                    if script_file.is_multi_image() {
                        for i in script_file.export_multi_image()? {
                            let img_data = match i {
                                Ok(data) => data,
                                Err(e) => {
                                    eprintln!("Error exporting image: {}", e);
                                    COUNTER.inc_error();
                                    if arg.backtrace {
                                        eprintln!("Backtrace: {}", e.backtrace());
                                    }
                                    continue;
                                }
                            };
                            let out_type = arg.image_type.unwrap_or(types::ImageOutputType::Png);
                            let mut out_path = std::path::PathBuf::from(&odir);
                            if !arg.image_output_flat {
                                out_path.push(f.name());
                                out_path.set_extension("");
                                out_path.push(img_data.name);
                            } else {
                                let name = std::path::Path::new(f.name());
                                out_path.push(format!(
                                    "{}_{}",
                                    name.file_stem().unwrap_or_default().to_string_lossy(),
                                    img_data.name
                                ));
                            }
                            out_path.set_extension(out_type.as_ref());
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
                            utils::img::encode_img(
                                img_data.data,
                                out_type,
                                &out_path.to_string_lossy(),
                                config,
                            )?;
                            COUNTER.inc(types::ScriptResult::Ok);
                        }
                        COUNTER.inc(types::ScriptResult::Ok);
                        continue;
                    }
                    let img_data = match script_file.export_image() {
                        Ok(data) => data,
                        Err(e) => {
                            eprintln!("Error exporting image: {}", e);
                            COUNTER.inc_error();
                            if arg.backtrace {
                                eprintln!("Backtrace: {}", e.backtrace());
                            }
                            continue;
                        }
                    };
                    let out_type = arg.image_type.unwrap_or(types::ImageOutputType::Png);
                    let mut out_path = std::path::PathBuf::from(&odir).join(f.name());
                    out_path.set_extension(out_type.as_ref());
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
                    match utils::img::encode_img(
                        img_data,
                        out_type,
                        &out_path.to_string_lossy(),
                        config,
                    ) {
                        Ok(_) => {}
                        Err(e) => {
                            eprintln!("Error encoding image: {}", e);
                            COUNTER.inc_error();
                            continue;
                        }
                    }
                    COUNTER.inc(types::ScriptResult::Ok);
                    continue;
                }
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
                if arg.output_no_extra_ext {
                    out_path.remove_all_extensions();
                }
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
                    types::OutputScriptType::M3t
                    | types::OutputScriptType::M3ta
                    | types::OutputScriptType::M3tTxt => {
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
                    types::OutputScriptType::Yaml => {
                        let enc = get_output_encoding(arg);
                        let s = match serde_yaml_ng::to_string(&mes) {
                            Ok(s) => s,
                            Err(e) => {
                                eprintln!("Error serializing messages to YAML: {}", e);
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
                    types::OutputScriptType::Pot | types::OutputScriptType::Po => {
                        let enc = get_output_encoding(arg);
                        let s = match output_scripts::po::PoDumper::new().dump(&mes, enc) {
                            Ok(s) => s,
                            Err(e) => {
                                eprintln!("Error dumping messages to PO format: {}", e);
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
                    Ok(mut fi) => match std::io::copy(&mut f, &mut fi) {
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
    #[cfg(feature = "image")]
    if script.is_image() {
        if script.is_multi_image() {
            for i in script.export_multi_image()? {
                let img_data = match i {
                    Ok(data) => data,
                    Err(e) => {
                        eprintln!("Error exporting image: {}", e);
                        COUNTER.inc_error();
                        if arg.backtrace {
                            eprintln!("Backtrace: {}", e.backtrace());
                        }
                        continue;
                    }
                };
                let out_type = arg.image_type.unwrap_or(types::ImageOutputType::Png);
                let f = match output.as_ref() {
                    Some(output) => {
                        if let Some(root_dir) = root_dir {
                            let f = std::path::PathBuf::from(filename);
                            let mut pb = std::path::PathBuf::from(output);
                            let rpath = utils::files::relative_path(root_dir, &f);
                            if let Some(parent) = rpath.parent() {
                                pb.push(parent);
                            }
                            if !arg.image_output_flat {
                                if let Some(fname) = f.file_name() {
                                    pb.push(fname);
                                    if arg.output_no_extra_ext {
                                        pb.remove_all_extensions();
                                    } else {
                                        pb.set_extension("");
                                    }
                                }
                                pb.push(img_data.name);
                            } else {
                                pb.push(format!(
                                    "{}_{}",
                                    f.file_stem().unwrap_or_default().to_string_lossy(),
                                    img_data.name
                                ));
                            }
                            pb.set_extension(out_type.as_ref());
                            pb.to_string_lossy().into_owned()
                        } else {
                            let mut pb = std::path::PathBuf::from(output);
                            if arg.image_output_flat {
                                let f = std::path::PathBuf::from(filename);
                                pb.push(format!(
                                    "{}_{}",
                                    f.file_stem().unwrap_or_default().to_string_lossy(),
                                    img_data.name
                                ));
                            } else {
                                pb.push(img_data.name);
                                if arg.output_no_extra_ext {
                                    pb.remove_all_extensions();
                                } else {
                                    pb.set_extension("");
                                }
                            }
                            pb.set_extension(out_type.as_ref());
                            pb.to_string_lossy().into_owned()
                        }
                    }
                    None => {
                        let mut pb = std::path::PathBuf::from(filename);
                        if arg.image_output_flat {
                            let f = std::path::PathBuf::from(filename);
                            pb.set_file_name(format!(
                                "{}_{}",
                                f.file_stem().unwrap_or_default().to_string_lossy(),
                                img_data.name
                            ));
                        } else {
                            if arg.output_no_extra_ext {
                                pb.remove_all_extensions();
                            } else {
                                pb.set_extension("");
                            }
                            pb.push(img_data.name);
                        }
                        pb.set_extension(out_type.as_ref());
                        pb.to_string_lossy().into_owned()
                    }
                };
                match utils::files::make_sure_dir_exists(&f) {
                    Ok(_) => {}
                    Err(e) => {
                        eprintln!("Error creating parent directory for {}: {}", f, e);
                        COUNTER.inc_error();
                        continue;
                    }
                }
                utils::img::encode_img(img_data.data, out_type, &f, config)?;
                COUNTER.inc(types::ScriptResult::Ok);
            }
            return Ok(types::ScriptResult::Ok);
        }
        let img_data = script.export_image()?;
        let out_type = arg.image_type.unwrap_or_else(|| {
            if root_dir.is_some() {
                types::ImageOutputType::Png
            } else {
                output
                    .as_ref()
                    .and_then(|s| types::ImageOutputType::try_from(std::path::Path::new(s)).ok())
                    .unwrap_or(types::ImageOutputType::Png)
            }
        });
        let f = if filename == "-" {
            String::from("-")
        } else {
            match output.as_ref() {
                Some(output) => {
                    if let Some(root_dir) = root_dir {
                        let f = std::path::PathBuf::from(filename);
                        let mut pb = std::path::PathBuf::from(output);
                        let rpath = utils::files::relative_path(root_dir, &f);
                        if let Some(parent) = rpath.parent() {
                            pb.push(parent);
                        }
                        if let Some(fname) = f.file_name() {
                            pb.push(fname);
                        }
                        if arg.output_no_extra_ext {
                            pb.remove_all_extensions();
                        }
                        pb.set_extension(out_type.as_ref());
                        pb.to_string_lossy().into_owned()
                    } else {
                        output.clone()
                    }
                }
                None => {
                    let mut pb = std::path::PathBuf::from(filename);
                    if arg.output_no_extra_ext {
                        pb.remove_all_extensions();
                    }
                    pb.set_extension(out_type.as_ref());
                    pb.to_string_lossy().into_owned()
                }
            }
        };
        utils::files::make_sure_dir_exists(&f)?;
        utils::img::encode_img(img_data, out_type, &f, config)?;
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
                if let Some(root_dir) = root_dir {
                    let f = std::path::PathBuf::from(filename);
                    let mut pb = std::path::PathBuf::from(output);
                    let rpath = utils::files::relative_path(root_dir, &f);
                    if let Some(parent) = rpath.parent() {
                        pb.push(parent);
                    }
                    if let Some(fname) = f.file_name() {
                        pb.push(fname);
                    }
                    if arg.output_no_extra_ext {
                        pb.remove_all_extensions();
                    }
                    pb.set_extension(ext);
                    pb.to_string_lossy().into_owned()
                } else {
                    output.clone()
                }
            }
            None => {
                let mut pb = std::path::PathBuf::from(filename);
                if arg.output_no_extra_ext {
                    pb.remove_all_extensions();
                }
                pb.set_extension(ext);
                pb.to_string_lossy().into_owned()
            }
        }
    };
    utils::files::make_sure_dir_exists(&f)?;
    match of {
        types::OutputScriptType::Json => {
            let enc = get_output_encoding(arg);
            let s = serde_json::to_string_pretty(&mes)?;
            let b = utils::encoding::encode_string(enc, &s, false)?;
            let mut f = utils::files::write_file(&f)?;
            f.write_all(&b)?;
        }
        types::OutputScriptType::M3t
        | types::OutputScriptType::M3ta
        | types::OutputScriptType::M3tTxt => {
            let enc = get_output_encoding(arg);
            let s = output_scripts::m3t::M3tDumper::dump(&mes);
            let b = utils::encoding::encode_string(enc, &s, false)?;
            let mut f = utils::files::write_file(&f)?;
            f.write_all(&b)?;
        }
        types::OutputScriptType::Yaml => {
            let enc = get_output_encoding(arg);
            let s = serde_yaml_ng::to_string(&mes)?;
            let b = utils::encoding::encode_string(enc, &s, false)?;
            let mut f = utils::files::write_file(&f)?;
            f.write_all(&b)?;
        }
        types::OutputScriptType::Pot | types::OutputScriptType::Po => {
            let enc = get_output_encoding(arg);
            let s = output_scripts::po::PoDumper::new().dump(&mes, enc)?;
            let b = utils::encoding::encode_string(enc, &s, false)?;
            let mut f = utils::files::write_file(&f)?;
            f.write_all(&b)?;
        }
        types::OutputScriptType::Custom => {
            let enc = get_output_encoding(arg);
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
    root_dir: Option<&std::path::Path>,
    name_csv: Option<&std::collections::HashMap<String, String>>,
    repl: Option<&types::ReplacementTable>,
) -> anyhow::Result<types::ScriptResult> {
    eprintln!("Importing {}", filename);
    let (script, builder) = parse_script(filename, arg, config)?;
    if script.is_archive() {
        let odir = {
            let mut pb = std::path::PathBuf::from(&imp_cfg.output);
            let filename = std::path::PathBuf::from(filename);
            if let Some(root_dir) = root_dir {
                let rpath = utils::files::relative_path(root_dir, &filename);
                if let Some(parent) = rpath.parent() {
                    pb.push(parent);
                }
                if let Some(fname) = filename.file_name() {
                    pb.push(fname);
                }
            }
            pb.set_extension("");
            if let Some(ext) = script.archive_output_ext() {
                pb.set_extension(ext);
            }
            pb.to_string_lossy().into_owned()
        };
        let files: Vec<_> = script.iter_archive_filename()?.collect();
        let files = files.into_iter().filter_map(|f| f.ok()).collect::<Vec<_>>();
        let patched_f = if let Some(root_dir) = root_dir {
            let f = std::path::PathBuf::from(filename);
            let mut pb = std::path::PathBuf::from(&imp_cfg.patched);
            let rpath = utils::files::relative_path(root_dir, &f);
            if let Some(parent) = rpath.parent() {
                pb.push(parent);
            }
            if let Some(fname) = f.file_name() {
                pb.push(fname);
            }
            pb.set_extension(builder.extensions().first().unwrap_or(&""));
            pb.to_string_lossy().into_owned()
        } else {
            imp_cfg.patched.clone()
        };
        let files: Vec<_> = files.iter().map(|s| s.as_str()).collect();
        let pencoding = get_patched_encoding(imp_cfg, builder);
        let enc = get_patched_archive_encoding(imp_cfg, builder, pencoding);
        utils::files::make_sure_dir_exists(&patched_f)?;
        let mut arch = builder.create_archive(&patched_f, &files, enc, config)?;
        for (index, filename) in script.iter_archive_filename()?.enumerate() {
            let filename = match filename {
                Ok(f) => f,
                Err(e) => {
                    eprintln!("Error reading archive filename: {}", e);
                    COUNTER.inc_error();
                    if arg.backtrace {
                        eprintln!("Backtrace: {}", e.backtrace());
                    }
                    continue;
                }
            };
            let mut f = match script.open_file(index) {
                Ok(f) => f,
                Err(e) => {
                    eprintln!("Error opening file {}: {}", filename, e);
                    COUNTER.inc_error();
                    if arg.backtrace {
                        eprintln!("Backtrace: {}", e.backtrace());
                    }
                    continue;
                }
            };
            let mut writer = arch.new_file(f.name())?;
            if arg.force_script || f.is_script() {
                let (script_file, _) = match parse_script_from_archive(&mut f, arg, config, &script)
                {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("Error parsing script '{}' from archive: {}", filename, e);
                        COUNTER.inc_error();
                        if arg.backtrace {
                            eprintln!("Backtrace: {}", e.backtrace());
                        }
                        continue;
                    }
                };
                let mut of = match &arg.output_type {
                    Some(t) => t.clone(),
                    None => script_file.default_output_script_type(),
                };
                if !script_file.is_output_supported(of) {
                    of = script_file.default_output_script_type();
                }
                let mut out_path = std::path::PathBuf::from(&odir).join(f.name());
                if arg.output_no_extra_ext {
                    out_path.remove_all_extensions();
                }
                let ext = if of.is_custom() {
                    script_file.custom_output_extension()
                } else {
                    of.as_ref()
                };
                out_path.set_extension(ext);
                if !out_path.exists() {
                    out_path = std::path::PathBuf::from(&odir).join(f.name());
                    if !out_path.exists() {
                        if imp_cfg.warn_when_output_file_not_found {
                            eprintln!(
                                "Warning: File {} does not exist, using file from original archive.",
                                out_path.display()
                            );
                            COUNTER.inc_warning();
                        }
                        match std::io::copy(&mut f, &mut writer) {
                            Ok(_) => {}
                            Err(e) => {
                                eprintln!("Error writing to file {}: {}", out_path.display(), e);
                                COUNTER.inc_error();
                                continue;
                            }
                        }
                        COUNTER.inc(types::ScriptResult::Ok);
                        continue;
                    } else {
                        let file = match std::fs::File::open(&out_path) {
                            Ok(f) => f,
                            Err(e) => {
                                eprintln!("Error opening file {}: {}", out_path.display(), e);
                                COUNTER.inc_error();
                                continue;
                            }
                        };
                        let mut f = std::io::BufReader::new(file);
                        match std::io::copy(&mut f, &mut writer) {
                            Ok(_) => {}
                            Err(e) => {
                                eprintln!("Error writing to file {}: {}", out_path.display(), e);
                                COUNTER.inc_error();
                                continue;
                            }
                        }
                        COUNTER.inc(types::ScriptResult::Ok);
                        continue;
                    }
                }
                let mut mes = match of {
                    types::OutputScriptType::Json => {
                        let enc = get_output_encoding(arg);
                        let b = match utils::files::read_file(&out_path) {
                            Ok(b) => b,
                            Err(e) => {
                                eprintln!("Error reading file {}: {}", out_path.display(), e);
                                COUNTER.inc_error();
                                continue;
                            }
                        };
                        let s = match utils::encoding::decode_to_string(enc, &b, true) {
                            Ok(s) => s,
                            Err(e) => {
                                eprintln!("Error decoding string: {}", e);
                                COUNTER.inc_error();
                                continue;
                            }
                        };
                        match serde_json::from_str::<Vec<types::Message>>(&s) {
                            Ok(mes) => mes,
                            Err(e) => {
                                eprintln!("Error parsing JSON: {}", e);
                                COUNTER.inc_error();
                                continue;
                            }
                        }
                    }
                    types::OutputScriptType::M3t
                    | types::OutputScriptType::M3ta
                    | types::OutputScriptType::M3tTxt => {
                        let enc = get_output_encoding(arg);
                        let b = match utils::files::read_file(&out_path) {
                            Ok(b) => b,
                            Err(e) => {
                                eprintln!("Error reading file {}: {}", out_path.display(), e);
                                COUNTER.inc_error();
                                continue;
                            }
                        };
                        let s = match utils::encoding::decode_to_string(enc, &b, true) {
                            Ok(s) => s,
                            Err(e) => {
                                eprintln!("Error decoding string: {}", e);
                                COUNTER.inc_error();
                                continue;
                            }
                        };
                        let mut parser = output_scripts::m3t::M3tParser::new(
                            &s,
                            arg.llm_trans_mark.as_ref().map(|s| s.as_str()),
                        );
                        match parser.parse() {
                            Ok(mes) => mes,
                            Err(e) => {
                                eprintln!("Error parsing M3T: {}", e);
                                COUNTER.inc_error();
                                continue;
                            }
                        }
                    }
                    types::OutputScriptType::Yaml => {
                        let enc = get_output_encoding(arg);
                        let b = match utils::files::read_file(&out_path) {
                            Ok(b) => b,
                            Err(e) => {
                                eprintln!("Error reading file {}: {}", out_path.display(), e);
                                COUNTER.inc_error();
                                continue;
                            }
                        };
                        let s = match utils::encoding::decode_to_string(enc, &b, true) {
                            Ok(s) => s,
                            Err(e) => {
                                eprintln!("Error decoding string: {}", e);
                                COUNTER.inc_error();
                                continue;
                            }
                        };
                        match serde_yaml_ng::from_str::<Vec<types::Message>>(&s) {
                            Ok(mes) => mes,
                            Err(e) => {
                                eprintln!("Error parsing YAML: {}", e);
                                COUNTER.inc_error();
                                continue;
                            }
                        }
                    }
                    types::OutputScriptType::Pot | types::OutputScriptType::Po => {
                        let enc = get_output_encoding(arg);
                        let b = match utils::files::read_file(&out_path) {
                            Ok(b) => b,
                            Err(e) => {
                                eprintln!("Error reading file {}: {}", out_path.display(), e);
                                COUNTER.inc_error();
                                continue;
                            }
                        };
                        let s = match utils::encoding::decode_to_string(enc, &b, true) {
                            Ok(s) => s,
                            Err(e) => {
                                eprintln!("Error decoding string: {}", e);
                                COUNTER.inc_error();
                                continue;
                            }
                        };
                        let mut parser = output_scripts::po::PoParser::new(
                            &s,
                            arg.llm_trans_mark.as_ref().map(|s| s.as_str()),
                        );
                        match parser.parse() {
                            Ok(mes) => mes,
                            Err(e) => {
                                eprintln!("Error parsing PO: {}", e);
                                COUNTER.inc_error();
                                continue;
                            }
                        }
                    }
                    types::OutputScriptType::Custom => {
                        Vec::new() // Custom scripts handle their own messages
                    }
                };
                if !of.is_custom() && mes.is_empty() {
                    eprintln!("No messages found in {}", f.name());
                    COUNTER.inc(types::ScriptResult::Ignored);
                    continue;
                }
                let encoding = get_patched_encoding(imp_cfg, builder);
                if of.is_custom() {
                    let enc = get_output_encoding(arg);
                    match script_file.custom_import(
                        &out_path.to_string_lossy(),
                        writer,
                        encoding,
                        enc,
                    ) {
                        Ok(_) => {}
                        Err(e) => {
                            eprintln!("Error importing custom script: {}", e);
                            COUNTER.inc_error();
                            continue;
                        }
                    }
                    COUNTER.inc(types::ScriptResult::Ok);
                    continue;
                }
                let fmt = match imp_cfg.patched_format {
                    Some(fmt) => match fmt {
                        types::FormatType::Fixed => types::FormatOptions::Fixed {
                            length: imp_cfg.patched_fixed_length.unwrap_or(32),
                            keep_original: imp_cfg.patched_keep_original,
                            break_words: imp_cfg.patched_break_words,
                            insert_fullwidth_space_at_line_start: imp_cfg
                                .patched_insert_fullwidth_space_at_line_start,
                        },
                        types::FormatType::None => types::FormatOptions::None,
                    },
                    None => script_file.default_format_type(),
                };
                match name_csv {
                    Some(name_table) => {
                        utils::name_replacement::replace_message(&mut mes, name_table);
                    }
                    None => {}
                }
                format::fmt_message(&mut mes, fmt, *builder.script_type());
                if let Err(e) = script_file.import_messages(
                    mes,
                    writer,
                    &out_path.to_string_lossy(),
                    encoding,
                    repl,
                ) {
                    eprintln!("Error importing messages: {}", e);
                    COUNTER.inc_error();
                    continue;
                }
            } else {
                let out_path = std::path::PathBuf::from(&odir).join(f.name());
                if out_path.is_file() {
                    let f = match std::fs::File::open(&out_path) {
                        Ok(f) => f,
                        Err(e) => {
                            eprintln!("Error opening file {}: {}", out_path.display(), e);
                            COUNTER.inc_error();
                            continue;
                        }
                    };
                    let mut f = std::io::BufReader::new(f);
                    match std::io::copy(&mut f, &mut writer) {
                        Ok(_) => {}
                        Err(e) => {
                            eprintln!("Error writing to file {}: {}", out_path.display(), e);
                            COUNTER.inc_error();
                            continue;
                        }
                    }
                } else {
                    eprintln!(
                        "Warning: File {} does not exist, use file from original archive.",
                        out_path.display()
                    );
                    COUNTER.inc_warning();
                    match std::io::copy(&mut f, &mut writer) {
                        Ok(_) => {}
                        Err(e) => {
                            eprintln!("Error writing to file {}: {}", out_path.display(), e);
                            COUNTER.inc_error();
                            continue;
                        }
                    }
                }
            }
            COUNTER.inc(types::ScriptResult::Ok);
        }
        arch.write_header()?;
        return Ok(types::ScriptResult::Ok);
    }
    #[cfg(feature = "image")]
    if script.is_image() {
        let out_type = arg.image_type.unwrap_or_else(|| {
            if root_dir.is_some() {
                types::ImageOutputType::Png
            } else {
                types::ImageOutputType::try_from(std::path::Path::new(&imp_cfg.output))
                    .unwrap_or(types::ImageOutputType::Png)
            }
        });
        let out_f = if let Some(root_dir) = root_dir {
            let f = std::path::PathBuf::from(filename);
            let mut pb = std::path::PathBuf::from(&imp_cfg.output);
            let rpath = utils::files::relative_path(root_dir, &f);
            if let Some(parent) = rpath.parent() {
                pb.push(parent);
            }
            if let Some(fname) = f.file_name() {
                pb.push(fname);
            }
            if arg.output_no_extra_ext {
                pb.remove_all_extensions();
            }
            pb.set_extension(out_type.as_ref());
            pb.to_string_lossy().into_owned()
        } else {
            imp_cfg.output.clone()
        };
        let data = utils::img::decode_img(out_type, &out_f)?;
        let patched_f = if let Some(root_dir) = root_dir {
            let f = std::path::PathBuf::from(filename);
            let mut pb = std::path::PathBuf::from(&imp_cfg.patched);
            let rpath = utils::files::relative_path(root_dir, &f);
            if let Some(parent) = rpath.parent() {
                pb.push(parent);
            }
            if let Some(fname) = f.file_name() {
                pb.push(fname);
            }
            pb.set_extension(builder.extensions().first().unwrap_or(&""));
            pb.to_string_lossy().into_owned()
        } else {
            imp_cfg.patched.clone()
        };
        utils::files::make_sure_dir_exists(&patched_f)?;
        script.import_image_filename(data, &patched_f)?;
        return Ok(types::ScriptResult::Ok);
    }
    let mut of = match &arg.output_type {
        Some(t) => t.clone(),
        None => script.default_output_script_type(),
    };
    if !script.is_output_supported(of) {
        of = script.default_output_script_type();
    }
    let out_f = if let Some(root_dir) = root_dir {
        let f = std::path::PathBuf::from(filename);
        let mut pb = std::path::PathBuf::from(&imp_cfg.output);
        let rpath = utils::files::relative_path(root_dir, &f);
        if let Some(parent) = rpath.parent() {
            pb.push(parent);
        }
        if let Some(fname) = f.file_name() {
            pb.push(fname);
        }
        if arg.output_no_extra_ext {
            pb.remove_all_extensions();
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
            let s = utils::encoding::decode_to_string(enc, &b, true)?;
            serde_json::from_str::<Vec<types::Message>>(&s)?
        }
        types::OutputScriptType::M3t
        | types::OutputScriptType::M3ta
        | types::OutputScriptType::M3tTxt => {
            let enc = get_output_encoding(arg);
            let b = utils::files::read_file(&out_f)?;
            let s = utils::encoding::decode_to_string(enc, &b, true)?;
            let mut parser = output_scripts::m3t::M3tParser::new(
                &s,
                arg.llm_trans_mark.as_ref().map(|s| s.as_str()),
            );
            parser.parse()?
        }
        types::OutputScriptType::Yaml => {
            let enc = get_output_encoding(arg);
            let b = utils::files::read_file(&out_f)?;
            let s = utils::encoding::decode_to_string(enc, &b, true)?;
            serde_yaml_ng::from_str::<Vec<types::Message>>(&s)?
        }
        types::OutputScriptType::Pot | types::OutputScriptType::Po => {
            let enc = get_output_encoding(arg);
            let b = utils::files::read_file(&out_f)?;
            let s = utils::encoding::decode_to_string(enc, &b, true)?;
            let mut parser = output_scripts::po::PoParser::new(
                &s,
                arg.llm_trans_mark.as_ref().map(|s| s.as_str()),
            );
            parser.parse()?
        }
        types::OutputScriptType::Custom => {
            Vec::new() // Custom scripts handle their own messages
        }
    };
    if !of.is_custom() && mes.is_empty() {
        eprintln!("No messages found");
        return Ok(types::ScriptResult::Ignored);
    }
    let encoding = get_patched_encoding(imp_cfg, builder);
    let patched_f = if let Some(root_dir) = root_dir {
        let f = std::path::PathBuf::from(filename);
        let mut pb = std::path::PathBuf::from(&imp_cfg.patched);
        let rpath = utils::files::relative_path(root_dir, &f);
        if let Some(parent) = rpath.parent() {
            pb.push(parent);
        }
        if let Some(fname) = f.file_name() {
            pb.push(fname);
        }
        pb.set_extension(builder.extensions().first().unwrap_or(&""));
        pb.to_string_lossy().into_owned()
    } else {
        imp_cfg.patched.clone()
    };
    utils::files::make_sure_dir_exists(&patched_f)?;
    if of.is_custom() {
        let enc = get_output_encoding(arg);
        script.custom_import_filename(&out_f, &patched_f, encoding, enc)?;
        return Ok(types::ScriptResult::Ok);
    }
    let fmt = match imp_cfg.patched_format {
        Some(fmt) => match fmt {
            types::FormatType::Fixed => types::FormatOptions::Fixed {
                length: imp_cfg.patched_fixed_length.unwrap_or(32),
                keep_original: imp_cfg.patched_keep_original,
                break_words: imp_cfg.patched_break_words,
                insert_fullwidth_space_at_line_start: imp_cfg
                    .patched_insert_fullwidth_space_at_line_start,
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

    script.import_messages_filename(mes, &patched_f, encoding, repl)?;
    Ok(types::ScriptResult::Ok)
}

pub fn pack_archive(
    input: &str,
    output: Option<&str>,
    arg: &args::Arg,
    config: &types::ExtraConfig,
) -> anyhow::Result<()> {
    let typ = match &arg.script_type {
        Some(t) => t,
        None => {
            return Err(anyhow::anyhow!("No script type specified"));
        }
    };
    let (files, isdir) = utils::files::collect_files(input, arg.recursive, true)
        .map_err(|e| anyhow::anyhow!("Error collecting files: {}", e))?;
    if !isdir {
        return Err(anyhow::anyhow!("Input must be a directory for packing"));
    }
    let re_files: Vec<String> = files
        .iter()
        .filter_map(|f| {
            std::path::PathBuf::from(f)
                .strip_prefix(input)
                .ok()
                .and_then(|p| {
                    p.to_str()
                        .map(|s| s.replace("\\", "/").trim_start_matches("/").to_owned())
                })
        })
        .collect();
    let reff = re_files.iter().map(|s| s.as_str()).collect::<Vec<_>>();
    let builder = scripts::BUILDER
        .iter()
        .find(|b| b.script_type() == typ)
        .ok_or_else(|| anyhow::anyhow!("Unsupported script type"))?;
    let output = match output {
        Some(output) => output.to_string(),
        None => {
            let mut pb = std::path::PathBuf::from(input);
            let ext = builder.extensions().first().unwrap_or(&"unk");
            pb.set_extension(ext);
            if pb.to_string_lossy() == input {
                pb.set_extension(format!("{}.{}", ext, ext));
            }
            pb.to_string_lossy().into_owned()
        }
    };
    let mut archive = builder.create_archive(
        &output,
        &reff,
        get_archived_encoding(arg, builder, get_encoding(arg, builder)),
        config,
    )?;
    for (file, name) in files.iter().zip(reff) {
        let mut f = match std::fs::File::open(file) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Error opening file {}: {}", file, e);
                COUNTER.inc_error();
                continue;
            }
        };
        let mut wf = match archive.new_file(name) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Error creating file {} in archive: {}", name, e);
                COUNTER.inc_error();
                continue;
            }
        };
        match std::io::copy(&mut f, &mut wf) {
            Ok(_) => {
                COUNTER.inc(types::ScriptResult::Ok);
            }
            Err(e) => {
                eprintln!("Error writing to file {} in archive: {}", name, e);
                COUNTER.inc_error();
                continue;
            }
        }
    }
    archive.write_header()?;
    Ok(())
}

pub fn unpack_archive(
    filename: &str,
    arg: &args::Arg,
    config: &types::ExtraConfig,
    output: &Option<String>,
    root_dir: Option<&std::path::Path>,
) -> anyhow::Result<types::ScriptResult> {
    eprintln!("Unpacking {}", filename);
    let script = parse_script(filename, arg, config)?.0;
    if !script.is_archive() {
        return Ok(types::ScriptResult::Ignored);
    }
    let odir = match output.as_ref() {
        Some(output) => {
            let mut pb = std::path::PathBuf::from(output);
            let filename = std::path::PathBuf::from(filename);
            if let Some(root_dir) = root_dir {
                let rpath = utils::files::relative_path(root_dir, &filename);
                if let Some(parent) = rpath.parent() {
                    pb.push(parent);
                }
                if let Some(fname) = filename.file_name() {
                    pb.push(fname);
                }
            }
            pb.set_extension("");
            if let Some(ext) = script.archive_output_ext() {
                pb.set_extension(ext);
            }
            pb.to_string_lossy().into_owned()
        }
        None => {
            let mut pb = std::path::PathBuf::from(filename);
            pb.set_extension("");
            if let Some(ext) = script.archive_output_ext() {
                pb.set_extension(ext);
            }
            pb.to_string_lossy().into_owned()
        }
    };
    if !std::fs::exists(&odir)? {
        std::fs::create_dir_all(&odir)?;
    }
    for (index, filename) in script.iter_archive_filename()?.enumerate() {
        let filename = match filename {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Error reading archive filename: {}", e);
                COUNTER.inc_error();
                if arg.backtrace {
                    eprintln!("Backtrace: {}", e.backtrace());
                }
                continue;
            }
        };
        let mut f = match script.open_file(index) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Error opening file {}: {}", filename, e);
                COUNTER.inc_error();
                if arg.backtrace {
                    eprintln!("Backtrace: {}", e.backtrace());
                }
                continue;
            }
        };
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
            Ok(mut fi) => match std::io::copy(&mut f, &mut fi) {
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
        COUNTER.inc(types::ScriptResult::Ok);
    }
    Ok(types::ScriptResult::Ok)
}

pub fn create_file(
    input: &str,
    output: Option<&str>,
    arg: &args::Arg,
    config: &types::ExtraConfig,
) -> anyhow::Result<()> {
    let typ = match &arg.script_type {
        Some(t) => t,
        None => {
            return Err(anyhow::anyhow!("No script type specified"));
        }
    };
    let builder = scripts::BUILDER
        .iter()
        .find(|b| b.script_type() == typ)
        .ok_or_else(|| anyhow::anyhow!("Unsupported script type"))?;

    #[cfg(feature = "image")]
    if builder.is_image() {
        if !builder.can_create_image_file() {
            return Err(anyhow::anyhow!(
                "Script type {:?} does not support image file creation",
                typ
            ));
        }
        let data = utils::img::decode_img(
            arg.image_type.unwrap_or_else(|| {
                types::ImageOutputType::try_from(std::path::Path::new(input))
                    .unwrap_or(types::ImageOutputType::Png)
            }),
            input,
        )?;
        let output = match output {
            Some(output) => output.to_string(),
            None => {
                let mut pb = std::path::PathBuf::from(input);
                let ext = builder.extensions().first().unwrap_or(&"");
                pb.set_extension(ext);
                if pb.to_string_lossy() == input {
                    if ext.is_empty() {
                        pb.set_extension("unk");
                    } else {
                        pb.set_extension(format!("{}.{}", ext, ext));
                    }
                }
                pb.to_string_lossy().into_owned()
            }
        };
        builder.create_image_file_filename(data, &output, config)?;
        return Ok(());
    }

    if !builder.can_create_file() {
        return Err(anyhow::anyhow!(
            "Script type {:?} does not support file creation",
            typ
        ));
    }

    let output = match output {
        Some(output) => output.to_string(),
        None => {
            let mut pb = std::path::PathBuf::from(input);
            let ext = builder.extensions().first().unwrap_or(&"");
            pb.set_extension(ext);
            if pb.to_string_lossy() == input {
                if ext.is_empty() {
                    pb.set_extension("unk");
                } else {
                    pb.set_extension(format!("{}.{}", ext, ext));
                }
            }
            pb.to_string_lossy().into_owned()
        }
    };

    builder.create_file_filename(
        input,
        &output,
        get_encoding(arg, builder),
        get_output_encoding(arg),
        config,
    )?;
    Ok(())
}

lazy_static::lazy_static! {
    static ref COUNTER: utils::counter::Counter = utils::counter::Counter::new();
}

fn main() {
    let _ = ctrlc::try_set_handler(|| {
        eprintln!("Aborted.");
        eprintln!("{}", std::ops::Deref::deref(&COUNTER));
        std::process::exit(1);
    });
    let arg = args::parse_args();
    if arg.backtrace {
        unsafe { std::env::set_var("RUST_LIB_BACKTRACE", "1") };
    }
    let cfg = types::ExtraConfig {
        #[cfg(feature = "circus")]
        circus_mes_type: arg.circus_mes_type.clone(),
        #[cfg(feature = "escude-arc")]
        escude_fake_compress: arg.escude_fake_compress,
        #[cfg(feature = "escude")]
        escude_enum_scr: arg.escude_enum_scr.clone(),
        #[cfg(feature = "bgi")]
        bgi_import_duplicate: arg.bgi_import_duplicate,
        #[cfg(feature = "bgi")]
        bgi_disable_append: arg.bgi_disable_append,
        #[cfg(feature = "image")]
        image_type: arg.image_type.clone(),
        #[cfg(all(feature = "bgi-arc", feature = "bgi-img"))]
        bgi_is_sysgrp_arc: arg.bgi_is_sysgrp_arc.clone(),
        #[cfg(feature = "bgi-img")]
        bgi_img_scramble: arg.bgi_img_scramble.clone(),
        #[cfg(feature = "cat-system-arc")]
        cat_system_int_encrypt_password: args::get_cat_system_int_encrypt_password(&arg)
            .expect("Failed to get CatSystem2 int encrypt password"),
        #[cfg(feature = "cat-system-img")]
        cat_system_image_canvas: arg.cat_system_image_canvas,
        #[cfg(feature = "kirikiri")]
        kirikiri_language_index: arg.kirikiri_language_index.clone(),
        #[cfg(feature = "kirikiri")]
        kirikiri_export_chat: arg.kirikiri_export_chat,
        #[cfg(feature = "kirikiri")]
        kirikiri_chat_key: arg.kirikiri_chat_key.clone(),
        #[cfg(feature = "kirikiri")]
        kirikiri_chat_json: args::load_kirikiri_chat_json(&arg)
            .expect("Failed to load Kirikiri chat JSON"),
        #[cfg(feature = "kirikiri")]
        kirikiri_languages: arg
            .kirikiri_languages
            .clone()
            .map(|s| std::sync::Arc::new(s)),
        #[cfg(feature = "kirikiri")]
        kirikiri_remove_empty_lines: arg.kirikiri_remove_empty_lines,
        #[cfg(feature = "kirikiri")]
        kirikiri_name_commands: std::sync::Arc::new(std::collections::HashSet::from_iter(
            arg.kirikiri_name_commands.iter().cloned(),
        )),
        #[cfg(feature = "kirikiri")]
        kirikiri_message_commands: std::sync::Arc::new(std::collections::HashSet::from_iter(
            arg.kirikiri_message_commands.iter().cloned(),
        )),
        #[cfg(feature = "bgi-arc")]
        bgi_compress_file: arg.bgi_compress_file,
        #[cfg(feature = "bgi-arc")]
        bgi_compress_min_len: arg.bgi_compress_min_len,
        #[cfg(feature = "emote-img")]
        emote_pimg_overlay: arg.emote_pimg_overlay,
        #[cfg(feature = "artemis-arc")]
        artemis_arc_disable_xor: arg.artemis_arc_disable_xor,
        #[cfg(feature = "artemis")]
        artemis_indent: arg.artemis_indent,
        #[cfg(feature = "artemis")]
        artemis_no_indent: arg.artemis_no_indent,
        #[cfg(feature = "artemis")]
        artemis_max_line_width: arg.artemis_max_line_width,
        #[cfg(feature = "artemis")]
        artemis_ast_lang: arg.artemis_ast_lang.clone(),
        #[cfg(feature = "cat-system")]
        cat_system_cstl_lang: arg.cat_system_cstl_lang.clone(),
        #[cfg(feature = "flate2")]
        zlib_compression_level: arg.zlib_compression_level,
        #[cfg(feature = "image")]
        png_compression_level: arg.png_compression_level,
        #[cfg(feature = "circus-img")]
        circus_crx_keep_original_bpp: arg.circus_crx_keep_original_bpp,
        #[cfg(feature = "circus-img")]
        circus_crx_zstd: arg.circus_crx_zstd,
        #[cfg(feature = "zstd")]
        zstd_compression_level: arg.zstd_compression_level,
        #[cfg(feature = "circus-img")]
        circus_crx_mode: arg.circus_crx_mode,
        #[cfg(feature = "ex-hibit")]
        ex_hibit_rld_xor_key: args::load_ex_hibit_rld_xor_key(&arg)
            .expect("Failed to load RLD XOR key"),
        #[cfg(feature = "ex-hibit")]
        ex_hibit_rld_def_xor_key: args::load_ex_hibit_rld_def_xor_key(&arg)
            .expect("Failed to load RLD DEF XOR key"),
        #[cfg(feature = "ex-hibit")]
        ex_hibit_rld_keys: scripts::ex_hibit::rld::load_keys(arg.ex_hibit_rld_keys.as_ref())
            .expect("Failed to load RLD keys"),
        #[cfg(feature = "ex-hibit")]
        ex_hibit_rld_def_keys: scripts::ex_hibit::rld::load_keys(
            arg.ex_hibit_rld_def_keys.as_ref(),
        )
        .expect("Failed to load RLD DEF keys"),
        #[cfg(feature = "mozjpeg")]
        jpeg_quality: arg.jpeg_quality,
        #[cfg(feature = "webp")]
        webp_lossless: arg.webp_lossless,
        #[cfg(feature = "webp")]
        webp_quality: arg.webp_quality,
        #[cfg(feature = "circus-img")]
        circus_crx_canvas: arg.circus_crx_canvas,
        custom_yaml: arg.custom_yaml.unwrap_or_else(|| {
            arg.output_type
                .map(|s| s == types::OutputScriptType::Yaml)
                .unwrap_or(false)
        }),
        #[cfg(feature = "entis-gls")]
        entis_gls_srcxml_lang: arg.entis_gls_srcxml_lang.clone(),
        #[cfg(feature = "will-plus")]
        will_plus_ws2_no_disasm: arg.will_plus_ws2_no_disasm,
        #[cfg(feature = "artemis-panmimisoft")]
        artemis_panmimisoft_txt_blacklist_names: std::sync::Arc::new(
            args::get_artemis_panmimisoft_txt_blacklist_names(&arg).unwrap(),
        ),
        #[cfg(feature = "artemis-panmimisoft")]
        artemis_panmimisoft_txt_lang: arg.artemis_panmimisoft_txt_lang.clone(),
        #[cfg(feature = "lossless-audio")]
        lossless_audio_fmt: arg.lossless_audio_fmt,
        #[cfg(feature = "audio-flac")]
        flac_compression_level: arg.flac_compression_level,
        #[cfg(feature = "artemis")]
        artemis_asb_format_lua: !arg.artemis_asb_no_format_lua,
        #[cfg(feature = "kirikiri")]
        kirikiri_title: arg.kirikiri_title,
        #[cfg(feature = "favorite")]
        favorite_hcb_filter_ascii: !arg.favorite_hcb_no_filter_ascii,
    };
    match &arg.command {
        args::Command::Export { input, output } => {
            let (scripts, is_dir) =
                utils::files::collect_files(input, arg.recursive, false).unwrap();
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
            let root_dir = if is_dir {
                Some(std::path::Path::new(input))
            } else {
                None
            };
            for script in scripts.iter() {
                let re = export_script(&script, &arg, &cfg, output, root_dir);
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
                utils::files::collect_files(&args.input, arg.recursive, false).unwrap();
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
            let root_dir = if is_dir {
                Some(std::path::Path::new(&args.input))
            } else {
                None
            };
            for script in scripts.iter() {
                let re = import_script(
                    &script,
                    &arg,
                    &cfg,
                    args,
                    root_dir,
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
        args::Command::Pack { input, output } => {
            let re = pack_archive(input, output.as_ref().map(|s| s.as_str()), &arg, &cfg);
            if let Err(e) = re {
                COUNTER.inc_error();
                eprintln!("Error packing archive: {}", e);
            }
        }
        args::Command::Unpack { input, output } => {
            let (scripts, is_dir) = utils::files::collect_arc_files(input, arg.recursive).unwrap();
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
            let root_dir = if is_dir {
                Some(std::path::Path::new(input))
            } else {
                None
            };
            for script in scripts.iter() {
                let re = unpack_archive(&script, &arg, &cfg, output, root_dir);
                match re {
                    Ok(s) => {
                        COUNTER.inc(s);
                    }
                    Err(e) => {
                        COUNTER.inc_error();
                        eprintln!("Error unpacking {}: {}", script, e);
                        if arg.backtrace {
                            eprintln!("Backtrace: {}", e.backtrace());
                        }
                    }
                }
            }
        }
        args::Command::Create { input, output } => {
            let re = create_file(input, output.as_ref().map(|s| s.as_str()), &arg, &cfg);
            if let Err(e) = re {
                COUNTER.inc_error();
                eprintln!("Error creating file: {}", e);
                if arg.backtrace {
                    eprintln!("Backtrace: {}", e.backtrace());
                }
            }
        }
    }
    eprintln!("{}", std::ops::Deref::deref(&COUNTER));
}
