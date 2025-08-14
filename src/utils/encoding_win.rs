use windows_sys::Win32::Foundation::{ERROR_NO_UNICODE_TRANSLATION, GetLastError};
use windows_sys::Win32::Globalization::{
    CP_UTF7, CP_UTF8, MB_ERR_INVALID_CHARS, MultiByteToWideChar, WideCharToMultiByte,
};
use windows_sys::Win32::System::Diagnostics::Debug::{
    FORMAT_MESSAGE_FROM_SYSTEM, FORMAT_MESSAGE_IGNORE_INSERTS, FormatMessageW,
};

#[derive(Debug)]
pub struct WinError {
    pub code: u32,
}

impl WinError {
    pub fn new(code: u32) -> Self {
        WinError { code }
    }

    pub fn from_last_error() -> Self {
        let code = unsafe { GetLastError() };
        WinError::new(code)
    }
}

impl std::error::Error for WinError {}

impl std::fmt::Display for WinError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut buffer = [0u16; 256];
        let len = unsafe {
            FormatMessageW(
                FORMAT_MESSAGE_FROM_SYSTEM | FORMAT_MESSAGE_IGNORE_INSERTS,
                std::ptr::null(),
                self.code,
                0,
                buffer.as_mut_ptr(),
                buffer.len() as u32,
                std::ptr::null_mut(),
            )
        };
        if len == 0 {
            write!(f, "Unknown error code: 0x{:08X}", self.code)
        } else {
            let message = String::from_utf16_lossy(&buffer[..len as usize]);
            write!(f, "{} (0x{:08X})", message.trim(), self.code)
        }
    }
}

pub fn decode_to_string(cp: u32, data: &[u8], check: bool) -> Result<String, WinError> {
    if data.is_empty() {
        return Ok(String::new());
    }
    let dwflags = if check { MB_ERR_INVALID_CHARS } else { 0 };
    let needed_len = unsafe {
        MultiByteToWideChar(
            cp,
            dwflags,
            data.as_ptr() as _,
            data.len() as i32,
            std::ptr::null_mut(),
            0,
        )
    };
    if needed_len == 0 {
        return Err(WinError::from_last_error());
    }
    let last_error = unsafe { GetLastError() };
    if last_error == ERROR_NO_UNICODE_TRANSLATION {
        if check {
            return Err(WinError::new(last_error));
        } else {
            eprintln!(
                "Warning: Some characters could not be decoded in code page {}: {:?}",
                cp, data
            );
            crate::COUNTER.inc_warning();
        }
    }
    let mut wc = Vec::with_capacity(needed_len as usize);
    wc.resize(needed_len as usize, 0);
    let result = unsafe {
        MultiByteToWideChar(
            cp,
            dwflags,
            data.as_ptr() as _,
            data.len() as i32,
            wc.as_mut_ptr(),
            needed_len,
        )
    };
    if result == 0 {
        return Err(WinError::from_last_error());
    }
    Ok(String::from_utf16_lossy(&wc))
}

pub fn encode_string(cp: u32, data: &str, check: bool) -> Result<Vec<u8>, WinError> {
    if data.is_empty() {
        return Ok(Vec::new());
    }
    let wstr = data.encode_utf16().collect::<Vec<u16>>();
    let needed_len = unsafe {
        WideCharToMultiByte(
            cp,
            0,
            wstr.as_ptr(),
            wstr.len() as i32,
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    if needed_len == 0 {
        return Err(WinError::from_last_error());
    }
    let mut mb = Vec::with_capacity(needed_len as usize);
    mb.resize(needed_len as usize, 0);
    let mut used_default_char = 0;
    let result = unsafe {
        WideCharToMultiByte(
            cp,
            0,
            wstr.as_ptr(),
            wstr.len() as i32,
            mb.as_mut_ptr(),
            needed_len,
            std::ptr::null_mut(),
            if cp == CP_UTF7 || cp == CP_UTF8 {
                std::ptr::null_mut()
            } else {
                &mut used_default_char
            },
        )
    };
    if used_default_char != 0 {
        if check {
            return Err(WinError::new(0));
        } else {
            eprintln!(
                "Warning: Some characters could not be encoded in code page {}: {}",
                cp, data
            );
            crate::COUNTER.inc_warning();
        }
    }
    if result == 0 {
        return Err(WinError::from_last_error());
    }
    Ok(mb)
}

#[test]
fn test_decode_to_string() {
    assert_eq!(
        decode_to_string(
            65001,
            &[228, 184, 173, 230, 150, 135, 230, 181, 139, 232, 175, 149],
            true
        )
        .unwrap(),
        "中文测试".to_string()
    );
    assert_eq!(
        decode_to_string(
            932,
            &[
                130, 171, 130, 225, 130, 215, 130, 194, 130, 187, 130, 211, 130, 198
            ],
            true
        )
        .unwrap(),
        "きゃべつそふと".to_string()
    );
    assert_eq!(
        decode_to_string(936, &[214, 208, 206, 196], true).unwrap(),
        "中文".to_string()
    );
}

#[test]
fn test_encode_string() {
    assert_eq!(
        encode_string(65001, "中文测试", true).unwrap(),
        vec![228, 184, 173, 230, 150, 135, 230, 181, 139, 232, 175, 149]
    );
    assert_eq!(
        encode_string(932, "きゃべつそふと", true).unwrap(),
        vec![
            130, 171, 130, 225, 130, 215, 130, 194, 130, 187, 130, 211, 130, 198
        ]
    );
    assert_eq!(
        encode_string(936, "中文", true).unwrap(),
        vec![214, 208, 206, 196]
    );
    assert!(
        encode_string(
            936,
            "「あ、こーら、逃げちゃダメだよー？　起きちゃうのも、まだダメだけ\nどね♪」",
            true
        )
        .is_err()
    );
}
