//! Emote DPAK-referenced Image File (.dref)
use crate::ext::io::*;
use crate::ext::psb::*;
use crate::scripts::base::*;
use crate::types::*;
use crate::utils::encoding::*;
use crate::utils::img::*;
use anyhow::Result;
use emote_psb::PsbReader;
use libtlg_rs::TlgColorType;
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use url::Url;

#[derive(Debug)]
/// Emote DREF Script Builder
pub struct DrefBuilder {}

impl DrefBuilder {
    /// Creates a new instance of `DrefBuilder`
    pub fn new() -> Self {
        Self {}
    }
}

impl ScriptBuilder for DrefBuilder {
    fn default_encoding(&self) -> Encoding {
        Encoding::Cp932
    }

    fn build_script(
        &self,
        buf: Vec<u8>,
        filename: &str,
        encoding: Encoding,
        _archive_encoding: Encoding,
        config: &ExtraConfig,
        archive: Option<&Box<dyn Script>>,
    ) -> Result<Box<dyn Script>> {
        Ok(Box::new(Dref::new(
            buf, encoding, filename, config, archive,
        )?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["dref"]
    }

    fn script_type(&self) -> &'static ScriptType {
        &ScriptType::EmoteDref
    }

    fn is_image(&self) -> bool {
        true
    }
}

struct Dpak {
    psb: VirtualPsbFixed,
}

struct OffsetData {
    left: u32,
    top: u32,
}

impl Dpak {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let f = std::fs::File::open(path)?;
        let mut f = std::io::BufReader::new(f);
        let mut psb = PsbReader::open_psb(&mut f)
            .map_err(|e| anyhow::anyhow!("Failed to read PSB from DPAK: {:?}", e))?;
        let psb = psb
            .load()
            .map_err(|e| anyhow::anyhow!("Failed to load PSB from DPAK: {:?}", e))?;
        let psb = psb.to_psb_fixed();
        Ok(Self { psb })
    }

    pub fn load_from_data(data: &[u8]) -> Result<Self> {
        let mut psb = PsbReader::open_psb(MemReaderRef::new(data))
            .map_err(|e| anyhow::anyhow!("Failed to read PSB from DPAK data: {:?}", e))?;
        let psb = psb
            .load()
            .map_err(|e| anyhow::anyhow!("Failed to load PSB from DPAK data: {:?}", e))?;
        let psb = psb.to_psb_fixed();
        Ok(Self { psb })
    }

    pub fn load_image(&self, name: &str) -> Result<(ImageData, Option<OffsetData>)> {
        let root = self.psb.root();
        let rid = root[name]
            .resource_id()
            .ok_or_else(|| anyhow::anyhow!("Resource ID for image '{}' not found in DPAK", name))?
            as usize;
        if rid >= self.psb.resources().len() {
            return Err(anyhow::anyhow!(
                "Resource ID {} out of bounds for DPAK with {} resources",
                rid,
                self.psb.resources().len()
            ));
        }
        let resource = &self.psb.resources()[rid];
        Self::load_img(&resource)
    }

    fn load_img(data: &[u8]) -> Result<(ImageData, Option<OffsetData>)> {
        if libtlg_rs::is_valid_tlg(data) {
            Ok((Self::load_tlg(data)?, None))
        } else {
            Self::load_png(data)
        }
    }

    fn load_tlg(data: &[u8]) -> Result<ImageData> {
        let img = libtlg_rs::load_tlg(MemReaderRef::new(data))
            .map_err(|e| anyhow::anyhow!("Failed to decode TLG image: {:?}", e))?;
        let color = img.color;
        let mut re = ImageData {
            width: img.width as u32,
            height: img.height as u32,
            color_type: match img.color {
                TlgColorType::Grayscale8 => ImageColorType::Grayscale,
                TlgColorType::Bgr24 => ImageColorType::Bgr,
                TlgColorType::Bgra32 => ImageColorType::Bgra,
            },
            data: img.data,
            depth: 8,
        };
        if let Some(v) = img.tags.get(&Vec::from(b"mode")) {
            if v == b"alpha" && color == TlgColorType::Bgr24 {
                convert_bgr_to_bgra(&mut re)?;
            }
        }
        Ok(re)
    }

    fn load_png(data: &[u8]) -> Result<(ImageData, Option<OffsetData>)> {
        let mut img = load_png(MemReaderRef::new(&data))?;
        match img.color_type {
            ImageColorType::Rgb => {
                convert_rgb_to_rgba(&mut img)?;
            }
            _ => {}
        }
        Ok((
            img,
            Self::try_read_offset_from_png(MemReaderRef::new(&data))?,
        ))
    }

    fn try_read_offset_from_png(mut data: MemReaderRef) -> Result<Option<OffsetData>> {
        data.pos = 8; // Skip PNG signature
        data.pos += 8; // Skip chunk size, type
        data.pos += 17; // Skip IHDR chunk (length + type + width + height + bit depth + color type + compression method + filter method + interlace method)
        loop {
            let chunk_size = data.read_u32_be()?;
            let mut chunk_type = [0u8; 4];
            data.read_exact(&mut chunk_type)?;
            if &chunk_type == b"IDAT" || &chunk_type == b"IEND" {
                break;
            }
            if &chunk_type == b"oFFs" {
                let x = data.read_u32_be()?;
                let y = data.read_u32_be()?;
                if data.read_u8()? == 0 {
                    return Ok(Some(OffsetData { left: x, top: y }));
                }
            }
            data.pos += chunk_size as usize + 4; // Skip chunk data and CRC
        }
        Ok(None)
    }
}

#[derive(Default)]
struct DpakLoader {
    map: HashMap<String, Dpak>,
}

impl DpakLoader {
    pub fn load_image(
        &mut self,
        dir: &Path,
        dpak: &str,
        filename: &str,
    ) -> Result<(ImageData, Option<OffsetData>)> {
        let dpak = match self.map.get(dpak) {
            Some(d) => d,
            None => {
                let path = dir.join(dpak);
                let ndpak = Dpak::new(&path)?;
                self.map.insert(dpak.to_string(), ndpak);
                self.map.get(dpak).unwrap()
            }
        };
        dpak.load_image(filename)
    }

    pub fn load_archives(&mut self, in_archives: &HashMap<String, Vec<u8>>) -> Result<()> {
        for (name, data) in in_archives.iter() {
            if !self.map.contains_key(name) {
                let dpak = Dpak::load_from_data(data)?;
                self.map.insert(name.clone(), dpak);
            }
        }
        Ok(())
    }
}

/// Emote DREF Script
pub struct Dref {
    urls: Vec<Url>,
    dir: PathBuf,
    in_archives: HashMap<String, Vec<u8>>,
}

impl std::fmt::Debug for Dref {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Dref")
            .field("urls", &self.urls)
            .field("dir", &self.dir)
            .finish()
    }
}

impl Dref {
    /// Create a new dref script
    ///
    /// * `buf` - The buffer containing the dref script data
    /// * `encoding` - The encoding of the script
    /// * `filename` - The name of the file
    /// * `config` - Extra configuration options
    /// * `archive` - Optional archive containing additional resources
    pub fn new(
        buf: Vec<u8>,
        encoding: Encoding,
        filename: &str,
        _config: &ExtraConfig,
        archive: Option<&Box<dyn Script>>,
    ) -> Result<Self> {
        let text = decode_with_bom_detect(encoding, &buf, true)?.0;
        let mut urls = Vec::new();
        for text in text.lines() {
            let text = text.trim();
            if text.is_empty() {
                continue;
            }
            urls.push(Url::parse(text)?);
        }
        let path = Path::new(filename);
        let dir = if let Some(parent) = path.parent() {
            parent.to_path_buf()
        } else {
            PathBuf::from(".")
        };
        if urls.is_empty() {
            return Err(anyhow::anyhow!("No URLs found in DREF file: {}", filename));
        }
        for u in urls.iter() {
            if u.scheme() != "psb" {
                return Err(anyhow::anyhow!(
                    "Invalid URL scheme in DREF file: {} (expected 'psb')",
                    u
                ));
            }
        }
        let mut in_archives = HashMap::new();
        if let Some(archive) = archive {
            if archive.is_archive() {
                for url in urls.iter() {
                    let filename = url.domain().ok_or(anyhow::anyhow!(
                        "Invalid URL in DREF file: {} (missing domain)",
                        url
                    ))?;
                    if let Ok(mut content) = archive.open_file_by_name(filename, true) {
                        in_archives.insert(filename.to_string(), content.data()?);
                    }
                }
            }
        }
        Ok(Self {
            urls,
            dir,
            in_archives,
        })
    }
}

impl Script for Dref {
    fn default_output_script_type(&self) -> OutputScriptType {
        OutputScriptType::Json
    }

    fn default_format_type(&self) -> FormatOptions {
        FormatOptions::None
    }

    fn is_image(&self) -> bool {
        true
    }

    fn export_image(&self) -> Result<ImageData> {
        let mut loader = DpakLoader::default();
        loader.load_archives(&self.in_archives)?;
        let base_url = &self.urls[0];
        let dpak = base_url.domain().ok_or(anyhow::anyhow!(
            "Invalid URL in DREF file: {} (missing domain)",
            base_url
        ))?;
        let (mut base_img, base_offset) =
            loader.load_image(&self.dir, dpak, base_url.path().trim_start_matches("/"))?;
        if let Some(o) = base_offset {
            eprintln!("WARN: Base image offset: left={}, top={}", o.left, o.top);
            crate::COUNTER.inc_warning();
        }
        for url in &self.urls[1..] {
            let dpak = url.domain().ok_or(anyhow::anyhow!(
                "Invalid URL in DREF file: {} (missing domain)",
                url
            ))?;
            let (mut img, img_offset) =
                loader.load_image(&self.dir, dpak, url.path().trim_start_matches("/"))?;
            let (top, left) = match img_offset {
                Some(o) => (o.top, o.left),
                None => (0, 0),
            };
            if base_img.color_type != img.color_type {
                if base_img.color_type == ImageColorType::Rgba
                    && img.color_type == ImageColorType::Rgb
                {
                    convert_rgb_to_rgba(&mut img)?;
                } else if base_img.color_type == ImageColorType::Bgra
                    && img.color_type == ImageColorType::Bgr
                {
                    convert_bgr_to_bgra(&mut img)?;
                } else if base_img.color_type == ImageColorType::Rgba
                    && img.color_type == ImageColorType::Bgra
                {
                    convert_bgra_to_rgba(&mut img)?;
                } else if base_img.color_type == ImageColorType::Bgra
                    && img.color_type == ImageColorType::Rgba
                {
                    convert_rgba_to_bgra(&mut img)?;
                }
            }
            draw_on_img_with_opacity(&mut base_img, &img, left, top, 0xff)?;
        }
        Ok(base_img)
    }
}
