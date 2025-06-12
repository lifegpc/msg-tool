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

pub fn encode_img(mut data: ImageData, typ: ImageOutputType, filename: &str) -> Result<()> {
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
            let mut writer = encoder.write_header()?;
            writer.write_image_data(&data.data)?;
            writer.finish()?;
            Ok(())
        }
    }
}

pub fn decode_img(typ: ImageOutputType, filename: &str) -> Result<ImageData> {
    match typ {
        ImageOutputType::Png => {
            let file = crate::utils::files::read_file(filename)?;
            let decoder = png::Decoder::new(&file[..]);
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
            let mut data = vec![0; reader.info().raw_bytes()];
            reader.next_frame(&mut data)?;
            Ok(ImageData {
                width: reader.info().width,
                height: reader.info().height,
                depth: bit_depth,
                color_type,
                data,
            })
        }
    }
}
