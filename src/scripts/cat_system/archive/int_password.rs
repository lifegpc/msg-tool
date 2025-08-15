//! Get Password from CatSystem2 Executable
use crate::types::*;
use crate::utils::blowfish::{Blowfish, BlowfishLE};
use crate::utils::encoding::*;
use anyhow::Result;
use pelite::FileMap;
use pelite::pe32::*;
use std::path::Path;

/// Retrieves the int archive's password from a CatSystem2 executable file.
pub fn get_password_from_exe<S: AsRef<Path> + ?Sized>(exe_path: &S) -> Result<String> {
    let path = exe_path.as_ref();
    let file_map = FileMap::open(path)?;
    let file = PeFile::from_bytes(&file_map)?;
    let resources = file.resources()?;
    let mut code = resources
        .find_resource(&["V_CODE2".into(), "DATA".into()])?
        .to_vec();
    if code.len() < 8 {
        return Err(anyhow::anyhow!("Invalid V_CODE2 resource length"));
    }
    let key = resources
        .find_resource(&["KEY_CODE".into(), "KEY".into()])
        .map(|s| {
            let mut s = s.to_vec();
            for i in s.iter_mut() {
                *i ^= 0xCD;
            }
            s
        })
        .unwrap_or_else(|_| b"windmill".to_vec());
    let blowfish: BlowfishLE = Blowfish::new(&key)?;
    blowfish.decrypt_block(&mut code);
    let len = code.iter().position(|&x| x == 0).unwrap_or(code.len());
    let result = decode_to_string(Encoding::Cp932, &code[..len], true)?;
    eprintln!("Used password from CatSystem2 executable: {}", result);
    Ok(result)
}
