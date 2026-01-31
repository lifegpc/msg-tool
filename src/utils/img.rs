//! Image Utilities
#[cfg(feature = "image-jxl")]
use super::jxl::*;
use crate::ext::io::*;
use crate::types::*;
use anyhow::Result;
use std::convert::TryFrom;
use std::io::Write;

/// Reverses the alpha values of an image.
///
/// Only supports RGBA or BGRA images with 8-bit depth.
pub fn reverse_alpha_values(data: &mut ImageData) -> Result<()> {
    if data.color_type != ImageColorType::Rgba && data.color_type != ImageColorType::Bgra {
        return Err(anyhow::anyhow!("Image is not RGBA or BGRA"));
    }
    if data.depth != 8 {
        return Err(anyhow::anyhow!(
            "Alpha value reversal only supports 8-bit depth"
        ));
    }
    for i in (0..data.data.len()).step_by(4) {
        data.data[i + 3] = 255 - data.data[i + 3];
    }
    Ok(())
}

/// Converts a BGR image to BGRA format.
///
/// Only supports BGR images with 8-bit depth.
pub fn convert_bgr_to_bgra(data: &mut ImageData) -> Result<()> {
    if data.color_type != ImageColorType::Bgr {
        return Err(anyhow::anyhow!("Image is not BGR"));
    }
    if data.depth != 8 {
        return Err(anyhow::anyhow!(
            "BGR to BGRA conversion only supports 8-bit depth"
        ));
    }
    let mut new_data = Vec::with_capacity(data.data.len() / 3 * 4);
    for chunk in data.data.chunks_exact(3) {
        new_data.push(chunk[0]); // B
        new_data.push(chunk[1]); // G
        new_data.push(chunk[2]); // R
        new_data.push(255); // A
    }
    data.data = new_data;
    data.color_type = ImageColorType::Bgra;
    Ok(())
}

/// Converts a BGR image to RGB format.
///
/// Only supports BGR images with 8-bit depth.
pub fn convert_bgr_to_rgb(data: &mut ImageData) -> Result<()> {
    if data.color_type != ImageColorType::Bgr {
        return Err(anyhow::anyhow!("Image is not BGR"));
    }
    if data.depth != 8 {
        return Err(anyhow::anyhow!(
            "BGR to RGB conversion only supports 8-bit depth"
        ));
    }
    for i in (0..data.data.len()).step_by(3) {
        let b = data.data[i];
        data.data[i] = data.data[i + 2];
        data.data[i + 2] = b;
    }
    data.color_type = ImageColorType::Rgb;
    Ok(())
}

/// Converts a BGRA image to BGR format.
///
/// Only supports BGRA images with 8-bit depth.
pub fn convert_bgra_to_bgr(data: &mut ImageData) -> Result<()> {
    if data.color_type != ImageColorType::Bgra {
        return Err(anyhow::anyhow!("Image is not BGRA"));
    }
    if data.depth != 8 {
        return Err(anyhow::anyhow!(
            "BGRA to BGR conversion only supports 8-bit depth"
        ));
    }
    let mut new_data = Vec::with_capacity(data.data.len() / 4 * 3);
    for chunk in data.data.chunks_exact(4) {
        new_data.push(chunk[0]); // B
        new_data.push(chunk[1]); // G
        new_data.push(chunk[2]); // R
    }
    data.data = new_data;
    data.color_type = ImageColorType::Bgr;
    Ok(())
}

/// Converts a BGRA image to RGBA format.
///
/// Only supports BGRA images with 8-bit depth.
pub fn convert_bgra_to_rgba(data: &mut ImageData) -> Result<()> {
    if data.color_type != ImageColorType::Bgra {
        return Err(anyhow::anyhow!("Image is not BGRA"));
    }
    if data.depth != 8 {
        return Err(anyhow::anyhow!(
            "BGRA to RGBA conversion only supports 8-bit depth"
        ));
    }
    for i in (0..data.data.len()).step_by(4) {
        let b = data.data[i];
        data.data[i] = data.data[i + 2];
        data.data[i + 2] = b;
    }
    data.color_type = ImageColorType::Rgba;
    Ok(())
}

/// Converts an RGB image to RGBA format.
///
/// Only supports RGB images with 8-bit depth.
pub fn convert_rgb_to_rgba(data: &mut ImageData) -> Result<()> {
    if data.color_type != ImageColorType::Rgb {
        return Err(anyhow::anyhow!("Image is not RGB"));
    }
    if data.depth != 8 {
        return Err(anyhow::anyhow!(
            "RGB to RGBA conversion only supports 8-bit depth"
        ));
    }
    let mut new_data = Vec::with_capacity(data.data.len() / 3 * 4);
    for chunk in data.data.chunks_exact(3) {
        new_data.push(chunk[0]); // R
        new_data.push(chunk[1]); // G
        new_data.push(chunk[2]); // B
        new_data.push(255); // A
    }
    data.data = new_data;
    data.color_type = ImageColorType::Rgba;
    Ok(())
}

/// Converts an RGB image to BGR format.
///
/// Only supports RGB images with 8-bit depth.
pub fn convert_rgb_to_bgr(data: &mut ImageData) -> Result<()> {
    if data.color_type != ImageColorType::Rgb {
        return Err(anyhow::anyhow!("Image is not RGB"));
    }
    if data.depth != 8 {
        return Err(anyhow::anyhow!(
            "RGB to BGR conversion only supports 8-bit depth"
        ));
    }
    for i in (0..data.data.len()).step_by(3) {
        let r = data.data[i];
        data.data[i] = data.data[i + 2];
        data.data[i + 2] = r;
    }
    data.color_type = ImageColorType::Bgr;
    Ok(())
}

/// Converts an RGBA image to BGRA format.
///
/// Only supports RGBA images with 8-bit depth.
pub fn convert_rgba_to_bgra(data: &mut ImageData) -> Result<()> {
    if data.color_type != ImageColorType::Rgba {
        return Err(anyhow::anyhow!("Image is not RGBA"));
    }
    if data.depth != 8 {
        return Err(anyhow::anyhow!(
            "RGBA to BGRA conversion only supports 8-bit depth"
        ));
    }
    for i in (0..data.data.len()).step_by(4) {
        let r = data.data[i];
        data.data[i] = data.data[i + 2];
        data.data[i + 2] = r;
    }
    data.color_type = ImageColorType::Bgra;
    Ok(())
}

/// Converts a Grayscale image to RGB format.
pub fn convert_grayscale_to_rgb(data: &mut ImageData) -> Result<()> {
    if data.color_type != ImageColorType::Grayscale {
        return Err(anyhow::anyhow!("Image is not Grayscale"));
    }
    if data.depth != 8 {
        return Err(anyhow::anyhow!(
            "Grayscale to RGB conversion only supports 8-bit depth"
        ));
    }
    let mut new_data = Vec::with_capacity(data.data.len() * 3);
    for &gray in &data.data {
        new_data.push(gray); // R
        new_data.push(gray); // G
        new_data.push(gray); // B
    }
    data.data = new_data;
    data.color_type = ImageColorType::Rgb;
    Ok(())
}

/// Converts a Grayscale image to RGBA format.
pub fn convert_grayscale_to_rgba(data: &mut ImageData) -> Result<()> {
    if data.color_type != ImageColorType::Grayscale {
        return Err(anyhow::anyhow!("Image is not Grayscale"));
    }
    if data.depth != 8 {
        return Err(anyhow::anyhow!(
            "Grayscale to RGBA conversion only supports 8-bit depth"
        ));
    }
    let mut new_data = Vec::with_capacity(data.data.len() * 4);
    for &gray in &data.data {
        new_data.push(gray); // R
        new_data.push(gray); // G
        new_data.push(gray); // B
        new_data.push(255); // A
    }
    data.data = new_data;
    data.color_type = ImageColorType::Rgba;
    Ok(())
}

/// Converts an image to RGBA format.
pub fn convert_to_rgba(data: &mut ImageData) -> Result<()> {
    match data.color_type {
        ImageColorType::Rgb => convert_rgb_to_rgba(data),
        ImageColorType::Bgr => convert_bgr_to_bgra(data),
        ImageColorType::Rgba => Ok(()),
        ImageColorType::Bgra => convert_bgra_to_rgba(data),
        ImageColorType::Grayscale => convert_grayscale_to_rgba(data),
    }
}

/// Encodes an image to the specified format and writes it to a file.
///
/// * `data` - The image data to encode.
/// * `typ` - The output image format.
/// * `filename` - The path of the file to write the encoded image to.
/// * `config` - Extra configuration.
pub fn encode_img(
    data: ImageData,
    typ: ImageOutputType,
    filename: &str,
    config: &ExtraConfig,
) -> Result<()> {
    let mut file = crate::utils::files::write_file(filename)?;
    encode_img_writer(data, typ, &mut file, config)
}

/// Encodes an image to the specified format and writes it to a writer.
///
/// * `data` - The image data to encode.
/// * `typ` - The output image format.
/// * `filename` - The path of the file to write the encoded image to.
/// * `config` - Extra configuration.
pub fn encode_img_writer<T: Write>(
    mut data: ImageData,
    typ: ImageOutputType,
    mut file: &mut T,
    config: &ExtraConfig,
) -> Result<()> {
    match typ {
        ImageOutputType::Png => {
            let color_type = match data.color_type {
                ImageColorType::Grayscale => png::ColorType::Grayscale,
                ImageColorType::Rgb => png::ColorType::Rgb,
                ImageColorType::Rgba => png::ColorType::Rgba,
                ImageColorType::Bgr => {
                    convert_bgr_to_rgb(&mut data)?;
                    png::ColorType::Rgb
                }
                ImageColorType::Bgra => {
                    convert_bgra_to_rgba(&mut data)?;
                    png::ColorType::Rgba
                }
            };
            let bit_depth = match &data.depth {
                1 => png::BitDepth::One,
                2 => png::BitDepth::Two,
                4 => png::BitDepth::Four,
                8 => png::BitDepth::Eight,
                16 => png::BitDepth::Sixteen,
                _ => return Err(anyhow::anyhow!("Unsupported bit depth: {}", data.depth)),
            };
            let mut encoder = png::Encoder::new(&mut file, data.width, data.height);
            encoder.set_color(color_type);
            encoder.set_depth(bit_depth);
            encoder.set_compression(config.png_compression_level.to_compression());
            let mut writer = encoder.write_header()?;
            writer.write_image_data(&data.data)?;
            writer.finish()?;
            Ok(())
        }
        #[cfg(feature = "image-jpg")]
        ImageOutputType::Jpg => {
            let color_type = match data.color_type {
                ImageColorType::Grayscale => mozjpeg::ColorSpace::JCS_GRAYSCALE,
                ImageColorType::Rgb => mozjpeg::ColorSpace::JCS_RGB,
                ImageColorType::Rgba => mozjpeg::ColorSpace::JCS_EXT_RGBA,
                ImageColorType::Bgr => {
                    convert_bgr_to_rgb(&mut data)?;
                    mozjpeg::ColorSpace::JCS_RGB
                }
                ImageColorType::Bgra => {
                    convert_bgra_to_rgba(&mut data)?;
                    mozjpeg::ColorSpace::JCS_EXT_RGBA
                }
            };
            if data.depth != 8 {
                return Err(anyhow::anyhow!(
                    "JPEG encoding only supports 8-bit depth, found: {}",
                    data.depth
                ));
            }
            let mut encoder = mozjpeg::compress::Compress::new(color_type);
            encoder.set_size(data.width as usize, data.height as usize);
            encoder.set_quality(config.jpeg_quality as f32);
            let mut start = encoder.start_compress(file)?;
            start.write_scanlines(&data.data)?;
            start.finish()?;
            Ok(())
        }
        #[cfg(feature = "image-webp")]
        ImageOutputType::Webp => {
            let color_type = match data.color_type {
                ImageColorType::Rgb => webp::PixelLayout::Rgb,
                ImageColorType::Rgba => webp::PixelLayout::Rgba,
                ImageColorType::Bgr => {
                    convert_bgr_to_rgb(&mut data)?;
                    webp::PixelLayout::Rgb
                }
                ImageColorType::Bgra => {
                    convert_bgra_to_rgba(&mut data)?;
                    webp::PixelLayout::Rgba
                }
                _ => {
                    return Err(anyhow::anyhow!(
                        "Unsupported color type for WebP: {:?}",
                        data.color_type
                    ));
                }
            };
            if data.depth != 8 {
                return Err(anyhow::anyhow!(
                    "WebP encoding only supports 8-bit depth, found: {}",
                    data.depth
                ));
            }
            let encoder = webp::Encoder::new(&data.data, color_type, data.width, data.height);
            let re = encoder
                .encode_simple(config.webp_lossless, config.webp_quality as f32)
                .map_err(|e| anyhow::anyhow!("Failed to encode WebP image: {:?}", e))?;
            file.write_all(&re)?;
            Ok(())
        }
        #[cfg(feature = "image-jxl")]
        ImageOutputType::Jxl => {
            let data = encode_jxl(data, config)?;
            file.write_all(&data)?;
            Ok(())
        }
    }
}

pub fn convert_grayscale_alpha_to_rgba(
    raw_data: &[u8],
    width: usize,
    height: usize,
    bit_depth: u8,
) -> Result<ImageData> {
    if bit_depth != 8 && bit_depth != 16 {
        return Err(anyhow::anyhow!(
            "Unsupported bit depth for GrayscaleAlpha to RGBA conversion: {}",
            bit_depth
        ));
    }
    let bytes_per_channel = (bit_depth / 8) as usize;
    let stride = width * 2 * bytes_per_channel;
    if raw_data.len() != stride * height {
        return Err(anyhow::anyhow!(
            "Input data size does not match expected size for GrayscaleAlpha image"
        ));
    }
    let mut data = Vec::with_capacity(width * height * 4 * bytes_per_channel);
    for y in 0..height {
        for x in 0..width {
            let base = y * stride + x * 2 * bytes_per_channel;
            // Grayscale channel
            data.extend_from_slice(&raw_data[base..base + bytes_per_channel]);
            data.extend_from_slice(&raw_data[base..base + bytes_per_channel]);
            data.extend_from_slice(&raw_data[base..base + bytes_per_channel]);
            // Alpha channel
            data.extend_from_slice(
                &raw_data[base + bytes_per_channel..base + 2 * bytes_per_channel],
            );
        }
    }
    Ok(ImageData {
        width: width as u32,
        height: height as u32,
        depth: bit_depth,
        color_type: ImageColorType::Rgba,
        data,
    })
}

/// Loads a PNG image from the given reader and returns its data.
pub fn load_png<R: std::io::Read + std::io::Seek>(data: R) -> Result<ImageData> {
    let decoder = png::Decoder::new(std::io::BufReader::new(data));
    let mut reader = decoder.read_info()?;
    let bit_depth = match reader.info().bit_depth {
        png::BitDepth::One => 1,
        png::BitDepth::Two => 2,
        png::BitDepth::Four => 4,
        png::BitDepth::Eight => 8,
        png::BitDepth::Sixteen => 16,
    };
    let color_type = match reader.info().color_type {
        png::ColorType::Grayscale => ImageColorType::Grayscale,
        png::ColorType::Rgb => ImageColorType::Rgb,
        png::ColorType::Rgba => ImageColorType::Rgba,
        png::ColorType::GrayscaleAlpha => ImageColorType::Rgba,
        png::ColorType::Indexed => {
            if let Some(palette) = &reader.info().palette {
                if palette.len() % 3 != 0 {
                    return Err(anyhow::anyhow!(
                        "Invalid PNG palette length: {}",
                        palette.len()
                    ));
                }
                ImageColorType::Rgb
            } else {
                return Err(anyhow::anyhow!(
                    "PNG image has indexed color type but no palette"
                ));
            }
        }
    };
    if reader.info().color_type == png::ColorType::GrayscaleAlpha {
        let height = reader.info().height as usize;
        let width = reader.info().width as usize;
        let raw_stride = width * 2 * bit_depth as usize / 8;
        let mut raw_data = vec![0; raw_stride * height];
        reader.next_frame(&mut raw_data)?;
        return convert_grayscale_alpha_to_rgba(&raw_data, width, height, bit_depth);
    }
    if reader.info().color_type == png::ColorType::Indexed {
        let mut palette = reader
            .info()
            .palette
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("PNG image has indexed color type but no palette"))?
            .to_vec();
        let mut palette_format = PaletteFormat::Rgb;
        if let Some(trns) = &reader.info().trns {
            let mut new_palette = Vec::with_capacity(palette.len() / 3 * 4);
            let trns_len = trns.len();
            for i in 0..(palette.len() / 3) {
                new_palette.push(palette[i * 3]);
                new_palette.push(palette[i * 3 + 1]);
                new_palette.push(palette[i * 3 + 2]);
                let alpha = if i < trns_len { trns[i] } else { 255 };
                new_palette.push(alpha);
            }
            palette = new_palette;
            palette_format = PaletteFormat::RgbA;
        }
        let width = reader.info().width as usize;
        let height = reader.info().height as usize;
        let raw_stride = width * bit_depth as usize / 8;
        let mut raw_data = vec![0; raw_stride * height];
        reader.next_frame(&mut raw_data)?;
        return convert_index_palette_to_normal_bitmap(
            &raw_data,
            bit_depth as usize,
            &palette,
            palette_format,
            width,
            height,
        );
    }
    let stride = reader.info().width as usize * color_type.bpp(bit_depth) as usize / 8;
    let mut data = vec![0; stride * reader.info().height as usize];
    reader.next_frame(&mut data)?;
    Ok(ImageData {
        width: reader.info().width,
        height: reader.info().height,
        depth: bit_depth,
        color_type,
        data,
    })
}

#[cfg(feature = "mozjpeg")]
pub fn load_jpg<R: std::io::Read>(data: R) -> Result<ImageData> {
    let decoder = mozjpeg::decompress::Decompress::new_reader(std::io::BufReader::new(data))?;
    let color_type = match decoder.color_space() {
        mozjpeg::ColorSpace::JCS_GRAYSCALE => ImageColorType::Grayscale,
        mozjpeg::ColorSpace::JCS_RGB => ImageColorType::Rgb,
        mozjpeg::ColorSpace::JCS_EXT_RGBA => ImageColorType::Rgba,
        _ => ImageColorType::Rgb, // Convert other types to RGB
    };
    let width = decoder.width() as u32;
    let height = decoder.height() as u32;
    let stride = width as usize * color_type.bpp(8) as usize / 8;
    let mut data = vec![0; stride * height as usize];
    let mut re = match color_type {
        ImageColorType::Grayscale => decoder.grayscale()?,
        ImageColorType::Rgb => decoder.rgb()?,
        ImageColorType::Rgba => decoder.rgba()?,
        _ => {
            unreachable!(); // We already checked the color type above
        }
    };
    re.read_scanlines_into(&mut data)?;
    Ok(ImageData {
        width,
        height,
        depth: 8,
        color_type,
        data,
    })
}

/// Decodes an image from the specified file path and returns its data.
///
/// * `typ` - The type of the image to decode.
/// * `filename` - The path of the file to decode.
pub fn decode_img(typ: ImageOutputType, filename: &str) -> Result<ImageData> {
    match typ {
        ImageOutputType::Png => {
            let file = crate::utils::files::read_file(filename)?;
            let reader = MemReader::new(file);
            load_png(reader)
        }
        #[cfg(feature = "image-jpg")]
        ImageOutputType::Jpg => {
            let file = crate::utils::files::read_file(filename)?;
            load_jpg(&file[..])
        }
        #[cfg(feature = "image-webp")]
        ImageOutputType::Webp => {
            let file = crate::utils::files::read_file(filename)?;
            let decoder = webp::Decoder::new(&file);
            let image = decoder
                .decode()
                .ok_or(anyhow::anyhow!("Failed to decode WebP image"))?;
            let color_type = if image.is_alpha() {
                ImageColorType::Rgba
            } else {
                ImageColorType::Rgb
            };
            let width = image.width();
            let height = image.height();
            let stride = width as usize * color_type.bpp(8) as usize / 8;
            let mut data = vec![0; stride * height as usize];
            if image.len() != data.len() {
                return Err(anyhow::anyhow!(
                    "WebP image data size mismatch: expected {}, got {}",
                    data.len(),
                    image.len()
                ));
            }
            data.copy_from_slice(&image);
            Ok(ImageData {
                width,
                height,
                depth: 8,
                color_type,
                data,
            })
        }
        #[cfg(feature = "image-jxl")]
        ImageOutputType::Jxl => {
            let file = crate::utils::files::read_file(filename)?;
            decode_jxl(&file[..])
        }
    }
}

/// Draws an image on a canvas with specified offsets.
///
/// * `img` - The image data to draw.
/// * `canvas_width` - The width of the canvas.
/// * `canvas_height` - The height of the canvas.
/// * `offset_x` - The horizontal offset to start drawing the image.
/// * `offset_y` - The vertical offset to start drawing the image.
///
/// Returns the canvas image data.
pub fn draw_on_canvas(
    img: ImageData,
    canvas_width: u32,
    canvas_height: u32,
    offset_x: u32,
    offset_y: u32,
) -> Result<ImageData> {
    let bytes_per_pixel = img.color_type.bpp(img.depth) as u32 / 8;
    let mut canvas_data = vec![0u8; (canvas_width * canvas_height * bytes_per_pixel) as usize];
    let canvas_stride = canvas_width * bytes_per_pixel;
    let img_stride = img.width * bytes_per_pixel;

    for y in 0..img.height {
        let canvas_y = y + offset_y;
        if canvas_y >= canvas_height {
            continue;
        }
        let canvas_start = (canvas_y * canvas_stride + offset_x * bytes_per_pixel) as usize;
        let img_start = (y * img_stride) as usize;
        let copy_len = img_stride as usize;
        if canvas_start + copy_len > canvas_data.len() {
            continue;
        }
        canvas_data[canvas_start..canvas_start + copy_len]
            .copy_from_slice(&img.data[img_start..img_start + copy_len]);
    }

    Ok(ImageData {
        width: canvas_width,
        height: canvas_height,
        color_type: img.color_type,
        depth: img.depth,
        data: canvas_data,
    })
}

/// Flips an image vertically.
pub fn flip_image(data: &mut ImageData) -> Result<()> {
    if data.height <= 1 {
        return Ok(());
    }
    let row_size = data.color_type.bpp(data.depth) as usize * data.width as usize / 8;
    if row_size == 0 {
        return Ok(());
    }

    let mut i = 0;
    let mut j = data.height as usize - 1;
    while i < j {
        let (top, bottom) = data.data.split_at_mut(j * row_size);
        let top_row = &mut top[i * row_size..i * row_size + row_size];
        let bottom_row = &mut bottom[0..row_size];
        top_row.swap_with_slice(bottom_row);
        i += 1;
        j -= 1;
    }

    Ok(())
}

/// Applies opacity to an image.
///
/// Only supports RGBA or BGRA images with 8-bit depth.
pub fn apply_opacity(img: &mut ImageData, opacity: u8) -> Result<()> {
    if img.color_type != ImageColorType::Rgba && img.color_type != ImageColorType::Bgra {
        return Err(anyhow::anyhow!("Image is not RGBA or BGRA"));
    }
    if img.depth != 8 {
        return Err(anyhow::anyhow!(
            "Opacity application only supports 8-bit depth"
        ));
    }
    for i in (0..img.data.len()).step_by(4) {
        img.data[i + 3] = (img.data[i + 3] as u16 * opacity as u16 / 255) as u8;
    }
    Ok(())
}

/// Draws an image on another image. The pixel data of `diff` will completely overwrite the pixel data of `base`.
///
/// * `base` - The base image to draw on.
/// * `diff` - The image to draw.
/// * `left` - The horizontal offset to start drawing the image.
/// * `top` - The vertical offset to start drawing the image.
pub fn draw_on_image(base: &mut ImageData, diff: &ImageData, left: u32, top: u32) -> Result<()> {
    if base.color_type != diff.color_type {
        return Err(anyhow::anyhow!("Image color types do not match"));
    }
    if base.depth != diff.depth {
        return Err(anyhow::anyhow!("Image depths do not match"));
    }

    let bits_per_pixel = base.color_type.bpp(base.depth) as usize;
    if bits_per_pixel == 0 || bits_per_pixel % 8 != 0 {
        return Err(anyhow::anyhow!(
            "Unsupported pixel bit layout: {} bits",
            bits_per_pixel
        ));
    }
    let bpp = bits_per_pixel / 8;

    let base_stride = base.width as usize * bpp;
    let diff_stride = diff.width as usize * bpp;

    for y in 0..diff.height {
        let base_y = top + y;
        if base_y >= base.height {
            continue;
        }

        for x in 0..diff.width {
            let base_x = left + x;
            if base_x >= base.width {
                continue;
            }

            let diff_idx = (y as usize * diff_stride) + (x as usize * bpp);
            let base_idx = (base_y as usize * base_stride) + (base_x as usize * bpp);

            // safety: bounds should hold given width/height checks, but guard to avoid panics
            if diff_idx + bpp > diff.data.len() || base_idx + bpp > base.data.len() {
                continue;
            }

            base.data[base_idx..base_idx + bpp]
                .copy_from_slice(&diff.data[diff_idx..diff_idx + bpp]);
        }
    }

    Ok(())
}

/// Draws an image on another image with specified opacity.
///
/// * `base` - The base image to draw on.
/// * `diff` - The image to draw with opacity.
/// * `left` - The horizontal offset to start drawing the image.
/// * `top` - The vertical offset to start drawing the image.
/// * `opacity` - The opacity level to apply to the drawn image (0-255
pub fn draw_on_img_with_opacity(
    base: &mut ImageData,
    diff: &ImageData,
    left: u32,
    top: u32,
    opacity: u8,
) -> Result<()> {
    if base.color_type != diff.color_type {
        return Err(anyhow::anyhow!("Image color types do not match"));
    }
    if base.color_type != ImageColorType::Rgba && base.color_type != ImageColorType::Bgra {
        return Err(anyhow::anyhow!("Images are not RGBA or BGRA"));
    }
    if base.depth != 8 || diff.depth != 8 {
        return Err(anyhow::anyhow!(
            "Image drawing with opacity only supports 8-bit depth"
        ));
    }

    let bpp = 4;
    let base_stride = base.width as usize * bpp;
    let diff_stride = diff.width as usize * bpp;

    for y in 0..diff.height {
        let base_y = top + y;
        if base_y >= base.height {
            continue;
        }

        for x in 0..diff.width {
            let base_x = left + x;
            if base_x >= base.width {
                continue;
            }

            let diff_idx = (y as usize * diff_stride) + (x as usize * bpp);
            let base_idx = (base_y as usize * base_stride) + (base_x as usize * bpp);

            let diff_pixel = &diff.data[diff_idx..diff_idx + bpp];
            let base_pixel_orig = base.data[base_idx..base_idx + bpp].to_vec();

            let src_alpha_u16 = (diff_pixel[3] as u16 * opacity as u16) / 255;

            if src_alpha_u16 == 0 {
                continue;
            }

            let dst_alpha_u16 = base_pixel_orig[3] as u16;

            // out_alpha = src_alpha + dst_alpha * (1 - src_alpha)
            let out_alpha_u16 = src_alpha_u16 + (dst_alpha_u16 * (255 - src_alpha_u16)) / 255;

            if out_alpha_u16 == 0 {
                for i in 0..4 {
                    base.data[base_idx + i] = 0;
                }
                continue;
            }

            // out_color = (src_color * src_alpha + dst_color * dst_alpha * (1 - src_alpha)) / out_alpha
            for i in 0..3 {
                let src_comp = diff_pixel[i] as u16;
                let dst_comp = base_pixel_orig[i] as u16;

                let numerator = src_comp * src_alpha_u16
                    + (dst_comp * dst_alpha_u16 * (255 - src_alpha_u16)) / 255;
                base.data[base_idx + i] = (numerator / out_alpha_u16) as u8;
            }
            base.data[base_idx + 3] = out_alpha_u16 as u8;
        }
    }

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteFormat {
    /// R G B Color
    Rgb,
    /// B G R Color
    Bgr,
    /// R G B Color with a unused byte
    RgbX,
    /// B G R Color with a unused byte
    BgrX,
    /// R G B A Color
    RgbA,
    /// B G R A Color
    BgrA,
}

/// Converts indexed pixel data and a palette into a standard image buffer.
///
/// `pixel_size` is expressed in bits per pixel for the indexed data.
pub fn convert_index_palette_to_normal_bitmap(
    pixel_data: &[u8],
    pixel_size: usize,
    palettes: &[u8],
    palette_format: PaletteFormat,
    width: usize,
    height: usize,
) -> Result<ImageData> {
    if width == 0 || height == 0 {
        return Err(anyhow::anyhow!("Image dimensions must be non-zero"));
    }
    if pixel_size == 0 {
        return Err(anyhow::anyhow!("pixel_size must be greater than zero"));
    }

    let width_u32 =
        u32::try_from(width).map_err(|_| anyhow::anyhow!("width exceeds u32::MAX: {}", width))?;
    let height_u32 = u32::try_from(height)
        .map_err(|_| anyhow::anyhow!("height exceeds u32::MAX: {}", height))?;

    let pixel_count = width
        .checked_mul(height)
        .ok_or_else(|| anyhow::anyhow!("Image dimensions overflow: {}x{}", width, height))?;

    let palette_entry_size = match palette_format {
        PaletteFormat::Rgb | PaletteFormat::Bgr => 3usize,
        PaletteFormat::RgbX | PaletteFormat::BgrX | PaletteFormat::RgbA | PaletteFormat::BgrA => {
            4usize
        }
    };

    if palettes.len() < palette_entry_size {
        return Err(anyhow::anyhow!("Palette data is too small"));
    }
    if palettes.len() % palette_entry_size != 0 {
        return Err(anyhow::anyhow!(
            "Palette length {} is not a multiple of {}",
            palettes.len(),
            palette_entry_size
        ));
    }
    let palette_color_count = palettes.len() / palette_entry_size;
    if palette_color_count == 0 {
        return Err(anyhow::anyhow!("Palette does not contain any colors"));
    }

    let (color_type, output_channels) = match palette_format {
        PaletteFormat::Rgb | PaletteFormat::RgbX => (ImageColorType::Rgb, 3usize),
        PaletteFormat::Bgr | PaletteFormat::BgrX => (ImageColorType::Bgr, 3usize),
        PaletteFormat::RgbA => (ImageColorType::Rgba, 4usize),
        PaletteFormat::BgrA => (ImageColorType::Bgra, 4usize),
    };

    let palette_table_len = palette_color_count
        .checked_mul(output_channels)
        .ok_or_else(|| anyhow::anyhow!("Palette size overflow"))?;
    let mut palette_table = Vec::with_capacity(palette_table_len);
    for idx in 0..palette_color_count {
        let base = idx * palette_entry_size;
        match palette_format {
            PaletteFormat::Rgb => {
                palette_table.extend_from_slice(&palettes[base..base + 3]);
            }
            PaletteFormat::Bgr => {
                palette_table.extend_from_slice(&palettes[base..base + 3]);
            }
            PaletteFormat::RgbX => {
                palette_table.extend_from_slice(&palettes[base..base + 3]);
            }
            PaletteFormat::BgrX => {
                palette_table.extend_from_slice(&palettes[base..base + 3]);
            }
            PaletteFormat::RgbA => {
                palette_table.extend_from_slice(&palettes[base..base + 4]);
            }
            PaletteFormat::BgrA => {
                palette_table.extend_from_slice(&palettes[base..base + 4]);
            }
        }
    }

    let total_bits_required = pixel_count
        .checked_mul(pixel_size)
        .ok_or_else(|| anyhow::anyhow!("Pixel count overflow for pixel_size {}", pixel_size))?;
    if total_bits_required > pixel_data.len() * 8 {
        return Err(anyhow::anyhow!(
            "Pixel data too short: need {} bits, have {} bits",
            total_bits_required,
            pixel_data.len() * 8
        ));
    }

    let output_len = pixel_count
        .checked_mul(output_channels)
        .ok_or_else(|| anyhow::anyhow!("Output image size overflow"))?;
    let mut output = Vec::with_capacity(output_len);

    let stride = output_channels;
    if pixel_size == 8 {
        if pixel_data.len() < pixel_count {
            return Err(anyhow::anyhow!(
                "Pixel data too short: expected {} bytes, got {}",
                pixel_count,
                pixel_data.len()
            ));
        }
        for &index in pixel_data.iter().take(pixel_count) {
            let idx = index as usize;
            if idx >= palette_color_count {
                return Err(anyhow::anyhow!(
                    "Palette index {} exceeds palette size {}",
                    idx,
                    palette_color_count
                ));
            }
            let start = idx * stride;
            output.extend_from_slice(&palette_table[start..start + stride]);
        }
    } else {
        let mut bit_offset = 0usize;
        for _ in 0..pixel_count {
            let idx = read_bits_as_usize(pixel_data, bit_offset, pixel_size)?;
            bit_offset = bit_offset
                .checked_add(pixel_size)
                .ok_or_else(|| anyhow::anyhow!("Bit offset overflow"))?;
            if idx >= palette_color_count {
                return Err(anyhow::anyhow!(
                    "Palette index {} exceeds palette size {}",
                    idx,
                    palette_color_count
                ));
            }
            let start = idx * stride;
            output.extend_from_slice(&palette_table[start..start + stride]);
        }
    }

    Ok(ImageData {
        width: width_u32,
        height: height_u32,
        color_type,
        depth: 8,
        data: output,
    })
}

fn read_bits_as_usize(data: &[u8], bit_offset: usize, bit_len: usize) -> Result<usize> {
    if bit_len == 0 {
        return Err(anyhow::anyhow!("bit_len must be greater than zero"));
    }
    if bit_len > (std::mem::size_of::<usize>() * 8) {
        return Err(anyhow::anyhow!("Cannot read {} bits into usize", bit_len));
    }

    let mut value = 0usize;
    for bit_idx in 0..bit_len {
        let absolute_bit = bit_offset + bit_idx;
        let byte_index = absolute_bit / 8;
        if byte_index >= data.len() {
            return Err(anyhow::anyhow!(
                "Bit offset {} exceeds pixel data",
                absolute_bit
            ));
        }
        let bit_in_byte = 7 - (absolute_bit % 8);
        let bit = (data[byte_index] >> bit_in_byte) & 1;
        value = (value << 1) | bit as usize;
    }
    Ok(value)
}
