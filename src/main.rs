#![cfg_attr(any(docsrs, feature = "unstable"), feature(doc_cfg))]
pub mod args;
pub mod ext;
pub mod format;
pub mod output_scripts;
pub mod scripts;
pub mod types;
pub mod utils;

use ext::path::PathBufExt;
use scripts::base::ArchiveContent;

fn escape_dep_string(s: &str) -> String {
    s.replace("\\", "\\\\").replace(" ", "\\ ")
}

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

fn get_input_output_script_encoding(arg: &args::Arg) -> types::Encoding {
    match &arg.encoding {
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
    match &arg.code_page {
        Some(code_page) => {
            return types::Encoding::CodePage(*code_page);
        }
        None => {}
    }
    types::Encoding::Utf8
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
    config: std::sync::Arc<types::ExtraConfig>,
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
                            &config,
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
            builder.build_script_from_file(filename, encoding, archive_encoding, &config, None)?,
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
            builder.build_script_from_file(filename, encoding, archive_encoding, &config, None)?,
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
    config: std::sync::Arc<types::ExtraConfig>,
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
                            &config,
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
                &config,
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
                &config,
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
    config: std::sync::Arc<types::ExtraConfig>,
    output: &Option<String>,
    root_dir: Option<&std::path::Path>,
    #[cfg(feature = "image")] img_threadpool: Option<
        &utils::threadpool::ThreadPool<Result<(), anyhow::Error>>,
    >,
) -> anyhow::Result<types::ScriptResult> {
    eprintln!("Exporting {}", filename);
    let script = parse_script(filename, arg, config.clone())?.0;
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
                let (script_file, _) =
                    match parse_script_from_archive(&mut f, arg, config.clone(), &script) {
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
                            if let Some(threadpool) = img_threadpool {
                                let outpath = out_path.to_string_lossy().into_owned();
                                let config = config.clone();
                                threadpool.execute(
                                    move |_| {
                                        utils::img::encode_img(
                                            img_data.data,
                                            out_type,
                                            &outpath,
                                            &config,
                                        )
                                        .map_err(|e| {
                                            anyhow::anyhow!(
                                                "Failed to encode image {}: {}",
                                                outpath,
                                                e
                                            )
                                        })
                                    },
                                    true,
                                )?;
                                continue;
                            } else {
                                match utils::img::encode_img(
                                    img_data.data,
                                    out_type,
                                    &out_path.to_string_lossy(),
                                    &config,
                                ) {
                                    Ok(_) => {}
                                    Err(e) => {
                                        eprintln!("Error encoding image: {}", e);
                                        COUNTER.inc_error();
                                        continue;
                                    }
                                }
                                COUNTER.inc(types::ScriptResult::Ok);
                            }
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
                    if let Some(threadpool) = img_threadpool {
                        let outpath = out_path.to_string_lossy().into_owned();
                        let config = config.clone();
                        threadpool.execute(
                            move |_| {
                                utils::img::encode_img(img_data, out_type, &outpath, &config)
                                    .map_err(|e| {
                                        anyhow::anyhow!("Failed to encode image {}: {}", outpath, e)
                                    })
                            },
                            true,
                        )?;
                        continue;
                    } else {
                        match utils::img::encode_img(
                            img_data,
                            out_type,
                            &out_path.to_string_lossy(),
                            &config,
                        ) {
                            Ok(_) => {}
                            Err(e) => {
                                eprintln!("Error encoding image: {}", e);
                                COUNTER.inc_error();
                                continue;
                            }
                        }
                        COUNTER.inc(types::ScriptResult::Ok);
                    }
                    continue;
                }
                let mut of = match &arg.output_type {
                    Some(t) => t.clone(),
                    None => script_file.default_output_script_type(),
                };
                if !script_file.is_output_supported(of) {
                    of = script_file.default_output_script_type();
                }
                if !arg.no_multi_message && !of.is_custom() && script_file.multiple_message_files()
                {
                    let mmes = script_file.extract_multiple_messages()?;
                    if mmes.is_empty() {
                        eprintln!("No messages found in {}", f.name());
                        COUNTER.inc(types::ScriptResult::Ignored);
                        continue;
                    }
                    let ext = of.as_ref();
                    let mut out_dir = std::path::PathBuf::from(&odir).join(f.name());
                    if arg.output_no_extra_ext {
                        out_dir.remove_all_extensions();
                    } else {
                        out_dir.set_extension("");
                    }
                    std::fs::create_dir_all(&out_dir)?;
                    for (name, data) in mmes {
                        let ofp = out_dir.join(name).with_extension(ext);
                        match of {
                            types::OutputScriptType::Json => {
                                let enc = get_output_encoding(arg);
                                let s = match serde_json::to_string_pretty(&data) {
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
                                let mut f = match utils::files::write_file(&ofp) {
                                    Ok(f) => f,
                                    Err(e) => {
                                        eprintln!("Error writing file {}: {}", ofp.display(), e);
                                        COUNTER.inc_error();
                                        continue;
                                    }
                                };
                                match f.write_all(&b) {
                                    Ok(_) => {}
                                    Err(e) => {
                                        eprintln!("Error writing to file {}: {}", ofp.display(), e);
                                        COUNTER.inc_error();
                                        continue;
                                    }
                                }
                            }
                            types::OutputScriptType::M3t
                            | types::OutputScriptType::M3ta
                            | types::OutputScriptType::M3tTxt => {
                                let enc = get_output_encoding(arg);
                                let s =
                                    output_scripts::m3t::M3tDumper::dump(&data, arg.m3t_no_quote);
                                let b = match utils::encoding::encode_string(enc, &s, false) {
                                    Ok(b) => b,
                                    Err(e) => {
                                        eprintln!("Error encoding string: {}", e);
                                        COUNTER.inc_error();
                                        continue;
                                    }
                                };
                                let mut f = match utils::files::write_file(&ofp) {
                                    Ok(f) => f,
                                    Err(e) => {
                                        eprintln!("Error writing file {}: {}", ofp.display(), e);
                                        COUNTER.inc_error();
                                        continue;
                                    }
                                };
                                match f.write_all(&b) {
                                    Ok(_) => {}
                                    Err(e) => {
                                        eprintln!("Error writing to file {}: {}", ofp.display(), e);
                                        COUNTER.inc_error();
                                        continue;
                                    }
                                }
                            }
                            types::OutputScriptType::Yaml => {
                                let enc = get_output_encoding(arg);
                                let s = match serde_yaml_ng::to_string(&data) {
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
                                let mut f = match utils::files::write_file(&ofp) {
                                    Ok(f) => f,
                                    Err(e) => {
                                        eprintln!("Error writing file {}: {}", ofp.display(), e);
                                        COUNTER.inc_error();
                                        continue;
                                    }
                                };
                                match f.write_all(&b) {
                                    Ok(_) => {}
                                    Err(e) => {
                                        eprintln!("Error writing to file {}: {}", ofp.display(), e);
                                        COUNTER.inc_error();
                                        continue;
                                    }
                                }
                            }
                            types::OutputScriptType::Pot | types::OutputScriptType::Po => {
                                let enc = get_output_encoding(arg);
                                let s = match output_scripts::po::PoDumper::new().dump(&data, enc) {
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
                                let mut f = match utils::files::write_file(&ofp) {
                                    Ok(f) => f,
                                    Err(e) => {
                                        eprintln!("Error writing file {}: {}", ofp.display(), e);
                                        COUNTER.inc_error();
                                        continue;
                                    }
                                };
                                match f.write_all(&b) {
                                    Ok(_) => {}
                                    Err(e) => {
                                        eprintln!("Error writing to file {}: {}", ofp.display(), e);
                                        COUNTER.inc_error();
                                        continue;
                                    }
                                }
                            }
                            types::OutputScriptType::Custom => {}
                        }
                    }
                    COUNTER.inc(types::ScriptResult::Ok);
                    continue;
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
                        let s = output_scripts::m3t::M3tDumper::dump(&mes, arg.m3t_no_quote);
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
                if let Some(threadpool) = img_threadpool {
                    let outpath = f.clone();
                    let config = config.clone();
                    threadpool.execute(
                        move |_| {
                            utils::img::encode_img(img_data.data, out_type, &outpath, &config)
                                .map_err(|e| {
                                    anyhow::anyhow!("Failed to encode image {}: {}", outpath, e)
                                })
                        },
                        true,
                    )?;
                    continue;
                } else {
                    match utils::img::encode_img(img_data.data, out_type, &f, &config) {
                        Ok(_) => {}
                        Err(e) => {
                            eprintln!("Error encoding image: {}", e);
                            COUNTER.inc_error();
                            continue;
                        }
                    }
                    COUNTER.inc(types::ScriptResult::Ok);
                }
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
        if let Some(threadpool) = img_threadpool {
            let outpath = f.clone();
            let config = config.clone();
            threadpool.execute(
                move |_| {
                    utils::img::encode_img(img_data, out_type, &outpath, &config)
                        .map_err(|e| anyhow::anyhow!("Failed to encode image {}: {}", outpath, e))
                },
                true,
            )?;
            return Ok(types::ScriptResult::Uncount);
        } else {
            utils::img::encode_img(img_data, out_type, &f, &config)?;
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
    if !arg.no_multi_message && !of.is_custom() && script.multiple_message_files() {
        let mmes = script.extract_multiple_messages()?;
        if mmes.is_empty() {
            eprintln!("No messages found");
            return Ok(types::ScriptResult::Ignored);
        }
        let ext = of.as_ref();
        let out_dir = if let Some(output) = output.as_ref() {
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
                } else {
                    pb.set_extension("");
                }
                pb.to_string_lossy().into_owned()
            } else {
                output.clone()
            }
        } else {
            let mut pb = std::path::PathBuf::from(filename);
            if arg.output_no_extra_ext {
                pb.remove_all_extensions();
            } else {
                pb.set_extension("");
            }
            pb.to_string_lossy().into_owned()
        };
        std::fs::create_dir_all(&out_dir)?;
        let outdir = std::path::PathBuf::from(&out_dir);
        for (name, data) in mmes {
            let ofp = outdir.join(name).with_extension(ext);
            match of {
                types::OutputScriptType::Json => {
                    let enc = get_output_encoding(arg);
                    let s = match serde_json::to_string_pretty(&data) {
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
                    let mut f = match utils::files::write_file(&ofp) {
                        Ok(f) => f,
                        Err(e) => {
                            eprintln!("Error writing file {}: {}", ofp.display(), e);
                            COUNTER.inc_error();
                            continue;
                        }
                    };
                    match f.write_all(&b) {
                        Ok(_) => {}
                        Err(e) => {
                            eprintln!("Error writing to file {}: {}", ofp.display(), e);
                            COUNTER.inc_error();
                            continue;
                        }
                    }
                }
                types::OutputScriptType::M3t
                | types::OutputScriptType::M3ta
                | types::OutputScriptType::M3tTxt => {
                    let enc = get_output_encoding(arg);
                    let s = output_scripts::m3t::M3tDumper::dump(&data, arg.m3t_no_quote);
                    let b = match utils::encoding::encode_string(enc, &s, false) {
                        Ok(b) => b,
                        Err(e) => {
                            eprintln!("Error encoding string: {}", e);
                            COUNTER.inc_error();
                            continue;
                        }
                    };
                    let mut f = match utils::files::write_file(&ofp) {
                        Ok(f) => f,
                        Err(e) => {
                            eprintln!("Error writing file {}: {}", ofp.display(), e);
                            COUNTER.inc_error();
                            continue;
                        }
                    };
                    match f.write_all(&b) {
                        Ok(_) => {}
                        Err(e) => {
                            eprintln!("Error writing to file {}: {}", ofp.display(), e);
                            COUNTER.inc_error();
                            continue;
                        }
                    }
                }
                types::OutputScriptType::Yaml => {
                    let enc = get_output_encoding(arg);
                    let s = match serde_yaml_ng::to_string(&data) {
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
                    let mut f = match utils::files::write_file(&ofp) {
                        Ok(f) => f,
                        Err(e) => {
                            eprintln!("Error writing file {}: {}", ofp.display(), e);
                            COUNTER.inc_error();
                            continue;
                        }
                    };
                    match f.write_all(&b) {
                        Ok(_) => {}
                        Err(e) => {
                            eprintln!("Error writing to file {}: {}", ofp.display(), e);
                            COUNTER.inc_error();
                            continue;
                        }
                    }
                }
                types::OutputScriptType::Pot | types::OutputScriptType::Po => {
                    let enc = get_output_encoding(arg);
                    let s = match output_scripts::po::PoDumper::new().dump(&data, enc) {
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
                    let mut f = match utils::files::write_file(&ofp) {
                        Ok(f) => f,
                        Err(e) => {
                            eprintln!("Error writing file {}: {}", ofp.display(), e);
                            COUNTER.inc_error();
                            continue;
                        }
                    };
                    match f.write_all(&b) {
                        Ok(_) => {}
                        Err(e) => {
                            eprintln!("Error writing to file {}: {}", ofp.display(), e);
                            COUNTER.inc_error();
                            continue;
                        }
                    }
                }
                types::OutputScriptType::Custom => {}
            }
            COUNTER.inc(types::ScriptResult::Ok);
        }
        return Ok(types::ScriptResult::Ok);
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
            let s = output_scripts::m3t::M3tDumper::dump(&mes, arg.m3t_no_quote);
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
    config: std::sync::Arc<types::ExtraConfig>,
    imp_cfg: &args::ImportArgs,
    root_dir: Option<&std::path::Path>,
    name_csv: Option<&std::collections::HashMap<String, String>>,
    repl: Option<&types::ReplacementTable>,
    mut dep_graph: Option<&mut (String, Vec<String>)>,
) -> anyhow::Result<types::ScriptResult> {
    eprintln!("Importing {}", filename);
    if let Some(dep_graph) = dep_graph.as_mut() {
        dep_graph.1.push(filename.to_string());
    }
    let (script, builder) = parse_script(filename, arg, config.clone())?;
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
        if let Some(dep_graph) = dep_graph.as_mut() {
            dep_graph.0 = patched_f.clone();
        }
        let files: Vec<_> = files.iter().map(|s| s.as_str()).collect();
        let pencoding = get_patched_encoding(imp_cfg, builder);
        let enc = get_patched_archive_encoding(imp_cfg, builder, pencoding);
        utils::files::make_sure_dir_exists(&patched_f)?;
        let mut arch = builder.create_archive(&patched_f, &files, enc, &config)?;
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
            if arg.force_script || f.is_script() {
                let mut writer = arch.new_file(f.name(), None)?;
                let (script_file, _) =
                    match parse_script_from_archive(&mut f, arg, config.clone(), &script) {
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
                if !arg.no_multi_message && !of.is_custom() && script_file.multiple_message_files()
                {
                    let out_dir = std::path::PathBuf::from(&odir)
                        .join(f.name())
                        .with_extension("");
                    let outfiles = utils::files::find_ext_files(
                        &out_dir.to_string_lossy(),
                        false,
                        &[of.as_ref()],
                    )?;
                    if outfiles.is_empty() {
                        if imp_cfg.warn_when_output_file_not_found {
                            eprintln!(
                                "Warning: No output files found in {}, using file from original archive.",
                                out_dir.display()
                            );
                            COUNTER.inc_warning();
                        } else {
                            COUNTER.inc(types::ScriptResult::Ignored);
                        }
                        continue;
                    }
                    if let Some(dep_graph) = dep_graph.as_mut() {
                        dep_graph.1.extend_from_slice(&outfiles);
                    }
                    let fmt = match imp_cfg.patched_format {
                        Some(fmt) => match fmt {
                            types::FormatType::Fixed => types::FormatOptions::Fixed {
                                length: imp_cfg.patched_fixed_length.unwrap_or(32),
                                keep_original: imp_cfg.patched_keep_original,
                                break_words: imp_cfg.patched_break_words,
                                insert_fullwidth_space_at_line_start: imp_cfg
                                    .patched_insert_fullwidth_space_at_line_start,
                                break_with_sentence: imp_cfg.patched_break_with_sentence,
                                #[cfg(feature = "jieba")]
                                break_chinese_words: !imp_cfg.patched_no_break_chinese_words,
                                #[cfg(feature = "jieba")]
                                jieba_dict: arg.jieba_dict.clone(),
                            },
                            types::FormatType::None => types::FormatOptions::None,
                        },
                        None => script.default_format_type(),
                    };
                    let mut mmes = std::collections::HashMap::new();
                    for out_f in outfiles {
                        let name = utils::files::relative_path(&out_dir, &out_f)
                            .with_extension("")
                            .to_string_lossy()
                            .into_owned();
                        let mut mes = match of {
                            types::OutputScriptType::Json => {
                                let enc = get_output_encoding(arg);
                                let b = match utils::files::read_file(&out_f) {
                                    Ok(b) => b,
                                    Err(e) => {
                                        eprintln!("Error reading file {}: {}", out_f, e);
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
                                let b = match utils::files::read_file(&out_f) {
                                    Ok(b) => b,
                                    Err(e) => {
                                        eprintln!("Error reading file {}: {}", out_f, e);
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
                                let b = match utils::files::read_file(&out_f) {
                                    Ok(b) => b,
                                    Err(e) => {
                                        eprintln!("Error reading file {}: {}", out_f, e);
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
                                let b = match utils::files::read_file(&out_f) {
                                    Ok(b) => b,
                                    Err(e) => {
                                        eprintln!("Error reading file {}: {}", out_f, e);
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
                                match output_scripts::po::PoParser::new(
                                    &s,
                                    arg.llm_trans_mark.as_ref().map(|s| s.as_str()),
                                )
                                .parse()
                                {
                                    Ok(mes) => mes,
                                    Err(e) => {
                                        eprintln!("Error parsing PO: {}", e);
                                        COUNTER.inc_error();
                                        continue;
                                    }
                                }
                            }
                            types::OutputScriptType::Custom => Vec::new(),
                        };
                        if mes.is_empty() {
                            eprintln!(
                                "No messages found in {}, using file from original archive.",
                                out_f
                            );
                            continue;
                        }
                        match name_csv {
                            Some(name_table) => {
                                utils::name_replacement::replace_message(&mut mes, name_table);
                            }
                            None => {}
                        }
                        format::fmt_message(&mut mes, fmt.clone(), *builder.script_type())?;
                        mmes.insert(name, mes);
                    }
                    if mmes.is_empty() {
                        COUNTER.inc(types::ScriptResult::Ignored);
                        continue;
                    }
                    let encoding = get_patched_encoding(imp_cfg, builder);
                    match script_file.import_multiple_messages(
                        mmes,
                        writer,
                        f.name(),
                        encoding,
                        repl,
                    ) {
                        Ok(_) => {}
                        Err(e) => {
                            eprintln!("Error importing messages to script '{}': {}", filename, e);
                            COUNTER.inc_error();
                            if arg.backtrace {
                                eprintln!("Backtrace: {}", e.backtrace());
                            }
                            continue;
                        }
                    }
                    COUNTER.inc(types::ScriptResult::Ok);
                    continue;
                }
                #[cfg(feature = "image")]
                if script_file.is_image() {
                    let out_type = arg.image_type.unwrap_or(types::ImageOutputType::Png);
                    let mut out_path = std::path::PathBuf::from(&odir).join(f.name());
                    if arg.output_no_extra_ext {
                        out_path.remove_all_extensions();
                    }
                    out_path.set_extension(out_type.as_ref());
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
                                    eprintln!(
                                        "Error writing to file {}: {}",
                                        out_path.display(),
                                        e
                                    );
                                    COUNTER.inc_error();
                                    continue;
                                }
                            }
                        } else {
                            if let Some(dep_graph) = dep_graph.as_mut() {
                                dep_graph.1.push(out_path.to_string_lossy().into_owned());
                            }
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
                                    eprintln!(
                                        "Error writing to file {}: {}",
                                        out_path.display(),
                                        e
                                    );
                                    COUNTER.inc_error();
                                    continue;
                                }
                            }
                        }
                    }
                    if let Some(dep_graph) = dep_graph.as_mut() {
                        dep_graph.1.push(out_path.to_string_lossy().into_owned());
                    }
                    let img_data =
                        match utils::img::decode_img(out_type, &out_path.to_string_lossy()) {
                            Ok(data) => data,
                            Err(e) => {
                                eprintln!("Error decoding image {}: {}", out_path.display(), e);
                                COUNTER.inc_error();
                                continue;
                            }
                        };
                    if let Err(err) = script_file.import_image(img_data, writer) {
                        eprintln!("Error importing image to script '{}': {}", filename, err);
                        COUNTER.inc_error();
                        if arg.backtrace {
                            eprintln!("Backtrace: {}", err.backtrace());
                        }
                        continue;
                    }
                    continue;
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
                        if let Some(dep_graph) = dep_graph.as_mut() {
                            dep_graph.1.push(out_path.to_string_lossy().into_owned());
                        }
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
                if let Some(dep_graph) = dep_graph.as_mut() {
                    dep_graph.1.push(out_path.to_string_lossy().into_owned());
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
                            break_with_sentence: imp_cfg.patched_break_with_sentence,
                            #[cfg(feature = "jieba")]
                            break_chinese_words: !imp_cfg.patched_no_break_chinese_words,
                            #[cfg(feature = "jieba")]
                            jieba_dict: arg.jieba_dict.clone(),
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
                format::fmt_message(&mut mes, fmt, *builder.script_type())?;
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
                let size = if out_path.is_file() {
                    match std::fs::metadata(&out_path) {
                        Ok(meta) => Some(meta.len()),
                        Err(e) => {
                            eprintln!(
                                "Error getting metadata for file {}: {}",
                                out_path.display(),
                                e
                            );
                            COUNTER.inc_error();
                            continue;
                        }
                    }
                } else {
                    None
                };
                let mut writer = arch.new_file_non_seek(f.name(), size)?;
                if out_path.is_file() {
                    if let Some(dep_graph) = dep_graph.as_mut() {
                        dep_graph.1.push(out_path.to_string_lossy().into_owned());
                    }
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
        if let Some(dep_graph) = dep_graph.as_mut() {
            dep_graph.1.push(out_f.clone());
        }
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
        if let Some(dep_graph) = dep_graph.as_mut() {
            dep_graph.0 = patched_f.clone();
        }
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
    if !arg.no_multi_message && !of.is_custom() && script.multiple_message_files() {
        let out_dir = if let Some(root_dir) = root_dir {
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
            } else {
                pb.set_extension("");
            }
            pb.to_string_lossy().into_owned()
        } else {
            imp_cfg.output.clone()
        };
        let outfiles = utils::files::find_ext_files(&out_dir, false, &[of.as_ref()])?;
        if outfiles.is_empty() {
            eprintln!("No output files found");
            return Ok(types::ScriptResult::Ignored);
        }
        if let Some(dep_graph) = dep_graph.as_mut() {
            dep_graph.1.extend_from_slice(&outfiles);
        }
        let fmt = match imp_cfg.patched_format {
            Some(fmt) => match fmt {
                types::FormatType::Fixed => types::FormatOptions::Fixed {
                    length: imp_cfg.patched_fixed_length.unwrap_or(32),
                    keep_original: imp_cfg.patched_keep_original,
                    break_words: imp_cfg.patched_break_words,
                    insert_fullwidth_space_at_line_start: imp_cfg
                        .patched_insert_fullwidth_space_at_line_start,
                    break_with_sentence: imp_cfg.patched_break_with_sentence,
                    #[cfg(feature = "jieba")]
                    break_chinese_words: !imp_cfg.patched_no_break_chinese_words,
                    #[cfg(feature = "jieba")]
                    jieba_dict: arg.jieba_dict.clone(),
                },
                types::FormatType::None => types::FormatOptions::None,
            },
            None => script.default_format_type(),
        };
        let mut mmes = std::collections::HashMap::new();
        for out_f in outfiles {
            let name = utils::files::relative_path(&out_dir, &out_f)
                .with_extension("")
                .to_string_lossy()
                .into_owned();
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
            if mes.is_empty() {
                eprintln!("No messages found in {}", out_f);
                continue;
            }
            match name_csv {
                Some(name_table) => {
                    utils::name_replacement::replace_message(&mut mes, name_table);
                }
                None => {}
            }
            format::fmt_message(&mut mes, fmt.clone(), *builder.script_type())?;
            mmes.insert(name, mes);
        }
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
        if let Some(dep_graph) = dep_graph.as_mut() {
            dep_graph.0 = patched_f.clone();
        }
        utils::files::make_sure_dir_exists(&patched_f)?;
        let encoding = get_patched_encoding(imp_cfg, builder);
        script.import_multiple_messages_filename(mmes, &patched_f, encoding, repl)?;
        return Ok(types::ScriptResult::Ok);
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
        pb.set_extension(if of.is_custom() {
            script.custom_output_extension()
        } else {
            of.as_ref()
        });
        pb.to_string_lossy().into_owned()
    } else {
        imp_cfg.output.clone()
    };
    if !std::fs::exists(&out_f).unwrap_or(false) {
        eprintln!("Output file does not exist");
        return Ok(types::ScriptResult::Ignored);
    }
    if let Some(dep_graph) = dep_graph.as_mut() {
        dep_graph.1.push(out_f.clone());
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
    if let Some(dep_graph) = dep_graph.as_mut() {
        dep_graph.0 = patched_f.clone();
    }
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
                break_with_sentence: imp_cfg.patched_break_with_sentence,
                #[cfg(feature = "jieba")]
                break_chinese_words: !imp_cfg.patched_no_break_chinese_words,
                #[cfg(feature = "jieba")]
                jieba_dict: arg.jieba_dict.clone(),
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
    format::fmt_message(&mut mes, fmt, *builder.script_type())?;

    script.import_messages_filename(mes, &patched_f, encoding, repl)?;
    Ok(types::ScriptResult::Ok)
}

pub fn pack_archive(
    input: &str,
    output: Option<&str>,
    arg: &args::Arg,
    config: std::sync::Arc<types::ExtraConfig>,
    backslash: bool,
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
                    p.to_str().map(|s| {
                        if backslash {
                            s.replace("/", "\\").trim_start_matches("\\").to_owned()
                        } else {
                            s.replace("\\", "/").trim_start_matches("/").to_owned()
                        }
                    })
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
        &config,
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
        let size = match std::fs::metadata(file) {
            Ok(meta) => meta.len(),
            Err(e) => {
                eprintln!("Error getting metadata for file {}: {}", file, e);
                COUNTER.inc_error();
                continue;
            }
        };
        let mut wf = match archive.new_file_non_seek(name, Some(size)) {
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

pub fn pack_archive_v2(
    input: &[&str],
    output: Option<&str>,
    arg: &args::Arg,
    config: std::sync::Arc<types::ExtraConfig>,
    backslash: bool,
    no_dir: bool,
    dep_file: Option<&str>,
) -> anyhow::Result<()> {
    let typ = match &arg.script_type {
        Some(t) => t,
        None => {
            return Err(anyhow::anyhow!("No script type specified"));
        }
    };
    // File List in real path
    let mut files = Vec::new();
    // File list in archive path
    let mut re_files = Vec::new();
    for i in input {
        let (fs, is_dir) = utils::files::collect_files(i, arg.recursive, true)?;
        if is_dir {
            files.extend_from_slice(&fs);
            for n in fs.iter() {
                if no_dir {
                    if let Some(p) = std::path::PathBuf::from(n).file_name() {
                        re_files.push(p.to_string_lossy().into_owned());
                    } else {
                        return Err(anyhow::anyhow!("Failed to get filename from {}", n));
                    }
                } else {
                    if let Some(p) = {
                        std::path::PathBuf::from(n)
                            .strip_prefix(i)
                            .ok()
                            .and_then(|p| {
                                p.to_str().map(|s| {
                                    if backslash {
                                        s.replace("/", "\\").trim_start_matches("\\").to_owned()
                                    } else {
                                        s.replace("\\", "/").trim_start_matches("/").to_owned()
                                    }
                                })
                            })
                    } {
                        re_files.push(p);
                    } else {
                        return Err(anyhow::anyhow!("Failed to get relative path from {}", n));
                    }
                }
            }
        } else {
            files.push(i.to_string());
            let p = std::path::PathBuf::from(i);
            if let Some(fname) = p.file_name() {
                re_files.push(fname.to_string_lossy().into_owned());
            } else {
                return Err(anyhow::anyhow!("Failed to get filename from {}", i));
            }
        }
    }
    let reff = re_files.iter().map(|s| s.as_str()).collect::<Vec<_>>();
    let builder = scripts::BUILDER
        .iter()
        .find(|b| b.script_type() == typ)
        .ok_or_else(|| anyhow::anyhow!("Unsupported script type"))?;
    let output = match output {
        Some(output) => output.to_string(),
        None => {
            let mut pb = std::path::PathBuf::from(input[0]);
            let ext = builder.extensions().first().unwrap_or(&"unk");
            pb.set_extension(ext);
            if pb.to_string_lossy() == input[0] {
                pb.set_extension(format!("{}.{}", ext, ext));
            }
            pb.to_string_lossy().into_owned()
        }
    };
    let mut archive = builder.create_archive(
        &output,
        &reff,
        get_archived_encoding(arg, builder, get_encoding(arg, builder)),
        &config,
    )?;
    if let Some(dep_file) = dep_file {
        let df = std::fs::File::create(dep_file)
            .map_err(|e| anyhow::anyhow!("Failed to create dep file {}: {}", dep_file, e))?;
        let mut df = std::io::BufWriter::new(df);
        use std::io::Write;
        write!(df, "{}:", escape_dep_string(&output))
            .map_err(|e| anyhow::anyhow!("Failed to write to dep file {}: {}", dep_file, e))?;
        for f in &files {
            write!(df, " {}", escape_dep_string(f))
                .map_err(|e| anyhow::anyhow!("Failed to write to dep file {}: {}", dep_file, e))?;
        }
        writeln!(df)
            .map_err(|e| anyhow::anyhow!("Failed to write to dep file {}: {}", dep_file, e))?;
    }
    for (file, name) in files.iter().zip(reff) {
        let mut f = match std::fs::File::open(file) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Error opening file {}: {}", file, e);
                COUNTER.inc_error();
                continue;
            }
        };
        let size = match std::fs::metadata(file) {
            Ok(meta) => meta.len(),
            Err(e) => {
                eprintln!("Error getting metadata for file {}: {}", file, e);
                COUNTER.inc_error();
                continue;
            }
        };
        let mut wf = match archive.new_file_non_seek(name, Some(size)) {
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
    config: std::sync::Arc<types::ExtraConfig>,
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
    config: std::sync::Arc<types::ExtraConfig>,
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
        builder.create_image_file_filename(data, &output, &config)?;
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

    crate::utils::files::make_sure_dir_exists(&output)?;

    builder.create_file_filename(
        input,
        &output,
        get_encoding(arg, builder),
        get_output_encoding(arg),
        &config,
    )?;
    Ok(())
}

pub fn parse_output_script_as_extend(
    input: &str,
    typ: types::OutputScriptType,
    arg: &args::Arg,
) -> anyhow::Result<Vec<types::ExtendedMessage>> {
    match typ {
        types::OutputScriptType::M3t
        | types::OutputScriptType::M3ta
        | types::OutputScriptType::M3tTxt => {
            let enc = get_input_output_script_encoding(arg);
            let b = utils::files::read_file(input)?;
            let s = utils::encoding::decode_to_string(enc, &b, true)?;
            let mut parser = output_scripts::m3t::M3tParser::new(
                &s,
                arg.llm_trans_mark.as_ref().map(|s| s.as_str()),
            );
            let mes = parser.parse_as_extend()?;
            Ok(mes)
        }
        types::OutputScriptType::Po | types::OutputScriptType::Pot => {
            let enc = get_input_output_script_encoding(arg);
            let b = utils::files::read_file(input)?;
            let s = utils::encoding::decode_to_string(enc, &b, true)?;
            let mut parser = output_scripts::po::PoParser::new(
                &s,
                arg.llm_trans_mark.as_ref().map(|s| s.as_str()),
            );
            let mes = parser.parse_as_extend()?;
            Ok(mes)
        }
        _ => Err(anyhow::anyhow!(
            "Output script type {:?} does not support extended messages",
            typ
        )),
    }
}

pub fn parse_output_script(
    input: &str,
    typ: types::OutputScriptType,
    arg: &args::Arg,
) -> anyhow::Result<Vec<types::Message>> {
    match typ {
        types::OutputScriptType::M3t
        | types::OutputScriptType::M3ta
        | types::OutputScriptType::M3tTxt => {
            let enc = get_input_output_script_encoding(arg);
            let b = utils::files::read_file(input)?;
            let s = utils::encoding::decode_to_string(enc, &b, true)?;
            let mut parser = output_scripts::m3t::M3tParser::new(
                &s,
                arg.llm_trans_mark.as_ref().map(|s| s.as_str()),
            );
            let mes = parser.parse()?;
            Ok(mes)
        }
        types::OutputScriptType::Po | types::OutputScriptType::Pot => {
            let enc = get_input_output_script_encoding(arg);
            let b = utils::files::read_file(input)?;
            let s = utils::encoding::decode_to_string(enc, &b, true)?;
            let mut parser = output_scripts::po::PoParser::new(
                &s,
                arg.llm_trans_mark.as_ref().map(|s| s.as_str()),
            );
            let mes = parser.parse()?;
            Ok(mes)
        }
        types::OutputScriptType::Json => {
            let enc = get_input_output_script_encoding(arg);
            let b = utils::files::read_file(input)?;
            let s = utils::encoding::decode_to_string(enc, &b, true)?;
            let mes = serde_json::from_str::<Vec<types::Message>>(&s)?;
            Ok(mes)
        }
        types::OutputScriptType::Yaml => {
            let enc = get_input_output_script_encoding(arg);
            let b = utils::files::read_file(input)?;
            let s = utils::encoding::decode_to_string(enc, &b, true)?;
            let mes = serde_yaml_ng::from_str::<Vec<types::Message>>(&s)?;
            Ok(mes)
        }
        _ => Err(anyhow::anyhow!(
            "Output script type {:?} does not support message parsing",
            typ
        )),
    }
}

pub fn dump_output_script_as_extend(
    output: &str,
    typ: types::OutputScriptType,
    mes: &[types::ExtendedMessage],
    arg: &args::Arg,
) -> anyhow::Result<()> {
    match typ {
        types::OutputScriptType::M3t
        | types::OutputScriptType::M3ta
        | types::OutputScriptType::M3tTxt => {
            let enc = get_output_encoding(arg);
            let s = output_scripts::m3t::M3tDumper::dump_extended(mes);
            let b = utils::encoding::encode_string(enc, &s, false)?;
            utils::files::write_file(output)?.write_all(&b)?;
            Ok(())
        }
        types::OutputScriptType::Po | types::OutputScriptType::Pot => {
            let enc = get_output_encoding(arg);
            let s = output_scripts::po::PoDumper::new();
            let s = s.dump_extended(mes, enc)?;
            let b = utils::encoding::encode_string(enc, &s, false)?;
            utils::files::write_file(output)?.write_all(&b)?;
            Ok(())
        }
        _ => Err(anyhow::anyhow!(
            "Output script type {:?} does not support extended messages",
            typ
        )),
    }
}

pub fn dump_output_script(
    output: &str,
    typ: types::OutputScriptType,
    mes: &[types::Message],
    arg: &args::Arg,
) -> anyhow::Result<()> {
    match typ {
        types::OutputScriptType::M3t
        | types::OutputScriptType::M3ta
        | types::OutputScriptType::M3tTxt => {
            let enc = get_output_encoding(arg);
            let s = output_scripts::m3t::M3tDumper::dump(mes, arg.m3t_no_quote);
            let b = utils::encoding::encode_string(enc, &s, false)?;
            utils::files::write_file(output)?.write_all(&b)?;
            Ok(())
        }
        types::OutputScriptType::Po | types::OutputScriptType::Pot => {
            let enc = get_output_encoding(arg);
            let s = output_scripts::po::PoDumper::new();
            let s = s.dump(mes, enc)?;
            let b = utils::encoding::encode_string(enc, &s, false)?;
            utils::files::write_file(output)?.write_all(&b)?;
            Ok(())
        }
        types::OutputScriptType::Json => {
            let enc = get_output_encoding(arg);
            let s = serde_json::to_string_pretty(mes)?;
            let b = utils::encoding::encode_string(enc, &s, false)?;
            utils::files::write_file(output)?.write_all(&b)?;
            Ok(())
        }
        types::OutputScriptType::Yaml => {
            let enc = get_output_encoding(arg);
            let s = serde_yaml_ng::to_string(mes)?;
            let b = utils::encoding::encode_string(enc, &s, false)?;
            utils::files::write_file(output)?.write_all(&b)?;
            Ok(())
        }
        _ => Err(anyhow::anyhow!(
            "Output script type {:?} does not support message dumping",
            typ
        )),
    }
}

pub fn convert_file(
    input: &str,
    input_type: types::OutputScriptType,
    output: Option<&str>,
    output_type: types::OutputScriptType,
    arg: &args::Arg,
    root_dir: Option<&std::path::Path>,
) -> anyhow::Result<types::ScriptResult> {
    let input_support_src = input_type.is_src_supported();
    let output_support_src = output_type.is_src_supported();
    let output = match output {
        Some(output) => match root_dir {
            Some(root_dir) => {
                let f = std::path::PathBuf::from(input);
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
                pb.set_extension(output_type.as_ref());
                pb.to_string_lossy().into_owned()
            }
            None => output.to_string(),
        },
        None => {
            let mut pb = std::path::PathBuf::from(input);
            if arg.output_no_extra_ext {
                pb.remove_all_extensions();
            }
            pb.set_extension(output_type.as_ref());
            pb.to_string_lossy().into_owned()
        }
    };
    if input_support_src && output_support_src {
        let input_mes = parse_output_script_as_extend(input, input_type, arg)?;
        dump_output_script_as_extend(&output, output_type, &input_mes, arg)?;
        return Ok(types::ScriptResult::Ok);
    }
    let input_mes = parse_output_script(input, input_type, arg)?;
    dump_output_script(&output, output_type, &input_mes, arg)?;
    Ok(types::ScriptResult::Ok)
}

lazy_static::lazy_static! {
    static ref COUNTER: utils::counter::Counter = utils::counter::Counter::new();
    static ref EXIT_LISTENER: std::sync::Mutex<std::collections::BTreeMap<usize, Box<dyn Fn() + Send + Sync>>> = std::sync::Mutex::new(std::collections::BTreeMap::new());
    #[allow(unused)]
    static ref EXIT_LISTENER_ID: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
}

#[allow(dead_code)]
fn add_exit_listener<F: Fn() + Send + Sync + 'static>(f: F) -> usize {
    let id = EXIT_LISTENER_ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    EXIT_LISTENER
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .insert(id, Box::new(f));
    id
}

#[allow(dead_code)]
fn remove_exit_listener(id: usize) {
    EXIT_LISTENER
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .remove(&id);
}

fn main() {
    let _ = ctrlc::try_set_handler(|| {
        let listeners = EXIT_LISTENER.lock().unwrap_or_else(|err| err.into_inner());
        for (_, f) in listeners.iter() {
            f();
        }
        eprintln!("Aborted.");
        eprintln!("{}", std::ops::Deref::deref(&COUNTER));
        std::process::exit(1);
    });
    let arg = args::parse_args();
    let argn = std::sync::Arc::new(arg.clone());
    if arg.backtrace {
        unsafe { std::env::set_var("RUST_LIB_BACKTRACE", "1") };
    }
    let cfg = std::sync::Arc::new(types::ExtraConfig {
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
        #[cfg(feature = "bgi-img")]
        bgi_img_workers: arg.bgi_img_workers,
        #[cfg(feature = "image-jxl")]
        jxl_lossless: !arg.jxl_lossy,
        #[cfg(feature = "image-jxl")]
        jxl_distance: arg.jxl_distance,
        #[cfg(feature = "image-jxl")]
        jxl_workers: arg.jxl_workers,
        #[cfg(feature = "emote-img")]
        psb_process_tlg: !arg.psb_no_process_tlg,
        #[cfg(feature = "softpal-img")]
        pgd_fake_compress: !arg.pgd_compress,
        #[cfg(feature = "softpal")]
        softpal_add_message_index: arg.softpal_add_message_index,
        #[cfg(feature = "kirikiri")]
        kirikiri_chat_multilang: !arg.kirikiri_chat_no_multilang,
        #[cfg(feature = "kirikiri-arc")]
        xp3_simple_crypt: !arg.xp3_no_simple_crypt,
        #[cfg(feature = "kirikiri-arc")]
        xp3_mdf_decompress: !arg.xp3_no_mdf_decompress,
        #[cfg(feature = "kirikiri-arc")]
        xp3_segmenter: arg.xp3_segmenter,
        #[cfg(feature = "kirikiri-arc")]
        xp3_compress_files: !arg.xp3_no_compress_files,
        #[cfg(feature = "kirikiri-arc")]
        xp3_compress_index: !arg.xp3_no_compress_index,
        #[cfg(feature = "kirikiri-arc")]
        xp3_compress_workers: arg.xp3_compress_workers,
        #[cfg(feature = "kirikiri-arc")]
        xp3_zstd: arg.xp3_zstd,
        #[cfg(feature = "kirikiri-arc")]
        xp3_pack_workers: arg.xp3_pack_workers,
        #[cfg(feature = "kirikiri")]
        kirikiri_language_insert: arg.kirikiri_language_insert,
        #[cfg(feature = "musica-arc")]
        musica_game_title: arg.musica_game_title.clone(),
        #[cfg(feature = "musica-arc")]
        musica_xor_key: arg.musica_xor_key,
        #[cfg(feature = "musica-arc")]
        musica_compress: arg.musica_compress,
        #[cfg(feature = "kirikiri-arc")]
        xp3_no_adler: arg.xp3_no_adler,
        #[cfg(feature = "bgi")]
        bgi_add_space: arg.bgi_add_space,
        #[cfg(feature = "escude")]
        escude_op: arg.escude_op,
    });
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
                                std::process::exit(
                                    argn.exit_code_all_failed.unwrap_or(argn.exit_code),
                                );
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
            #[cfg(feature = "image")]
            let img_threadpool = if arg.image_workers > 1 {
                let tp = std::sync::Arc::new(
                    utils::threadpool::ThreadPool::<Result<(), anyhow::Error>>::new(
                        arg.image_workers,
                        Some("img-output-worker-"),
                        false,
                    )
                    .expect("Failed to create image thread pool"),
                );
                let tp2 = tp.clone();
                let id = add_exit_listener(move || {
                    for r in tp2.take_results() {
                        if let Err(e) = r {
                            eprintln!("{}", e);
                            COUNTER.inc_error();
                        } else {
                            COUNTER.inc(types::ScriptResult::Ok);
                        }
                    }
                });
                Some((tp, id))
            } else {
                None
            };
            for script in scripts.iter() {
                #[cfg(feature = "image")]
                let re = export_script(
                    &script,
                    &arg,
                    cfg.clone(),
                    output,
                    root_dir,
                    img_threadpool.as_ref().map(|(t, _)| &**t),
                );
                #[cfg(not(feature = "image"))]
                let re = export_script(&script, &arg, cfg.clone(), output, root_dir);
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
                #[cfg(feature = "image")]
                img_threadpool.as_ref().map(|(t, _)| {
                    for r in t.take_results() {
                        if let Err(e) = r {
                            COUNTER.inc_error();
                            eprintln!("{}", e);
                        } else {
                            COUNTER.inc(types::ScriptResult::Ok);
                        }
                    }
                });
            }
            #[cfg(feature = "image")]
            img_threadpool.map(|(t, id)| {
                t.join();
                remove_exit_listener(id);
                for r in t.take_results() {
                    if let Err(e) = r {
                        COUNTER.inc_error();
                        eprintln!("{}", e);
                    } else {
                        COUNTER.inc(types::ScriptResult::Ok);
                    }
                }
            });
        }
        args::Command::Import(args) => {
            let name_csv = match &args.name_csv {
                Some(name_csv) => {
                    let name_table = utils::name_replacement::read_csv(name_csv).unwrap();
                    Some(name_table)
                }
                None => None,
            };
            let repl = std::sync::Arc::new(match &args.replacement_json {
                Some(replacement_json) => {
                    let b = utils::files::read_file(replacement_json).unwrap();
                    let s = String::from_utf8(b).unwrap();
                    let table = serde_json::from_str::<types::ReplacementTable>(&s).unwrap();
                    Some(table)
                }
                None => None,
            });
            let (scripts, is_dir) =
                utils::files::collect_files(&args.input, arg.recursive, false).unwrap();
            if is_dir {
                let pb = std::path::Path::new(&args.patched);
                if pb.exists() {
                    if !pb.is_dir() {
                        eprintln!("Patched path is not a directory");
                        std::process::exit(argn.exit_code_all_failed.unwrap_or(argn.exit_code));
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
            let workers = if args.jobs > 1 {
                Some(
                    utils::threadpool::ThreadPool::<()>::new(
                        args.jobs,
                        Some("import-worker-"),
                        true,
                    )
                    .unwrap(),
                )
            } else {
                None
            };
            let dep_files = if args.dep_file.is_some() {
                Some(std::sync::Arc::new(std::sync::Mutex::new(
                    std::collections::HashMap::new(),
                )))
            } else {
                None
            };
            for script in scripts.iter() {
                if let Some(workers) = workers.as_ref() {
                    let arg = argn.clone();
                    let cfg = cfg.clone();
                    let script = script.clone();
                    let name_csv = name_csv.as_ref().map(|s| s.clone());
                    let repl = repl.clone();
                    let root_dir = root_dir.map(|s| s.to_path_buf());
                    let args = args.clone();
                    let dep_files = dep_files.clone();
                    if let Err(e) = workers.execute(
                        move |_| {
                            let mut dep_graph = if dep_files.is_some() {
                                Some((String::new(), Vec::new()))
                            } else {
                                None
                            };
                            let re = import_script(
                                &script,
                                &arg,
                                cfg,
                                &args,
                                root_dir.as_ref().map(|s| s.as_path()),
                                name_csv.as_ref(),
                                (*repl).as_ref(),
                                dep_graph.as_mut(),
                            );
                            match re {
                                Ok(s) => {
                                    COUNTER.inc(s);
                                    if let Some((fname, deps)) = dep_graph {
                                        if let Some(dep_files) = dep_files {
                                            let mut lock =
                                                crate::ext::mutex::MutexExt::lock_blocking(
                                                    dep_files.as_ref(),
                                                );
                                            lock.insert(fname, deps);
                                        }
                                    }
                                }
                                Err(e) => {
                                    COUNTER.inc_error();
                                    eprintln!("Error exporting {}: {}", script, e);
                                    if arg.backtrace {
                                        eprintln!("Backtrace: {}", e.backtrace());
                                    }
                                }
                            }
                        },
                        true,
                    ) {
                        COUNTER.inc_error();
                        eprintln!("Error executing import worker: {}", e);
                    }
                } else {
                    let mut dep_graph = if dep_files.is_some() {
                        Some((String::new(), Vec::new()))
                    } else {
                        None
                    };
                    let re = import_script(
                        &script,
                        &arg,
                        cfg.clone(),
                        args,
                        root_dir,
                        name_csv.as_ref(),
                        (*repl).as_ref(),
                        dep_graph.as_mut(),
                    );
                    match re {
                        Ok(s) => {
                            COUNTER.inc(s);
                            if let Some((fname, deps)) = dep_graph {
                                if let Some(dep_files) = dep_files.as_ref() {
                                    let mut lock = crate::ext::mutex::MutexExt::lock_blocking(
                                        dep_files.as_ref(),
                                    );
                                    lock.insert(fname, deps);
                                }
                            }
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
            if let Some(map) = dep_files {
                let lock = crate::ext::mutex::MutexExt::lock_blocking(map.as_ref());
                if let Some(dep_file) = &args.dep_file {
                    let df = std::fs::File::create(dep_file).unwrap();
                    let mut df = std::io::BufWriter::new(df);
                    use std::io::Write;
                    for (fname, deps) in lock.iter() {
                        write!(df, "{}:", escape_dep_string(fname)).unwrap();
                        for d in deps {
                            write!(df, " {}", escape_dep_string(d)).unwrap();
                        }
                        writeln!(df).unwrap();
                    }
                }
            }
        }
        args::Command::Pack {
            input,
            output,
            backslash,
        } => {
            let re = pack_archive(
                input,
                output.as_ref().map(|s| s.as_str()),
                &arg,
                cfg.clone(),
                *backslash,
            );
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
                                std::process::exit(
                                    argn.exit_code_all_failed.unwrap_or(argn.exit_code),
                                );
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
                let re = unpack_archive(&script, &arg, cfg.clone(), output, root_dir);
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
            let re = create_file(
                input,
                output.as_ref().map(|s| s.as_str()),
                &arg,
                cfg.clone(),
            );
            if let Err(e) = re {
                COUNTER.inc_error();
                eprintln!("Error creating file: {}", e);
                if arg.backtrace {
                    eprintln!("Backtrace: {}", e.backtrace());
                }
            }
        }
        args::Command::PackV2 {
            output,
            input,
            backslash,
            no_dir,
            dep_file,
        } => {
            if !input.is_empty() {
                let input = input.iter().map(|s| s.as_str()).collect::<Vec<_>>();
                let re = pack_archive_v2(
                    &input,
                    output.as_ref().map(|s| s.as_str()),
                    &arg,
                    cfg.clone(),
                    *backslash,
                    *no_dir,
                    dep_file.as_ref().map(|s| s.as_str()),
                );
                if let Err(e) = re {
                    COUNTER.inc_error();
                    eprintln!("Error packing archive: {}", e);
                }
            } else {
                eprintln!("No input files specified for packing.");
            }
        }
        args::Command::Convert {
            input_type,
            output_type,
            input,
            output,
        } => {
            if input_type.is_custom() {
                eprintln!("Custom input type is not supported for conversion.");
                std::process::exit(argn.exit_code_all_failed.unwrap_or(argn.exit_code));
            }
            if output_type.is_custom() {
                eprintln!("Custom output type is not supported for conversion.");
                std::process::exit(argn.exit_code_all_failed.unwrap_or(argn.exit_code));
            }
            let (scripts, is_dir) =
                utils::files::collect_ext_files(input, arg.recursive, &[input_type.as_ref()])
                    .unwrap();
            if is_dir {
                match &output {
                    Some(output) => {
                        let op = std::path::Path::new(output);
                        if op.exists() {
                            if !op.is_dir() {
                                eprintln!("Output path is not a directory");
                                std::process::exit(
                                    argn.exit_code_all_failed.unwrap_or(argn.exit_code),
                                );
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
                let re = convert_file(
                    &script,
                    *input_type,
                    output.as_ref().map(|s| s.as_str()),
                    *output_type,
                    &arg,
                    root_dir,
                );
                match re {
                    Ok(s) => {
                        COUNTER.inc(s);
                    }
                    Err(e) => {
                        COUNTER.inc_error();
                        eprintln!("Error converting {}: {}", script, e);
                        if arg.backtrace {
                            eprintln!("Backtrace: {}", e.backtrace());
                        }
                    }
                }
            }
        }
    }
    let counter = std::ops::Deref::deref(&COUNTER);
    eprintln!("{}", counter);
    if counter.all_failed() {
        std::process::exit(argn.exit_code_all_failed.unwrap_or(argn.exit_code));
    } else if counter.has_error() {
        std::process::exit(argn.exit_code);
    }
}
