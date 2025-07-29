use crate::types::*;
use anyhow::Result;

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

pub fn encode_img(
    mut data: ImageData,
    typ: ImageOutputType,
    filename: &str,
    config: &ExtraConfig,
) -> Result<()> {
    match typ {
        ImageOutputType::Png => {
            let mut file = crate::utils::files::write_file(filename)?;
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
    }
}

pub fn load_png<R: std::io::Read>(data: R) -> Result<ImageData> {
    let decoder = png::Decoder::new(data);
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
        _ => {
            return Err(anyhow::anyhow!(
                "Unsupported color type: {:?}",
                reader.info().color_type
            ));
        }
    };
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

pub fn decode_img(typ: ImageOutputType, filename: &str) -> Result<ImageData> {
    match typ {
        ImageOutputType::Png => {
            let file = crate::utils::files::read_file(filename)?;
            load_png(&file[..])
        }
    }
}

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
