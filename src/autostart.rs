// HKCU\Software\Microsoft\Windows\CurrentVersion\Run autostart entry.
// Per-user, no admin needed.

#![cfg(windows)]

use anyhow::Result;
use windows::core::HSTRING;
use windows::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
    HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ,
};

const SUBKEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const VALUE: &str = "BlackholeScreensaver";

pub fn is_enabled() -> bool {
    unsafe {
        let mut key = HKEY::default();
        if RegOpenKeyExW(HKEY_CURRENT_USER, &HSTRING::from(SUBKEY), 0, KEY_READ, &mut key).is_err()
        {
            return false;
        }
        let mut size: u32 = 0;
        let exists = RegQueryValueExW(
            key,
            &HSTRING::from(VALUE),
            None,
            None,
            None,
            Some(&mut size),
        )
        .is_ok();
        let _ = RegCloseKey(key);
        exists
    }
}

pub fn enable() -> Result<()> {
    let exe = std::env::current_exe()?;
    // Quote the path; the value is a single command line.
    let cmd = format!("\"{}\" --daemon", exe.display());
    unsafe {
        let mut key = HKEY::default();
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            &HSTRING::from(SUBKEY),
            0,
            None,
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE,
            None,
            &mut key,
            None,
        )
        .ok()?;
        let wide: Vec<u16> = cmd.encode_utf16().chain(std::iter::once(0)).collect();
        let bytes = std::slice::from_raw_parts(
            wide.as_ptr() as *const u8,
            wide.len() * std::mem::size_of::<u16>(),
        );
        RegSetValueExW(key, &HSTRING::from(VALUE), 0, REG_SZ, Some(bytes)).ok()?;
        let _ = RegCloseKey(key);
    }
    Ok(())
}

pub fn disable() -> Result<()> {
    unsafe {
        let mut key = HKEY::default();
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            &HSTRING::from(SUBKEY),
            0,
            KEY_SET_VALUE,
            &mut key,
        )
        .is_ok()
        {
            let _ = RegDeleteValueW(key, &HSTRING::from(VALUE));
            let _ = RegCloseKey(key);
        }
    }
    Ok(())
}
