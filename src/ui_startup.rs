// DesktopORZ — Native Rust GUI
// Copyright (C) 2026 ThainanViniciusKatchan
//
// Inicialização da interface gráfica (`DesktopORZ-GUI.exe`) junto com o
// Windows via tarefa agendada como administrador (`/RL HIGHEST`), controlada
// pela aba Idioma & Sistema.

use std::env;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Output};

use windows::core::{w, PCWSTR};
use windows::Win32::System::Registry::{
    RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER,
    KEY_QUERY_VALUE, KEY_SET_VALUE, REG_SAM_FLAGS,
};

/// Nome da tarefa agendada no Agendador de Tarefas do Windows e do valor
/// legado na chave `Run` do usuário atual.
const TASK_NAME: &str = "DesktopORZ-GUI";
const CREATE_NO_WINDOW: u32 = 0x08000000;
const RUN_KEY: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
const VALUE_NAME: PCWSTR = w!("DesktopORZ-GUI");

/// Caminho do executável da interface (`DesktopORZ-GUI.exe`), sem o prefixo
/// `\\?\` que ferramentas do Windows não aceitam.
fn exe_path() -> Result<PathBuf, String> {
    let path = env::current_exe().map_err(|e| format!("{e}"))?;
    let path = path.canonicalize().unwrap_or(path);
    let clean = match path.to_str() {
        Some(text) => match text.strip_prefix(r"\\?\") {
            Some(rest) => PathBuf::from(rest),
            None => path,
        },
        None => path,
    };

    let is_gui = clean
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.eq_ignore_ascii_case("desktoporz-gui.exe"))
        .unwrap_or(false);
    if !is_gui {
        if let Some(parent) = clean.parent() {
            let gui_exe = parent.join("DesktopORZ-GUI.exe");
            if gui_exe.is_file() {
                return Ok(gui_exe);
            }
        }
    }
    Ok(clean)
}

fn run_schtasks(args: &[&str]) -> Result<Output, String> {
    Command::new("schtasks")
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("{e}"))
}

fn schtasks_error(output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !stderr.is_empty() {
        stderr
    } else if !stdout.is_empty() {
        stdout
    } else {
        format!("schtasks falhou com código {:?}", output.status.code())
    }
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

/// Remove a entrada legada de `HKCU\...\Run` para evitar iniciar uma segunda
/// instância sem privilégios de administrador no logon.
fn remove_legacy_run_entry() {
    if let Ok(key) = open_run_key(KEY_SET_VALUE) {
        unsafe {
            let _ = RegDeleteValueW(key, VALUE_NAME);
            let _ = RegCloseKey(key);
        }
    }
}

fn legacy_run_entry_exists() -> bool {
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

/// Verifica no Agendador de Tarefas se a tarefa agendada existe.
pub fn scheduled_task_exists() -> bool {
    run_schtasks(&["/Query", "/TN", TASK_NAME])
        .map(|out| out.status.success())
        .unwrap_or(false)
}

/// Cria a tarefa agendada no logon com privilégios elevados (`/RL HIGHEST`),
/// permitindo iniciar como administrador sem solicitar confirmação do UAC.
pub fn create_scheduled_task() -> Result<(), String> {
    let exe = exe_path()?;
    let target = format!("\"{}\"", exe.display());
    let output = run_schtasks(&[
        "/Create",
        "/SC",
        "ONLOGON",
        "/TN",
        TASK_NAME,
        "/TR",
        &target,
        "/RL",
        "HIGHEST",
        "/F",
    ])?;
    if !output.status.success() {
        return Err(schtasks_error(&output));
    }
    remove_legacy_run_entry();
    crate::config::set_scheduled_task(true);
    Ok(())
}

/// Remove a tarefa agendada do Agendador de Tarefas (idempotente).
pub fn delete_scheduled_task() -> Result<(), String> {
    if scheduled_task_exists() {
        let output = run_schtasks(&["/Delete", "/TN", TASK_NAME, "/F"])?;
        if !output.status.success() && scheduled_task_exists() {
            return Err(schtasks_error(&output));
        }
    }
    crate::config::set_scheduled_task(false);
    Ok(())
}

/// Executa a tarefa agendada para abrir a interface já elevada sem UAC.
pub fn run_scheduled_task() -> bool {
    if !crate::config::get_scheduled_task() && !scheduled_task_exists() {
        return false;
    }
    run_schtasks(&["/Run", "/TN", TASK_NAME])
        .map(|out| out.status.success())
        .unwrap_or(false)
}

/// Na inicialização do software, se a opção de iniciar com o sistema estiver
/// ativada e a tarefa agendada ainda não existir, cria o agendamento.
pub fn ensure_scheduled_task_on_startup() {
    let cfg = crate::config::load_or_create_config();
    let startup_enabled = cfg.ui_startup == Some(true)
        || cfg.startup.as_ref().map(|s| s.enabled).unwrap_or(false)
        || legacy_run_entry_exists();
    if startup_enabled && (!cfg.scheduled_task || !scheduled_task_exists()) {
        if create_scheduled_task().is_ok() {
            crate::config::set_ui_startup(true);
            crate::config::set_scheduled_task(true);
        }
    }
}

/// Cria a tarefa agendada que abre a interface como administrador no logon.
pub fn enable() -> Result<(), String> {
    create_scheduled_task()?;
    crate::config::set_ui_startup(true);
    crate::config::set_scheduled_task(true);
    Ok(())
}

/// Remove a tarefa agendada e qualquer entrada legada (idempotente).
pub fn disable() -> Result<(), String> {
    remove_legacy_run_entry();
    delete_scheduled_task()?;
    crate::config::set_ui_startup(false);
    crate::config::set_scheduled_task(false);
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

/// Estado da inicialização da interface com o sistema.
pub fn is_enabled() -> bool {
    let cfg = crate::config::load_or_create_config();
    if let Some(enabled) = cfg.ui_startup {
        return enabled || cfg.scheduled_task;
    }
    scheduled_task_exists() || legacy_run_entry_exists()
}
