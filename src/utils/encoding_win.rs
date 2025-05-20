use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::Globalization::{MB_ERR_INVALID_CHARS, MultiByteToWideChar, WideCharToMultiByte};

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
        write!(f, "Windows error code: {}", self.code)
    }
}

pub fn decode_to_string(cp: u32, data: &[u8]) -> Result<String, WinError> {
    let needed_len = unsafe {
        MultiByteToWideChar(
            cp,
            MB_ERR_INVALID_CHARS,
            data.as_ptr() as _,
            data.len() as i32,
            std::ptr::null_mut(),
            0,
        )
    };
    if needed_len == 0 {
        return Err(WinError::from_last_error());
    }
    let mut wc = Vec::with_capacity(needed_len as usize);
    wc.resize(needed_len as usize, 0);
    let result = unsafe {
        MultiByteToWideChar(
            cp,
            MB_ERR_INVALID_CHARS,
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

pub fn encode_string(cp: u32, data: &str) -> Result<Vec<u8>, WinError> {
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
    let result = unsafe {
        WideCharToMultiByte(
            cp,
            0,
            wstr.as_ptr(),
            wstr.len() as i32,
            mb.as_mut_ptr(),
            needed_len,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
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
            &[228, 184, 173, 230, 150, 135, 230, 181, 139, 232, 175, 149]
        )
        .unwrap(),
        "中文测试".to_string()
    );
    assert_eq!(
        decode_to_string(
            932,
            &[
                130, 171, 130, 225, 130, 215, 130, 194, 130, 187, 130, 211, 130, 198
            ]
        )
        .unwrap(),
        "きゃべつそふと".to_string()
    );
    assert_eq!(
        decode_to_string(936, &[214, 208, 206, 196]).unwrap(),
        "中文".to_string()
    );
}
