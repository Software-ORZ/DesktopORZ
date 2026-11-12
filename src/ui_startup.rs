// DesktopORZ — Native Rust GUI
// Copyright (C) 2026 ThainanViniciusKatchan
//
// Inicialização da interface gráfica (`DesktopORZ-GUI.exe`) junto com o
// Windows, controlada pela aba Idioma & Sistema.

use std::env;
use std::path::{Path, PathBuf};

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::ERROR_FILE_NOT_FOUND;
use windows::Win32::System::Registry::{
    RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW, HKEY,
    HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_SAM_FLAGS, REG_SZ,
};

/// Chave `Run` do usuário atual. É a mesma usada pelo CLI, mas com um nome de
/// valor próprio (`DesktopORZ-GUI`) para as duas inicializações coexistirem.
const RUN_KEY: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
const VALUE_NAME: PCWSTR = w!("DesktopORZ-GUI");

/// Caminho do executável da interface, sem o prefixo `\\?\` que a chave Run
/// não entende.
fn exe_path() -> Result<PathBuf, String> {
    let path = env::current_exe().map_err(|e| format!("{e}"))?;
    let path = path.canonicalize().unwrap_or(path);
    let Some(text) = path.to_str() else {
        return Ok(path);
    };
    Ok(match text.strip_prefix(r"\\?\") {
        Some(rest) => PathBuf::from(rest),
        None => path,
    })
}

fn command_line(exe: &Path) -> Vec<u16> {
    let mut s: Vec<u16> = format!("\"{}\"", exe.display())
        .encode_utf16()
        .collect();
    s.push(0);
    s
}

fn open_run_key(access: REG_SAM_FLAGS) -> Result<HKEY, String> {
    unsafe {
        let mut key = HKEY::default();
        let status = RegOpenKeyExW(HKEY_CURRENT_USER, RUN_KEY, 0, access, &mut key);
        if status.is_err() {
            return Err(format!("{status:?}"));
        }
        Ok(key)
    }
}

/// Cria a entrada que abre a interface junto com o Windows.
pub fn enable() -> Result<(), String> {
    let exe = exe_path()?;
    let cmd = command_line(&exe);
    unsafe {
        let key = open_run_key(KEY_SET_VALUE)?;
        let status = RegSetValueExW(
            key,
            VALUE_NAME,
            0,
            REG_SZ,
            Some(std::slice::from_raw_parts(
                cmd.as_ptr() as *const u8,
                cmd.len() * 2,
            )),
        );
        let _ = RegCloseKey(key);
        if status.is_err() {
            return Err(format!("{status:?}"));
        }
    }
    crate::config::set_ui_startup(true);
    Ok(())
}

/// Remove a entrada (idempotente: inexistente não é erro).
pub fn disable() -> Result<(), String> {
    unsafe {
        let key = open_run_key(KEY_SET_VALUE)?;
        let status = RegDeleteValueW(key, VALUE_NAME);
        let _ = RegCloseKey(key);
        if status.is_err() && status != ERROR_FILE_NOT_FOUND {
            return Err(format!("{status:?}"));
        }
    }
    crate::config::set_ui_startup(false);
    Ok(())
}

/// Ativa/desativa a inicialização da interface com o Windows.
pub fn set_enabled(enabled: bool) -> Result<(), String> {
    if enabled {
        enable()
    } else {
        disable()
    }
}

/// Estado real, lido direto do registro (fonte da verdade).
pub fn is_enabled() -> bool {
    unsafe {
        let Ok(key) = open_run_key(KEY_QUERY_VALUE) else {
            return false;
        };
        let mut size: u32 = 0;
        let query = RegQueryValueExW(key, VALUE_NAME, None, None, None, Some(&mut size));
        let _ = RegCloseKey(key);
        query.is_ok() && size > 0
    }
}
