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
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::config::{self, HideIconsMirror};
use crate::i18n::{t, t_args};
use crate::shell_locator;

use windows::core::w;
use windows::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW,
    RegSetValueExW, HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_CREATE_KEY_DISPOSITION,
    REG_DWORD, REG_OPTION_NON_VOLATILE, REG_SAM_FLAGS, REG_SZ,
};
use windows::Win32::Foundation::POINT;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON, VK_MBUTTON, VK_RBUTTON};
use windows::Win32::UI::WindowsAndMessaging::{GetCursorPos, IsWindowVisible, ShowWindow, SW_HIDE, SW_SHOW};

const APP_KEY: windows::core::PCWSTR = w!("Software\\DesktopORZ");
const RUN_KEY: windows::core::PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
const VALUE_ENABLED: windows::core::PCWSTR = w!("HideIconsEnabled");
const VALUE_TIMEOUT: windows::core::PCWSTR = w!("HideIconsTimeout");
const RUN_VALUE_NAME: windows::core::PCWSTR = w!("DesktopORZ-HideIcons");

/// Intervalo de varredura do loop: 500ms mantém a CPU praticamente em zero
/// e ainda assim reage rápido a atividade/inatividade.
const POLL_INTERVAL: Duration = Duration::from_millis(500);
/// Timeout padrão quando `on` é chamado sem argumento de segundos.
const DEFAULT_TIMEOUT_SECS: u64 = 5;

#[derive(Default)]
pub struct HideIconsConfig {
    pub enabled: bool,
    pub timeout_secs: u64,
}

fn exe_path() -> Result<PathBuf, String> {
    let p = env::current_exe()
        .map(|p| p.canonicalize().unwrap_or(p))
        .map_err(|e| format!("{e}"))?;
    // Remove o prefixo estendido `\\?\` que o gerenciador da chave Run não entende.
    if let Some(s) = p.to_str() {
        if let Some(rest) = s.strip_prefix("\\\\?\\") {
            return Ok(PathBuf::from(rest));
        }
    }
    Ok(p)
}

fn open_key(root: HKEY, subkey: windows::core::PCWSTR, access: REG_SAM_FLAGS) -> Result<HKEY, String> {
    unsafe {
        let mut key = HKEY::default();
        let status = RegOpenKeyExW(root, subkey, 0, access, &mut key);
        if status.is_err() {
            return Err(t_args(
                "hide_icons.error_open_key",
                &[("error", &format!("{status:?}"))],
            ));
        }
        Ok(key)
    }
}

fn open_app_key(access: REG_SAM_FLAGS) -> Result<HKEY, String> {
    if access == KEY_SET_VALUE {
        unsafe {
            let mut key = HKEY::default();
            let mut disposition = REG_CREATE_KEY_DISPOSITION(0);
            let status = RegCreateKeyExW(
                HKEY_CURRENT_USER,
                APP_KEY,
                0,
                windows::core::PCWSTR::null(),
                REG_OPTION_NON_VOLATILE,
                KEY_SET_VALUE,
                None,
                &mut key,
                Some(&mut disposition as *mut _),
            );
            if status.is_err() {
                return Err(t_args(
                    "hide_icons.error_create_key",
                    &[("error", &format!("{status:?}"))],
                ));
            }
            return Ok(key);
        }
    }
    open_key(HKEY_CURRENT_USER, APP_KEY, access)
}

fn set_dword(key: HKEY, name: windows::core::PCWSTR, value: u32) -> Result<(), String> {
    unsafe {
        let status = RegSetValueExW(
            key,
            name,
            0,
            REG_DWORD,
            Some(std::slice::from_raw_parts(
                &value as *const u32 as *const u8,
                std::mem::size_of::<u32>(),
            )),
        );
        if status.is_err() {
            return Err(t_args(
                "hide_icons.error_write",
                &[("error", &format!("{status:?}"))],
            ));
        }
    }
    Ok(())
}

fn set_sz(key: HKEY, name: windows::core::PCWSTR, value: &str) -> Result<(), String> {
    let mut data: Vec<u16> = value.encode_utf16().collect();
    data.push(0);
    unsafe {
        let status = RegSetValueExW(
            key,
            name,
            0,
            REG_SZ,
            Some(std::slice::from_raw_parts(
                data.as_ptr() as *const u8,
                data.len() * 2,
            )),
        );
        if status.is_err() {
            return Err(t_args(
                "hide_icons.error_write",
                &[("error", &format!("{status:?}"))],
            ));
        }
    }
    Ok(())
}

fn delete_value(key: HKEY, name: windows::core::PCWSTR) {
    unsafe {
        let _ = RegDeleteValueW(key, name);
    }
}

fn query_dword(key: HKEY, name: windows::core::PCWSTR) -> Option<u32> {
    unsafe {
        let mut value: u32 = 0;
        let mut size = std::mem::size_of::<u32>() as u32;
        let status = RegQueryValueExW(
            key,
            name,
            None,
            None,
            Some(&mut value as *mut u32 as *mut u8),
            Some(&mut size),
        );
        if status.is_err() {
            None
        } else {
            Some(value)
        }
    }
}

/// Caminho do binário monitor (`DesktopORZ-HideIcons.exe`), compilado no
/// subsistema Windows (sem janela de console), na mesma pasta deste exe.
fn daemon_path() -> Result<PathBuf, String> {
    Ok(exe_path()?
        .parent()
        .ok_or_else(|| "Could not determine the executable's folder.".to_string())?
        .join("DesktopORZ-HideIcons.exe"))
}

pub const DAEMON_EXE_NAME: &str = "DesktopORZ-HideIcons.exe";

fn register_run_entry() -> Result<(), String> {
    let exe = daemon_path()?;
    let cmd = format!("\"{}\"", exe.display());
    unsafe {
        let key = open_key(HKEY_CURRENT_USER, RUN_KEY, KEY_SET_VALUE)?;
        let result = set_sz(key, RUN_VALUE_NAME, &cmd);
        let _ = RegCloseKey(key);
        result
    }
}

fn remove_run_entry() {
    if let Ok(key) = open_key(HKEY_CURRENT_USER, RUN_KEY, KEY_SET_VALUE) {
        delete_value(key, RUN_VALUE_NAME);
        unsafe {
            let _ = RegCloseKey(key);
        }
    }
}

pub fn load_config() -> HideIconsConfig {
    // Consulta rápida: lê o espelho no config.json (evita abrir o registro).
    if let Some(mirror) = config::get_hide_icons() {
        return HideIconsConfig {
            enabled: mirror.enabled,
            timeout_secs: mirror.timeout_secs.unwrap_or(DEFAULT_TIMEOUT_SECS),
        };
    }
    // Sem espelho: cai no registro e autopopula o config.json.
    let mut config = HideIconsConfig {
        timeout_secs: DEFAULT_TIMEOUT_SECS,
        ..Default::default()
    };
    if let Ok(key) = open_app_key(KEY_QUERY_VALUE) {
        config.enabled = query_dword(key, VALUE_ENABLED).map(|v| v != 0).unwrap_or(false);
        if let Some(v) = query_dword(key, VALUE_TIMEOUT) {
            if v > 0 {
                config.timeout_secs = v as u64;
            }
        }
        unsafe {
            let _ = RegCloseKey(key);
        }

        config::set_hide_icons(Some(HideIconsMirror {
            enabled: config.enabled,
            timeout_secs: Some(config.timeout_secs),
        }));
    }
    config
}

/// Regrava a entrada Run do daemon (idempotente). Chamada pelo `enable`
/// e também pelo `startup on`, para que o monitor acompanhe a inicialização
/// do CLI quando estiver ativado.
pub fn ensure_run_entry() -> Result<(), String> {
    register_run_entry()
}

/// Inicia o monitor imediatamente (detached), sem esperar o próximo login.
/// Não faz nada se uma instância já estiver em execução.
pub fn spawn_daemon() {
    if crate::process_watcher::is_process_running(DAEMON_EXE_NAME) {
        return;
    }
    if let Ok(daemon) = daemon_path() {
        if daemon.exists() {
            let _ = std::process::Command::new(daemon)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn();
        }
    }
}

pub fn enable(timeout_secs: u64) -> Result<String, String> {
    if timeout_secs == 0 {
        return Err(t("hide_icons.timeout_zero"));
    }
    let key = open_app_key(KEY_SET_VALUE)?;
    let result = set_dword(key, VALUE_ENABLED, 1)
        .and_then(|_| set_dword(key, VALUE_TIMEOUT, timeout_secs as u32));
    unsafe {
        let _ = RegCloseKey(key);
    }
    result?;
    register_run_entry()?;
    // Espelha a configuração no config.json (o registro continua a fonte real).
    config::set_hide_icons(Some(HideIconsMirror {
        enabled: true,
        timeout_secs: Some(timeout_secs),
    }));
    spawn_daemon();
    Ok(t_args(
        "hide_icons.enable_success",
        &[("seconds", &timeout_secs.to_string())],
    ))
}

pub fn disable() -> Result<String, String> {
    let key = open_app_key(KEY_SET_VALUE)?;
    delete_value(key, VALUE_ENABLED);
    delete_value(key, VALUE_TIMEOUT);
    unsafe {
        let _ = RegCloseKey(key);
    }
    remove_run_entry();
    config::set_hide_icons(Some(HideIconsMirror {
        enabled: false,
        timeout_secs: None,
    }));
    Ok(t("hide_icons.disabled"))
}

pub fn status() -> Result<String, String> {
    let config = load_config();
    if !config.enabled {
        return Ok(t("hide_icons.status_disabled"));
    }
    Ok(t_args(
        "hide_icons.status_enabled",
        &[("seconds", &config.timeout_secs.to_string())],
    ))
}

/// Lê apenas a flag `HideIconsEnabled` direto do registro. Usada pelo loop
/// `run` a cada ciclo para responder ao comando `off` em tempo real.
fn is_enabled_in_registry() -> bool {
    if let Ok(key) = open_app_key(KEY_QUERY_VALUE) {
        let enabled = query_dword(key, VALUE_ENABLED).map(|v| v != 0).unwrap_or(false);
        unsafe {
            let _ = RegCloseKey(key);
        }
        return enabled;
    }
    false
}

/// Detecta atividade vinda **somente do mouse** desde o ciclo anterior:
/// movimento do cursor (`GetCursorPos`) ou clique pressionado agora
/// (`GetAsyncKeyState` nos botões esquerdo/direito/meio — cobre cliques com
/// o cursor parado). Teclado é ignorado de propósito.
fn mouse_activity(prev_pos: &mut POINT) -> bool {
    unsafe {
        let mut pos = POINT::default();
        let moved = GetCursorPos(&mut pos).is_ok() && pos != *prev_pos;
        *prev_pos = pos;

        let clicked = [VK_LBUTTON, VK_RBUTTON, VK_MBUTTON]
            .iter()
            .any(|&vk| (GetAsyncKeyState(vk.0 as i32) as u16) & 0x8000 != 0);

        moved || clicked
    }
}

/// Altera a visibilidade dos ícones apenas quando o estado realmente muda,
/// evitando chamadas redundantes à API.
fn set_icons_visible(visible: bool) {
    if let Ok(listview) = shell_locator::find_desktop_listview() {
        unsafe {
            let currently_visible = IsWindowVisible(listview).as_bool();
            if currently_visible != visible {
                let _ = ShowWindow(listview, if visible { SW_SHOW } else { SW_HIDE });
            }
        }
    }
}

/// Loop de monitoramento (comando interno `hide-icons run`).
///
/// Estratégia: a cada varredura (~500ms) verifica atividade **exclusiva do
/// mouse** (movimento do cursor ou clique). Um `Instant` local mede há
/// quanto tempo não há atividade. Teclado não interfere.
pub fn run() -> Result<String, String> {
    let config = load_config();
    if !config.enabled {
        return Err(t("hide_icons.disabled_error"));
    }

    let timeout = Duration::from_secs(config.timeout_secs);
    let mut icons_hidden = false;
    let mut last_pos = POINT::default();
    // Começa contando a ociosidade de agora (instante do login/execução).
    let mut idle_since = Instant::now();

    loop {
        std::thread::sleep(POLL_INTERVAL);

        // Se foi desativado pelo comando `off`, reexibe os ícones e encerra.
        if !is_enabled_in_registry() {
            if icons_hidden {
                set_icons_visible(true);
            }
            return Ok(t("hide_icons.run_stopped"));
        }

        if mouse_activity(&mut last_pos) {
            // Houve atividade de mouse: zera o contador e reexibe os ícones.
            idle_since = Instant::now();
            if icons_hidden {
                set_icons_visible(true);
                icons_hidden = false;
            }
        } else if !icons_hidden && idle_since.elapsed() >= timeout {
            set_icons_visible(false);
            icons_hidden = true;
        }
    }
}
