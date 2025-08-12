//! Encoding Utilities
use crate::ext::atomic::*;
use crate::types::*;
use encoding::{ByteWriter, DecoderTrap, EncoderTrap, Encoding as EncodingTrait, RawEncoder};
use std::sync::atomic::AtomicBool;

/// Decodes a byte slice to a string using the specified encoding with BOM detection.
///
/// * `check` - If true, checks for decoding errors and returns an error if any.
///
/// Returns the decoded string and the detected BOM type.
pub fn decode_with_bom_detect(
    encoding: Encoding,
    data: &[u8],
    check: bool,
) -> Result<(String, BomType), anyhow::Error> {
    if data.len() >= 2 {
        if data[0] == 0xFE && data[1] == 0xFF {
            return Ok((
                encoding::codec::utf_16::UTF_16BE_ENCODING
                    .decode(
                        &data[2..],
                        if check {
                            DecoderTrap::Strict
                        } else {
                            DecoderTrap::Replace
                        },
                    )
                    .map_err(|_| anyhow::anyhow!("Failed to decode UTF-16BE"))?,
                BomType::Utf16BE,
            ));
        } else if data[0] == 0xFF && data[1] == 0xFE {
            return Ok((
                encoding::codec::utf_16::UTF_16LE_ENCODING
                    .decode(
                        &data[2..],
                        if check {
                            DecoderTrap::Strict
                        } else {
                            DecoderTrap::Replace
                        },
                    )
                    .map_err(|_| anyhow::anyhow!("Failed to decode UTF-16LE"))?,
                BomType::Utf16LE,
            ));
        }
    }
    if data.len() >= 3 {
        if data[0] == 0xEF && data[1] == 0xBB && data[2] == 0xBF {
            return Ok((String::from_utf8(data[3..].to_vec())?, BomType::Utf8));
        }
    }
    #[cfg(feature = "kirikiri")]
    {
        use crate::ext::io::*;
        use crate::scripts::kirikiri::mdf::Mdf;
        use crate::scripts::kirikiri::simple_crypt::SimpleCrypt;
        if data.len() >= 8 && data.starts_with(b"mdf\0") {
            let reader = MemReaderRef::new(&data[4..]);
            let decoded = Mdf::unpack(reader)?;
            return decode_with_bom_detect(encoding, &decoded, check);
        }
        if data.len() >= 5
            && data[0] == 0xFE
            && data[1] == 0xFE
            && (data[2] == 0 || data[2] == 1 || data[2] == 2)
            && data[3] == 0xFF
            && data[4] == 0xFE
        {
            let crypt = data[2];
            let reader = MemReaderRef::new(data);
            let decoded = SimpleCrypt::unpack(crypt, reader)?;
            return decode_with_bom_detect(encoding, &decoded, check);
        }
    }
    decode_to_string(encoding, data, check).map(|s| (s, BomType::None))
}

/// Decodes a byte slice to a string using the specified encoding.
///
/// * `check` - If true, checks for decoding errors and returns an error if any.
pub fn decode_to_string(
    encoding: Encoding,
    data: &[u8],
    check: bool,
) -> Result<String, anyhow::Error> {
    match encoding {
        Encoding::Auto => decode_to_string(Encoding::Utf8, data, check)
            .or_else(|_| decode_to_string(Encoding::Cp932, data, check))
            .or_else(|_| decode_to_string(Encoding::Gb2312, data, check)),
        Encoding::Utf8 => Ok(String::from_utf8(data.to_vec())?),
        Encoding::Cp932 => {
            let result = encoding::codec::japanese::Windows31JEncoding
                .decode(
                    data,
                    if check {
                        DecoderTrap::Strict
                    } else {
                        DecoderTrap::Call(|_, d, out| {
                            if d.len() == 1 && d[0] == 0xFF {
                                out.write_char('\u{f8f3}'); // PUA character for U+F8F3
                            } else {
                                out.write_char('\u{FFFD}'); // Replacement character
                            }
                            true
                        })
                    },
                )
                .map_err(|_| anyhow::anyhow!("Failed to decode Shift-JIS"))?;
            if result.contains('\u{FFFD}') {
                eprintln!(
                    "Warning: Some characters could not be decoded in Shift-JIS: {:?}",
                    data
                );
                crate::COUNTER.inc_warning();
            }
            Ok(result)
        }
        Encoding::Gb2312 => {
            let result = encoding::codec::simpchinese::GBK_ENCODING
                .decode(
                    data,
                    if check {
                        DecoderTrap::Strict
                    } else {
                        DecoderTrap::Replace
                    },
                )
                .map_err(|_| anyhow::anyhow!("Failed to decode GB2312"))?;
            if result.contains('\u{FFFD}') {
                eprintln!(
                    "Warning: Some characters could not be decoded in GB2312: {:?}",
                    data
                );
                crate::COUNTER.inc_warning();
            }
            Ok(result)
        }
        #[cfg(windows)]
        Encoding::CodePage(code_page) => Ok(super::encoding_win::decode_to_string(
            code_page, data, check,
        )?),
    }
}

thread_local! {
    static ENCODE_REPLACED: AtomicBool = AtomicBool::new(false);
}

fn jis_encoder_trap(_: &mut dyn RawEncoder, data: &str, out: &mut dyn ByteWriter) -> bool {
    if data == "\u{f8f3}" {
        out.write_byte(0xFF); // PUA character for U+F8F3
    } else {
        out.write_byte(b'?'); // Replacement character
        ENCODE_REPLACED.with(|f| f.qsave(true));
    }
    true
}

fn gbk_encoder_trap(_: &mut dyn RawEncoder, _: &str, out: &mut dyn ByteWriter) -> bool {
    out.write_byte(b'?'); // Replacement character
    ENCODE_REPLACED.with(|f| f.qsave(true));
    true
}

/// Encodes a string to a byte vector using the specified encoding.
///
/// * `check` - If true, checks for encoding errors and returns an error if any.
pub fn encode_string(
    encoding: Encoding,
    data: &str,
    check: bool,
) -> Result<Vec<u8>, anyhow::Error> {
    match encoding {
        Encoding::Auto => Ok(data.as_bytes().to_vec()),
        Encoding::Utf8 => Ok(data.as_bytes().to_vec()),
        Encoding::Cp932 => {
            ENCODE_REPLACED.with(|f| f.qsave(false));
            let result = encoding::codec::japanese::Windows31JEncoding
                .encode(
                    data,
                    if check {
                        EncoderTrap::Strict
                    } else {
                        EncoderTrap::Call(jis_encoder_trap)
                    },
                )
                .map_err(|_| anyhow::anyhow!("Failed to encode Shift-JIS"))?;
            ENCODE_REPLACED.with(|f| {
                if f.qload() {
                    eprintln!(
                        "Warning: Some characters could not be encoded in Shift-JIS: {}",
                        data
                    );
                    crate::COUNTER.inc_warning();
                }
            });
            Ok(result)
        }
        Encoding::Gb2312 => {
            ENCODE_REPLACED.with(|f| f.qsave(false));
            let result = encoding::codec::simpchinese::GBK_ENCODING
                .encode(
                    data,
                    if check {
                        EncoderTrap::Strict
                    } else {
                        EncoderTrap::Call(gbk_encoder_trap)
                    },
                )
                .map_err(|_| anyhow::anyhow!("Failed to encode GB2312"))?;
            ENCODE_REPLACED.with(|f| {
                if f.qload() {
                    eprintln!(
                        "Warning: Some characters could not be encoded in GB2312: {}",
                        data
                    );
                    crate::COUNTER.inc_warning();
                }
            });
            Ok(result)
        }
        #[cfg(windows)]
        Encoding::CodePage(code_page) => {
            Ok(super::encoding_win::encode_string(code_page, data, check)?)
        }
    }
}

/// Encodes a string to a byte vector using the specified encoding with BOM.
///
/// * `bom` - The BOM type to use.
/// * `check` - If true, checks for encoding errors and returns an error if any
pub fn encode_string_with_bom(
    encoding: Encoding,
    data: &str,
    check: bool,
    bom: BomType,
) -> Result<Vec<u8>, anyhow::Error> {
    match bom {
        BomType::None => encode_string(encoding, data, check),
        BomType::Utf8 => {
            let mut result = vec![0xEF, 0xBB, 0xBF];
            result.extend_from_slice(data.as_bytes());
            Ok(result)
        }
        BomType::Utf16LE => {
            let mut result = vec![0xFF, 0xFE];
            let re = utf16string::WString::<utf16string::LE>::from(data);
            result.extend(re.as_bytes());
            Ok(result)
        }
        BomType::Utf16BE => {
            let mut result = vec![0xFE, 0xFF];
            let re = utf16string::WString::<utf16string::BE>::from(data);
            result.extend(re.as_bytes());
            Ok(result)
        }
    }
}

#[test]
fn test_decode_to_string() {
    assert_eq!(
        decode_to_string(
            Encoding::Utf8,
            &[228, 184, 173, 230, 150, 135, 230, 181, 139, 232, 175, 149],
            true
        )
        .unwrap(),
        "中文测试".to_string()
    );
    assert_eq!(
        decode_to_string(
            Encoding::Cp932,
            &[
                130, 171, 130, 225, 130, 215, 130, 194, 130, 187, 130, 211, 130, 198
            ],
            true
        )
        .unwrap(),
        "きゃべつそふと".to_string()
    );
    assert_eq!(
        decode_to_string(Encoding::Gb2312, &[214, 208, 206, 196], true).unwrap(),
        "中文".to_string()
    );
    assert_eq!(
        decode_to_string(
            Encoding::Auto,
            &[228, 184, 173, 230, 150, 135, 230, 181, 139, 232, 175, 149],
            true
        )
        .unwrap(),
        "中文测试".to_string()
    );
    assert_eq!(
        decode_to_string(
            Encoding::Auto,
            &[
                130, 171, 130, 225, 130, 215, 130, 194, 130, 187, 130, 211, 130, 198
            ],
            true
        )
        .unwrap(),
        "きゃべつそふと".to_string()
    );
    #[cfg(windows)]
    assert_eq!(
        decode_to_string(Encoding::CodePage(936), &[214, 208, 206, 196], true).unwrap(),
        "中文".to_string()
    );
}

#[test]
fn test_encode_string() {
    assert_eq!(
        encode_string(Encoding::Utf8, "中文测试", true).unwrap(),
        vec![228, 184, 173, 230, 150, 135, 230, 181, 139, 232, 175, 149]
    );
    assert_eq!(
        encode_string(Encoding::Cp932, "きゃべつそふと", true).unwrap(),
        vec![
            130, 171, 130, 225, 130, 215, 130, 194, 130, 187, 130, 211, 130, 198
        ]
    );
    assert_eq!(
        encode_string(Encoding::Gb2312, "中文", true).unwrap(),
        vec![214, 208, 206, 196]
    );
    #[cfg(windows)]
    assert_eq!(
        encode_string(Encoding::CodePage(936), "中文", true).unwrap(),
        vec![214, 208, 206, 196]
    );
}

#[test]
fn test_decode_with_bom_detect() {
    let utf8_data = vec![0xEF, 0xBB, 0xBF, 0xE4, 0xB8, 0xAD, 0xE6, 0x96, 0x87];
    let (decoded_utf8, bom_type) =
        decode_with_bom_detect(Encoding::Auto, &utf8_data, true).unwrap();
    assert_eq!(decoded_utf8, "中文");
    assert_eq!(bom_type, BomType::Utf8);
    let utf16le_data = vec![0xFF, 0xFE, 0x2D, 0x4E, 0x87, 0x65];
    let (decoded_utf16le, bom_type) =
        decode_with_bom_detect(Encoding::Auto, &utf16le_data, true).unwrap();
    assert_eq!(decoded_utf16le, "中文");
    assert_eq!(bom_type, BomType::Utf16LE);
    let utf16be_data = vec![0xFE, 0xFF, 0x4E, 0x2D, 0x65, 0x87];
    let (decoded_utf16be, bom_type) =
        decode_with_bom_detect(Encoding::Auto, &utf16be_data, true).unwrap();
    assert_eq!(decoded_utf16be, "中文");
    assert_eq!(bom_type, BomType::Utf16BE);
    let no_bom_data = vec![0xE4, 0xB8, 0xAD, 0xE6, 0x96, 0x87];
    let (decoded_no_bom, bom_type) =
        decode_with_bom_detect(Encoding::Auto, &no_bom_data, true).unwrap();
    assert_eq!(decoded_no_bom, "中文");
    assert_eq!(bom_type, BomType::None);
    #[cfg(feature = "kirikiri")]
    {
        let simple_crypt_data = vec![
            0xFE, 0xFE, 0x01, 0xFF, 0xFE, // Header
            0x11, 0x00, 0x34, 0x00, 0x36, 0x00, 0x3a, 0x00, 0x11, 0x00, 0x0e, 0x00, 0x05, 0x00,
        ];
        let (decoded_simple_crypt, bom_type) =
            decode_with_bom_detect(Encoding::Auto, &simple_crypt_data, true).unwrap();
        assert_eq!(decoded_simple_crypt, "\"895\"\r\n");
        assert_eq!(bom_type, BomType::Utf16LE);
    }
}

#[test]
fn test_encode_string_with_bom() {
    assert_eq!(
        encode_string_with_bom(Encoding::Utf8, "中文", true, BomType::Utf8).unwrap(),
        vec![0xEF, 0xBB, 0xBF, 0xE4, 0xB8, 0xAD, 0xE6, 0x96, 0x87]
    );
    assert_eq!(
        encode_string_with_bom(Encoding::Utf8, "中文", true, BomType::Utf16LE).unwrap(),
        vec![0xFF, 0xFE, 0x2D, 0x4E, 0x87, 0x65]
    );
    assert_eq!(
        encode_string_with_bom(Encoding::Utf8, "中文", true, BomType::Utf16BE).unwrap(),
        vec![0xFE, 0xFF, 0x4E, 0x2D, 0x65, 0x87]
    );
    assert_eq!(
        encode_string_with_bom(Encoding::Utf8, "中文", true, BomType::None).unwrap(),
        vec![0xE4, 0xB8, 0xAD, 0xE6, 0x96, 0x87]
    );
}

#[test]
fn shift_jis_pua_test() {
    let ff = [0xFF, 0x01];
    #[cfg(windows)]
    assert_eq!(
        decode_to_string(Encoding::CodePage(932), &ff, false).unwrap(),
        "\u{f8f3}\x01".to_string()
    );
    assert_eq!(
        decode_to_string(Encoding::Cp932, &ff, false).unwrap(),
        "\u{f8f3}\x01".to_string()
    );
}
