use crate::types::*;

pub fn decode_to_string(encoding: Encoding, data: &[u8]) -> Result<String, anyhow::Error> {
    match encoding {
        Encoding::Auto => decode_to_string(Encoding::Utf8, data)
            .or_else(|_| decode_to_string(Encoding::Cp932, data))
            .or_else(|_| decode_to_string(Encoding::Gb2312, data)),
        Encoding::Utf8 => Ok(String::from_utf8(data.to_vec())?),
        Encoding::Cp932 => {
            let result = encoding_rs::SHIFT_JIS.decode(data);
            if result.2 {
                Err(anyhow::anyhow!("Failed to decode Shift-JIS"))
            } else {
                Ok(result.0.to_string())
            }
        }
        Encoding::Gb2312 => {
            let result = encoding_rs::GBK.decode(data);
            if result.2 {
                Err(anyhow::anyhow!("Failed to decode GB2312"))
            } else {
                Ok(result.0.to_string())
            }
        }
        #[cfg(windows)]
        Encoding::CodePage(code_page) => {
            Ok(super::encoding_win::decode_to_string(code_page, data)?)
        }
    }
}

pub fn encode_string(
    encoding: Encoding,
    data: &str,
    check: bool,
) -> Result<Vec<u8>, anyhow::Error> {
    match encoding {
        Encoding::Auto => Ok(data.as_bytes().to_vec()),
        Encoding::Utf8 => Ok(data.as_bytes().to_vec()),
        Encoding::Cp932 => {
            let result = encoding_rs::SHIFT_JIS.encode(data);
            if result.2 {
                if check {
                    return Err(anyhow::anyhow!("Failed to encode Shift-JIS"));
                }
                eprintln!(
                    "Warning: Some characters could not be encoded in Shift-JIS: {}",
                    data
                );
                crate::COUNTER.inc_warning();
            }
            Ok(result.0.to_vec())
        }
        Encoding::Gb2312 => {
            let result = encoding_rs::GBK.encode(data);
            if result.2 {
                if check {
                    return Err(anyhow::anyhow!("Failed to encode GB2312"));
                }
                eprintln!(
                    "Warning: Some characters could not be encoded in GB2312: {}",
                    data
                );
                crate::COUNTER.inc_warning();
            }
            Ok(result.0.to_vec())
        }
        #[cfg(windows)]
        Encoding::CodePage(code_page) => {
            Ok(super::encoding_win::encode_string(code_page, data, check)?)
        }
    }
}

#[test]
fn test_decode_to_string() {
    assert_eq!(
        decode_to_string(
            Encoding::Utf8,
            &[228, 184, 173, 230, 150, 135, 230, 181, 139, 232, 175, 149]
        )
        .unwrap(),
        "中文测试".to_string()
    );
    assert_eq!(
        decode_to_string(
            Encoding::Cp932,
            &[
                130, 171, 130, 225, 130, 215, 130, 194, 130, 187, 130, 211, 130, 198
            ]
        )
        .unwrap(),
        "きゃべつそふと".to_string()
    );
    assert_eq!(
        decode_to_string(Encoding::Gb2312, &[214, 208, 206, 196]).unwrap(),
        "中文".to_string()
    );
    assert_eq!(
        decode_to_string(
            Encoding::Auto,
            &[228, 184, 173, 230, 150, 135, 230, 181, 139, 232, 175, 149]
        )
        .unwrap(),
        "中文测试".to_string()
    );
    assert_eq!(
        decode_to_string(
            Encoding::Auto,
            &[
                130, 171, 130, 225, 130, 215, 130, 194, 130, 187, 130, 211, 130, 198
            ]
        )
        .unwrap(),
        "きゃべつそふと".to_string()
    );
    #[cfg(windows)]
    assert_eq!(
        decode_to_string(Encoding::CodePage(936), &[214, 208, 206, 196]).unwrap(),
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
