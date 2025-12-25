//! Basic Handle for all emote PSB files.
use super::rle::*;
use crate::ext::io::*;
use crate::ext::psb::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::encoding::*;
use crate::utils::files::*;
use crate::utils::img::*;
use anyhow::Result;
use base64::Engine;
use emote_psb::*;
use libtlg_rs::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Read, Seek, Write};

#[derive(Debug)]
pub struct PsbBuilder {}

impl PsbBuilder {
    pub fn new() -> Self {
        Self {}
    }
}

impl ScriptBuilder for PsbBuilder {
    fn default_encoding(&self) -> Encoding {
        Encoding::Utf8
    }

    fn build_script(
        &self,
        buf: Vec<u8>,
        _filename: &str,
        encoding: Encoding,
        _archive_encoding: Encoding,
        config: &ExtraConfig,
        _archive: Option<&Box<dyn Script>>,
    ) -> Result<Box<dyn Script>> {
        Ok(Box::new(Psb::new(MemReader::new(buf), encoding, config)?))
    }

    fn build_script_from_reader(
        &self,
        reader: Box<dyn ReadSeek>,
        _filename: &str,
        encoding: Encoding,
        _archive_encoding: Encoding,
        config: &ExtraConfig,
        _archive: Option<&Box<dyn Script>>,
    ) -> Result<Box<dyn Script>> {
        Ok(Box::new(Psb::new(reader, encoding, config)?))
    }

    fn build_script_from_file(
        &self,
        filename: &str,
        encoding: Encoding,
        _archive_encoding: Encoding,
        config: &ExtraConfig,
        _archive: Option<&Box<dyn Script>>,
    ) -> Result<Box<dyn Script>> {
        let file = std::fs::File::open(filename)?;
        let f = std::io::BufReader::new(file);
        Ok(Box::new(Psb::new(f, encoding, config)?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["psb"]
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::EmotePsb
    }

    fn is_this_format(&self, _filename: &str, buf: &[u8], buf_len: usize) -> Option<u8> {
        if buf_len >= 4 && buf.starts_with(b"PSB\0") {
            return Some(10);
        } else if buf_len >= 4 && buf.starts_with(&[0x04, 0x22, 0x4D, 0x18]) {
            for i in 4..buf_len - 4 {
                if buf[i..i + 4] == *b"PSB\0" {
                    return Some(10);
                }
            }
        }
        None
    }

    fn can_create_file(&self) -> bool {
        true
    }

    fn create_file<'a>(
        &'a self,
        filename: &'a str,
        writer: Box<dyn WriteSeek + 'a>,
        encoding: Encoding,
        file_encoding: Encoding,
        _config: &ExtraConfig,
    ) -> Result<()> {
        create_file(filename, writer, encoding, file_encoding)
    }
}

#[derive(Debug)]
pub struct Psb {
    psb: VirtualPsbFixed,
    encoding: Encoding,
    config: ExtraConfig,
}

impl Psb {
    pub fn new<R: Read + Seek>(
        reader: R,
        encoding: Encoding,
        config: &ExtraConfig,
    ) -> Result<Self> {
        let psb = PsbReader::open_psb_v2(reader)?.to_psb_fixed();
        Ok(Self {
            psb,
            encoding,
            config: config.clone(),
        })
    }

    fn output_resource(
        &self,
        folder_path: &std::path::PathBuf,
        path: String,
        data: &[u8],
    ) -> Result<Resource> {
        let mut res = Resource {
            path,
            tlg: None,
            rle: None,
        };
        if self.config.psb_process_tlg && is_valid_tlg(&data) {
            let tlg = load_tlg(MemReaderRef::new(&data))?;
            res.tlg = Some(TlgInfo::from_tlg(&tlg, self.encoding));
            let outtype = self.config.image_type.unwrap_or(ImageOutputType::Png);
            res.path = {
                let mut pb = std::path::PathBuf::from(&res.path);
                pb.set_extension(outtype.as_ref());
                pb.to_string_lossy().to_string()
            };
            let path = folder_path.join(&res.path);
            make_sure_dir_exists(&path)?;
            let img = ImageData {
                width: tlg.width as u32,
                height: tlg.height as u32,
                color_type: match tlg.color {
                    TlgColorType::Bgr24 => ImageColorType::Bgr,
                    TlgColorType::Bgra32 => ImageColorType::Bgra,
                    TlgColorType::Grayscale8 => ImageColorType::Grayscale,
                },
                depth: 8,
                data: tlg.data,
            };
            encode_img(img, outtype, &path.to_string_lossy(), &self.config)?;
        } else {
            let path = folder_path.join(&res.path);
            make_sure_dir_exists(&path)?;
            std::fs::write(&path, data)?;
        }
        Ok(res)
    }

    fn output_rle_resource(
        &self,
        folder_path: &std::path::PathBuf,
        path: String,
        data: &[u8],
        width: i64,
        height: i64,
    ) -> Result<Resource> {
        let mut res = Resource {
            path,
            tlg: None,
            rle: Some(RLPixelInfo { width, height }),
        };
        let decompressed = rl_decompress(MemReaderRef::new(data), 4, None)?;
        let outtype = self.config.image_type.unwrap_or(ImageOutputType::Png);
        res.path = {
            let mut pb = std::path::PathBuf::from(&res.path);
            pb.set_extension(outtype.as_ref());
            pb.to_string_lossy().to_string()
        };
        let path = folder_path.join(&res.path);
        make_sure_dir_exists(&path)?;
        let img = ImageData {
            width: width as u32,
            height: height as u32,
            color_type: ImageColorType::Bgra,
            depth: 8,
            data: decompressed,
        };
        encode_img(img, outtype, &path.to_string_lossy(), &self.config)?;
        Ok(res)
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct TlgInfo {
    metadata: HashMap<String, String>,
}

impl TlgInfo {
    fn from_tlg(tlg: &Tlg, encoding: Encoding) -> Self {
        let mut metadata = HashMap::new();
        for (k, v) in &tlg.tags {
            let k = if let Ok(s) = decode_to_string(encoding, &k, true) {
                s
            } else {
                format!(
                    "base64:{}",
                    base64::engine::general_purpose::STANDARD.encode(k)
                )
            };
            let v = if let Ok(s) = decode_to_string(encoding, &v, true) {
                s
            } else {
                format!(
                    "base64:{}",
                    base64::engine::general_purpose::STANDARD.encode(v)
                )
            };
            metadata.insert(k, v);
        }
        Self { metadata }
    }

    fn to_tlg_tags(&self, encoding: Encoding) -> Result<HashMap<Vec<u8>, Vec<u8>>> {
        let mut tags = HashMap::new();
        for (k, v) in &self.metadata {
            let k = if k.starts_with("base64:") {
                base64::engine::general_purpose::STANDARD.decode(&k[7..])?
            } else {
                encode_string(encoding, k, false)?
            };
            let v = if v.starts_with("base64:") {
                base64::engine::general_purpose::STANDARD.decode(&v[7..])?
            } else {
                encode_string(encoding, v, false)?
            };
            tags.insert(k, v);
        }
        Ok(tags)
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct RLPixelInfo {
    width: i64,
    height: i64,
}

#[derive(Debug, Deserialize, Serialize)]
struct Resource {
    path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    tlg: Option<TlgInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rle: Option<RLPixelInfo>,
}

impl Script for Psb {
    fn default_output_script_type(&self) -> OutputScriptType {
        OutputScriptType::Custom
    }

    fn is_output_supported(&self, output: OutputScriptType) -> bool {
        matches!(output, OutputScriptType::Custom)
    }

    fn default_format_type(&self) -> FormatOptions {
        FormatOptions::None
    }

    fn custom_output_extension<'a>(&'a self) -> &'a str {
        "json"
    }

    fn custom_export(&self, filename: &std::path::Path, encoding: Encoding) -> Result<()> {
        let mut data = self.psb.to_json();
        let mut resources = Vec::new();
        let mut extra_resources = Vec::new();
        let folder_path = {
            let mut pb = filename.to_path_buf();
            pb.set_extension("");
            pb
        };
        for (i, data) in self.psb.resources().iter().enumerate() {
            let i = i as u64;
            let res_path = self.psb.root().find_resource_key(i, vec![]);
            if let Some(path) = &res_path {
                if path.len() >= 2 && *path.last().unwrap() == "pixel" {
                    let pb_data = self.psb.root();
                    let mut pb_data = &pb_data[*path.first().unwrap()];
                    for p in path.iter().take(path.len() - 1).skip(1) {
                        pb_data = &pb_data[*p];
                    }
                    let width = pb_data["width"].as_i64();
                    let height = pb_data["height"].as_i64();
                    let compress = pb_data["compress"].as_str();
                    if compress.is_some_and(|s| s == "RL") && (width.is_none() || height.is_none())
                    {
                        eprintln!(
                            "Warning: Resource {:?} is marked as RL compressed but width/height is missing (width={:?}, height={:?})",
                            path, pb_data["width"], pb_data["height"]
                        );
                        crate::COUNTER.inc_warning();
                    }
                    if let (Some(w), Some(h), Some(c)) = (width, height, compress) {
                        if c == "RL" {
                            let res_name: Vec<_> = path
                                .iter()
                                .take(path.len() - 1)
                                .map(|s| s.to_string())
                                .collect();
                            let res_name = res_name.join("/");
                            let res_name = sanitize_path(&res_name);
                            let res =
                                self.output_rle_resource(&folder_path, res_name, data, w, h)?;
                            resources.push(res);
                            continue;
                        }
                    }
                }
            }
            let res_name = res_path
                .map(|s| s.join("/"))
                .unwrap_or(format!("res_{}", i));
            let res_name = sanitize_path(&res_name);
            let res = self.output_resource(&folder_path, res_name, data)?;
            resources.push(res);
        }
        for (i, data) in self.psb.extra().iter().enumerate() {
            let i = i as u64;
            let res_name = self
                .psb
                .root()
                .find_resource_key(i, vec![])
                .map(|s| format!("extra_{}", s.join("/")))
                .unwrap_or(format!("extra_res_{}", i));
            let res_name = sanitize_path(&res_name);
            let res = self.output_resource(&folder_path, res_name, data)?;
            extra_resources.push(res);
        }
        data["resources"] = json::parse(&serde_json::to_string(&resources)?)?;
        data["extra_resources"] = json::parse(&serde_json::to_string(&extra_resources)?)?;
        let s = json::stringify_pretty(data, 2);
        let s = encode_string(encoding, &s, false)?;
        let mut file = std::fs::File::create(filename)?;
        file.write_all(&s)?;
        Ok(())
    }

    fn custom_import<'a>(
        &'a self,
        custom_filename: &'a str,
        file: Box<dyn WriteSeek + 'a>,
        encoding: Encoding,
        output_encoding: Encoding,
    ) -> Result<()> {
        create_file(custom_filename, file, encoding, output_encoding)
    }
}

fn read_resource(
    folder_path: &std::path::PathBuf,
    res: &Resource,
    encoding: Encoding,
) -> Result<Vec<u8>> {
    if let Some(tlg) = &res.tlg {
        let path = folder_path.join(&res.path);
        let imgfmt = ImageOutputType::try_from(path.as_path())?;
        let mut img = decode_img(imgfmt, &path.to_string_lossy())?;
        if img.depth != 8 {
            return Err(anyhow::anyhow!(
                "Only 8-bit images are supported for TLG conversion"
            ));
        }
        let color_type = match img.color_type {
            ImageColorType::Bgr => TlgColorType::Bgr24,
            ImageColorType::Bgra => TlgColorType::Bgra32,
            ImageColorType::Grayscale => TlgColorType::Grayscale8,
            ImageColorType::Rgb => {
                convert_rgb_to_bgr(&mut img)?;
                TlgColorType::Bgr24
            }
            ImageColorType::Rgba => {
                convert_rgba_to_bgra(&mut img)?;
                TlgColorType::Bgra32
            }
        };
        let tlg = Tlg {
            width: img.width,
            height: img.height,
            version: 5,
            color: color_type,
            data: img.data,
            tags: tlg.to_tlg_tags(encoding)?,
        };
        let mut writer = MemWriter::new();
        save_tlg(&tlg, &mut writer)?;
        Ok(writer.into_inner())
    } else if let Some(rle) = &res.rle {
        let path = folder_path.join(&res.path);
        let imgfmt = ImageOutputType::try_from(path.as_path())?;
        let mut img = decode_img(imgfmt, &path.to_string_lossy())?;
        if img.depth != 8 {
            return Err(anyhow::anyhow!(
                "Only 8-bit images are supported for RLE conversion"
            ));
        }
        if img.color_type == ImageColorType::Rgba {
            convert_rgba_to_bgra(&mut img)?;
        } else if img.color_type == ImageColorType::Rgb {
            convert_rgb_to_bgr(&mut img)?;
            convert_bgr_to_bgra(&mut img)?;
        } else if img.color_type == ImageColorType::Bgr {
            convert_bgr_to_bgra(&mut img)?;
        }
        if img.color_type != ImageColorType::Bgra {
            return Err(anyhow::anyhow!(
                "Only BGRA images are supported for RLE conversion"
            ));
        }
        if img.width as i64 != rle.width {
            eprintln!(
                "Warning: Image width {} does not match RLE width {}",
                img.width, rle.width
            );
        }
        if img.height as i64 != rle.height {
            eprintln!(
                "Warning: Image height {} does not match RLE height {}",
                img.height, rle.height
            );
        }
        let compressed = rl_compress(MemReaderRef::new(&img.data), 4)?;
        Ok(compressed)
    } else {
        let path = folder_path.join(&res.path);
        Ok(std::fs::read(&path)?)
    }
}

fn create_file<'a>(
    custom_filename: &'a str,
    mut writer: Box<dyn WriteSeek + 'a>,
    encoding: Encoding,
    output_encoding: Encoding,
) -> Result<()> {
    let input = read_file(custom_filename)?;
    let s = decode_to_string(output_encoding, &input, true)?;
    let data = json::parse(&s)?;
    let resources: Vec<Resource> = serde_json::from_str(&data["resources"].dump())?;
    let extra_resources: Vec<Resource> = serde_json::from_str(&data["extra_resources"].dump())?;
    let mut psb = VirtualPsbFixed::with_json(&data)?;
    let folder_path = {
        let mut pb = std::path::PathBuf::from(custom_filename);
        pb.set_extension("");
        pb
    };
    for res in resources {
        let res = read_resource(&folder_path, &res, encoding)?;
        psb.resources_mut().push(res);
    }
    for res in extra_resources {
        let res = read_resource(&folder_path, &res, encoding)?;
        psb.extra_mut().push(res);
    }
    let psb = psb.to_psb(false);
    let psb_writer = PsbWriter::new(psb, &mut writer);
    psb_writer
        .finish()
        .map_err(|e| anyhow::anyhow!("Failed to write psb: {:?}", e))?;
    Ok(())
}
