// DesktopORZ — Save and restore Windows desktop icon layouts
// Copyright (C) 2026 ThainanViniciusKatchan
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

use std::env;
use std::path::{Path, PathBuf};

use crate::config::{self, StartupMirror};
use crate::hide_icons;
use crate::i18n::{t, t_args};

use windows::core::w;
use windows::Win32::Foundation::ERROR_FILE_NOT_FOUND;
use windows::Win32::System::Registry::{
    RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW, HKEY,
    HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_SAM_FLAGS, REG_SZ,
};

const RUN_KEY: windows::core::PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
const VALUE_NAME: windows::core::PCWSTR = w!("desk0k");

fn exe_path() -> Result<PathBuf, String> {
    let p = env::current_exe()
        .map(|p| p.canonicalize().unwrap_or(p))
        .map_err(|e| format!("{e}"))?;
    // No Windows o caminho pode vir com o prefixo estendido `\\?\`, que o
    // gerenciador da chave Run não entende — removemos aqui.
    Ok(strip_unc_prefix(p))
}

fn strip_unc_prefix(p: PathBuf) -> PathBuf {
    if let Some(s) = p.to_str() {
        if let Some(rest) = s.strip_prefix("\\\\?\\") {
            return PathBuf::from(rest);
        }
    }
    p
}

fn command_line(exe: &Path, profile: &str) -> Vec<u16> {
    let mut s: Vec<u16> = format!("\"{}\" restore \"{profile}\"", exe.display())
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
            return Err(t_args(
                "startup.error_open_key",
                &[("error", &format!("{status:?}"))],
            ));
        }
        Ok(key)
    }
}

pub fn enable(profile: &str) -> Result<String, String> {
    let exe = exe_path()?;
    let cmd = command_line(&exe, profile);
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
            return Err(t_args(
                "startup.error_write",
                &[("error", &format!("{status:?}"))],
            ));
        }
    }
    // Espelha a configuração no config.json (o registro continua a fonte real).
    let command = format!("\"{}\" restore \"{profile}\"", exe.display());
    config::set_startup(Some(StartupMirror {
        enabled: true,
        profile: Some(profile.to_string()),
        command: Some(command),
    }));
    // Se o ocultar-ícones estiver ativado, garante que o monitor também
    // inicie junto com o sistema (entrada Run própria + spawn imediato).
    if hide_icons::load_config().enabled {
        let _ = hide_icons::ensure_run_entry();
        hide_icons::spawn_daemon();
    }
    Ok(t_args(
        "startup.enable_success",
        &[("profile", profile), ("path", &exe.display().to_string())],
    ))
}

pub fn disable() -> Result<String, String> {
    unsafe {
        let key = open_run_key(KEY_SET_VALUE)?;
        let status = RegDeleteValueW(key, VALUE_NAME);
        let _ = RegCloseKey(key);
        if status.is_err() {
            if status == ERROR_FILE_NOT_FOUND {
                config::set_startup(Some(StartupMirror {
                    enabled: false,
                    profile: None,
                    command: None,
                }));
                return Ok(t("startup.already_disabled"));
            }
            return Err(t_args(
                "startup.error_remove",
                &[("error", &format!("{status:?}"))],
            ));
        }
    }
    config::set_startup(Some(StartupMirror {
        enabled: false,
        profile: None,
        command: None,
    }));
    Ok(t("startup.disabled"))
}

pub fn status() -> Result<String, String> {
    // Consulta rápida: lê o espelho no config.json (evita abrir o registro).
    if let Some(mirror) = config::get_startup() {
        if !mirror.enabled {
            return Ok(t("startup.status_disabled"));
        }
        let command = mirror.command.unwrap_or_default();
        return Ok(t_args("startup.status_enabled", &[("command", &command)]));
    }
    // Sem espelho: cai no registro e autopopula o config.json.
    unsafe {
        let key = open_run_key(KEY_QUERY_VALUE)?;
        let mut size: u32 = 0;
        let query = RegQueryValueExW(key, VALUE_NAME, None, None, None, Some(&mut size));
        if query.is_err() {
            let _ = RegCloseKey(key);
            return Ok(t("startup.status_disabled"));
        }
        let mut buf = vec![0u8; size as usize];
        let query = RegQueryValueExW(
            key,
            VALUE_NAME,
            None,
            None,
            Some(buf.as_mut_ptr()),
            Some(&mut size),
        );
        let _ = RegCloseKey(key);
        if query.is_err() {
            return Ok(t("startup.status_disabled"));
        }
        let wide = std::slice::from_raw_parts(buf.as_ptr() as *const u16, (size as usize) / 2);
        let text = String::from_utf16_lossy(wide)
            .trim_end_matches('\0')
            .to_string();
        let profile = extract_profile(&text);
        config::set_startup(Some(StartupMirror {
            enabled: true,
            profile,
            command: Some(text.clone()),
        }));
        Ok(t_args("startup.status_enabled", &[("command", &text)]))
    }
}

/// Extrai o nome do perfil de `"<exe>" restore "<perfil>"`.
fn extract_profile(command: &str) -> Option<String> {
    let marker = " restore \"";
    let start = command.find(marker)? + marker.len();
    let rest = &command[start..];
    let end = rest.find('\"')?;
    Some(rest[..end].to_string())
}
