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
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use crate::config::{self, HideIconsMirror};
use crate::i18n::{t, t_args};
use crate::shell_locator;

use windows::core::w;
use windows::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW,
    RegSetValueExW, HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_CREATE_KEY_DISPOSITION,
    REG_DWORD, REG_OPTION_NON_VOLATILE, REG_SAM_FLAGS, REG_SZ,
};
use windows::Win32::Foundation::{HMODULE, HWND, LPARAM, LRESULT, RECT, WPARAM, BOOL};
use windows::Win32::System::LibraryLoader::{
    GetModuleHandleExW, GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS,
    GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
};
use windows::Win32::Foundation::WAIT_EVENT;
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, EnumWindows, FindWindowW, GetClassNameW, GetSystemMetrics,
    GetWindowRect, GetWindowThreadProcessId, IsWindowVisible, IsZoomed,
    MsgWaitForMultipleObjectsEx, PeekMessageW, SendNotifyMessageW, SetWindowPos, ShowWindow,
    SystemParametersInfoW, TranslateMessage, UnhookWindowsHookEx, HHOOK, HWND_BROADCAST, MSG,
    MWMO_INPUTAVAILABLE, PM_REMOVE, QS_ALLINPUT, SM_CXSCREEN, SM_CYSCREEN, SPIF_SENDCHANGE,
    SPI_GETWORKAREA, SPI_SETWORKAREA, SWP_NOACTIVATE, SWP_NOZORDER, SW_HIDE, SW_SHOW,
    SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, WH_KEYBOARD_LL, WH_MOUSE_LL, WM_DISPLAYCHANGE,
    WM_SETTINGCHANGE, SetWindowsHookExW,
};
use windows::Win32::Graphics::Gdi::{EnumDisplaySettingsW, DEVMODEW, ENUM_CURRENT_SETTINGS};

const APP_KEY: windows::core::PCWSTR = w!("Software\\DesktopORZ");
const RUN_KEY: windows::core::PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
const VALUE_ENABLED: windows::core::PCWSTR = w!("HideIconsEnabled");
const VALUE_TIMEOUT: windows::core::PCWSTR = w!("HideIconsTimeout");
const VALUE_INPUTS: windows::core::PCWSTR = w!("HideIconsInputs");
const VALUE_TASKBAR: windows::core::PCWSTR = w!("HideIconsTaskbar");
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
    pub inputs: InputSource,
    /// Quando ativo, a barra de tarefas também é ocultada junto com os ícones
    /// e a área útil da tela é expandida para 100% (workarea).
    pub include_taskbar: bool,
}

/// Lê a flag `-include-taskbar`/`--include-taskbar` dos argumentos recebidos
/// pelo daemon na linha de comando (entrada Run ou spawn do CLI).
pub fn include_taskbar_arg(args: &[String]) -> bool {
    args.iter()
        .any(|a| a == "-include-taskbar" || a == "--include-taskbar")
}

/// Fonte(s) de atividade que restauram os ícones e zeram o contador de
/// inatividade: mouse, teclado ou ambos (padrão).
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum InputSource {
    Mouse,
    Keyboard,
    #[default]
    Both,
}

impl InputSource {
    const BIT_MOUSE: u32 = 1;
    const BIT_KEYBOARD: u32 = 2;

    /// Regra de fallback da CLI: nenhuma flag ou ambas → `Both`; apenas uma
    /// → fonte exclusiva.
    pub fn from_flags(keyboard: bool, mouse: bool) -> Self {
        match (keyboard, mouse) {
            (true, false) => Self::Keyboard,
            (false, true) => Self::Mouse,
            _ => Self::Both,
        }
    }

    /// Lê as flags `--kb`/`--keyboard` e `--mou`/`--mouse` dos argumentos
    /// recebidos pelo daemon na linha de comando.
    pub fn from_args(args: &[String]) -> Self {
        let keyboard = args.iter().any(|a| a == "--kb" || a == "--keyboard");
        let mouse = args.iter().any(|a| a == "--mou" || a == "--mouse");
        Self::from_flags(keyboard, mouse)
    }

    pub fn from_bits(bits: u32) -> Self {
        Self::from_flags(
            bits & Self::BIT_KEYBOARD != 0,
            bits & Self::BIT_MOUSE != 0,
        )
    }

    pub fn bits(self) -> u32 {
        let mut bits = 0;
        if self.watches_mouse() {
            bits |= Self::BIT_MOUSE;
        }
        if self.watches_keyboard() {
            bits |= Self::BIT_KEYBOARD;
        }
        bits
    }

    pub fn watches_mouse(self) -> bool {
        matches!(self, Self::Mouse | Self::Both)
    }

    pub fn watches_keyboard(self) -> bool {
        matches!(self, Self::Keyboard | Self::Both)
    }

    /// Texto das flags a repassar na linha de comando do daemon desacoplado.
    pub fn flags(self) -> &'static str {
        match self {
            Self::Mouse => "--mou",
            Self::Keyboard => "--kb",
            Self::Both => "--kb --mou",
        }
    }

    /// Chave i18n que nomeia a fonte ativa (usada pelo `status`).
    pub fn i18n_key(self) -> &'static str {
        match self {
            Self::Mouse => "hide_icons.source_mouse",
            Self::Keyboard => "hide_icons.source_keyboard",
            Self::Both => "hide_icons.source_both",
        }
    }
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

fn register_run_entry(inputs: InputSource, include_taskbar: bool) -> Result<(), String> {
    let exe = daemon_path()?;
    let cmd = format!(
        "\"{}\" {}{}",
        exe.display(),
        inputs.flags(),
        if include_taskbar {
            " --include-taskbar"
        } else {
            ""
        }
    );
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
            inputs: mirror.inputs.map(InputSource::from_bits).unwrap_or_default(),
            include_taskbar: mirror.include_taskbar.unwrap_or(false),
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
        if let Some(v) = query_dword(key, VALUE_INPUTS) {
            config.inputs = InputSource::from_bits(v);
        }
        config.include_taskbar = query_dword(key, VALUE_TASKBAR).map(|v| v != 0).unwrap_or(false);
        unsafe {
            let _ = RegCloseKey(key);
        }

        config::set_hide_icons(Some(HideIconsMirror {
            enabled: config.enabled,
            timeout_secs: Some(config.timeout_secs),
            inputs: Some(config.inputs.bits()),
            include_taskbar: Some(config.include_taskbar),
        }));
    }
    config
}

/// Regrava a entrada Run do daemon (idempotente). Chamada pelo `enable`
/// e também pelo `startup on`, para que o monitor acompanhe a inicialização
/// do CLI quando estiver ativado.
pub fn ensure_run_entry() -> Result<(), String> {
    let config = load_config();
    register_run_entry(config.inputs, config.include_taskbar)
}

/// Inicia o monitor imediatamente (detached), sem esperar o próximo login.
/// Não faz nada se uma instância já estiver em execução.
pub fn spawn_daemon() {
    if crate::process_watcher::is_process_running(DAEMON_EXE_NAME) {
        return;
    }
    if let Ok(daemon) = daemon_path() {
        if daemon.exists() {
            let config = load_config();
            let mut daemon_args: Vec<String> =
                config.inputs.flags().split(' ').map(String::from).collect();
            if config.include_taskbar {
                daemon_args.push("--include-taskbar".to_string());
            }
            let _ = std::process::Command::new(daemon)
                .args(daemon_args)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn();
        }
    }
}

pub fn enable(
    timeout_secs: u64,
    inputs: InputSource,
    include_taskbar: bool,
) -> Result<String, String> {
    if timeout_secs == 0 {
        return Err(t("hide_icons.timeout_zero"));
    }
    let key = open_app_key(KEY_SET_VALUE)?;
    let result = set_dword(key, VALUE_ENABLED, 1)
        .and_then(|_| set_dword(key, VALUE_TIMEOUT, timeout_secs as u32))
        .and_then(|_| set_dword(key, VALUE_INPUTS, inputs.bits()))
        .and_then(|_| set_dword(key, VALUE_TASKBAR, include_taskbar as u32));
    unsafe {
        let _ = RegCloseKey(key);
    }
    result?;
    register_run_entry(inputs, include_taskbar)?;
    // Espelha a configuração no config.json (o registro continua a fonte real).
    config::set_hide_icons(Some(HideIconsMirror {
        enabled: true,
        timeout_secs: Some(timeout_secs),
        inputs: Some(inputs.bits()),
        include_taskbar: Some(include_taskbar),
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
    delete_value(key, VALUE_INPUTS);
    delete_value(key, VALUE_TASKBAR);
    unsafe {
        let _ = RegCloseKey(key);
    }
    remove_run_entry();
    config::set_hide_icons(Some(HideIconsMirror {
        enabled: false,
        timeout_secs: None,
        inputs: None,
        include_taskbar: None,
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
        &[
            ("seconds", &config.timeout_secs.to_string()),
            ("source", &t(config.inputs.i18n_key())),
            (
                "taskbar",
                &if config.include_taskbar {
                    t("hide_icons.taskbar_included")
                } else {
                    String::new()
                },
            ),
        ],
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

/// Estado da barra de tarefas para o loop de monitoramento: guarda se está
/// oculta e o `RECT` original da área de trabalho (para restauração exata).
#[derive(Default)]
pub struct TaskbarState {
    hidden: bool,
    original_work_area: Option<RECT>,
}

/// Localiza as barras de tarefas: `Shell_TrayWnd` (principal) e
/// `Shell_SecondaryTrayWnd` (monitores secundários, quando existem).
fn taskbar_hwnds() -> Vec<HWND> {
    let mut hwnds = Vec::new();
    for class in [w!("Shell_TrayWnd"), w!("Shell_SecondaryTrayWnd")] {
        if let Ok(hwnd) = unsafe { FindWindowW(class, None) } {
            if hwnd != HWND::default() && !hwnds.contains(&hwnd) {
                hwnds.push(hwnd);
            }
        }
    }
    hwnds
}

/// Resolução física do monitor primário via `EnumDisplaySettingsW`.
/// Diferente de `GetSystemMetrics(SM_CXSCREEN/SM_CYSCREEN)`, não é afetada
/// pela virtualização de DPI em processos não-DPI-aware (como este daemon),
/// então devolve os valores reais em pixels da tela (ex.: 1920x1080 mesmo
/// com escala de 125%/150%).
fn physical_screen_size() -> (i32, i32) {
    unsafe {
        let mut devmode = DEVMODEW::default();
        devmode.dmSize = std::mem::size_of::<DEVMODEW>() as u16;
        if EnumDisplaySettingsW(None, ENUM_CURRENT_SETTINGS, &mut devmode).as_bool() {
            (devmode.dmPelsWidth as i32, devmode.dmPelsHeight as i32)
        } else {
            // Fallback: métricas do sistema (podem estar virtualizadas por DPI).
            (
                GetSystemMetrics(SM_CXSCREEN),
                GetSystemMetrics(SM_CYSCREEN),
            )
        }
    }
}

/// Broadcast explícito de WM_SETTINGCHANGE (SPI_SETWORKAREA) + WM_DISPLAYCHANGE:
/// complementa o SPIF_SENDCHANGE e notifica janelas maximizadas que precisam se
/// reajustar à nova área útil (algumas ignoram a notificação interna do SPI; o
/// WM_DISPLAYCHANGE força o DefWindowProc a recalcular o layout).
fn broadcast_workarea_change() {
    let (width, height) = physical_screen_size();
    unsafe {
        let _ = SendNotifyMessageW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            WPARAM(SPI_SETWORKAREA.0 as usize),
            LPARAM(0),
        );
        // WM_DISPLAYCHANGE com a resolução física (lo word = largura, hi = altura).
        let dims = (((height & 0xffff) as isize) << 16) | (width & 0xffff) as isize;
        let _ = SendNotifyMessageW(HWND_BROADCAST, WM_DISPLAYCHANGE, WPARAM(32), LPARAM(dims));
    }
}

/// Oculta a barra de tarefas e expande a área útil para 100% da tela:
/// 1. Salva o `RECT` original da workarea (SPI_GETWORKAREA) na primeira vez.
/// 2. Esconde a(s) barra(s) com SW_HIDE ANTES de mexer na workarea: se a
///    workarea fosse aplicada antes, o shell poderia recalculá-la quando a
///    barra desaparecesse, sobrescrevendo o retângulo de tela cheia.
/// 3. Monta `full_screen_rect` com a resolução física atual.
/// 4. Chama SPI_SETWORKAREA com SPIF_SENDCHANGE + broadcast explícito.
/// 5. Redimensiona via SetWindowPos as janelas maximizadas que ignoram o
///    broadcast (o Explorer/CabinetWClass), eliminando o "buraco" de onde
///    a barra de tarefas saiu.
fn hide_taskbar(state: &mut TaskbarState) {
    if state.hidden {
        return;
    }
    let (width, height) = physical_screen_size();
    let full_screen_rect = RECT {
        left: 0,
        top: 0,
        right: width,
        bottom: height,
    };
    unsafe {
        if state.original_work_area.is_none() {
            let mut rect = RECT::default();
            if SystemParametersInfoW(
                SPI_GETWORKAREA,
                0,
                Some(&mut rect as *mut RECT as *mut _),
                SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
            )
            .is_ok()
            {
                state.original_work_area = Some(rect);
            }
        }
        for hwnd in taskbar_hwnds() {
            let _ = ShowWindow(hwnd, SW_HIDE);
        }
        daemon_log(&format!(
            "workarea: expandindo {:?} -> {:?}",
            state.original_work_area, full_screen_rect
        ));
        if let Err(e) = SystemParametersInfoW(
            SPI_SETWORKAREA,
            0,
            Some(&full_screen_rect as *const RECT as *mut _),
            SPIF_SENDCHANGE,
        ) {
            daemon_log(&format!("SPI_SETWORKAREA falhou ao ocultar: {e}"));
        }
    }
    broadcast_workarea_change();
    resize_zoomed_windows(&full_screen_rect);
    state.hidden = true;
}

/// Restaura a barra de tarefas: reexibe a(s) barra(s) com SW_SHOW ANTES de
/// reaplicar o `RECT` original (SPI_SETWORKAREA). A ordem espelha a ocultação:
/// ao reaplicar a workarea com a barra já visível, o shell registra a barra de
/// volta como appbar e as janelas maximizadas reduzem para abrir espaço.
/// Depois, `resize_zoomed_windows` devolve ao retângulo original as janelas
/// que foram expandidas manualmente na ocultação (elas podem não encolher
/// sozinhas só com o broadcast).
fn restore_taskbar(state: &mut TaskbarState) {
    if !state.hidden {
        return;
    }
    let restore_rect = state.original_work_area;
    unsafe {
        let hwnds = taskbar_hwnds();
        for hwnd in &hwnds {
            let before = IsWindowVisible(*hwnd).as_bool();
            let _ = ShowWindow(*hwnd, SW_SHOW);
            let after = IsWindowVisible(*hwnd).as_bool();
            daemon_log(&format!(
                "taskbar {hwnd:?}: visivel={before} -> {after}"
            ));
        }
        if hwnds.is_empty() {
            daemon_log("nenhuma barra de tarefas encontrada para restaurar");
        }
        if let Some(rect) = restore_rect {
            daemon_log(&format!("workarea: restaurando {:?}", rect));
            if let Err(e) = SystemParametersInfoW(
                SPI_SETWORKAREA,
                0,
                Some(&rect as *const RECT as *mut _),
                SPIF_SENDCHANGE,
            ) {
                daemon_log(&format!("SPI_SETWORKAREA falhou ao restaurar: {e}"));
            }
        }
    }
    broadcast_workarea_change();
    if let Some(rect) = restore_rect {
        resize_zoomed_windows(&rect);
    }
    state.original_work_area = None;
    state.hidden = false;
}

/// Contexto passado ao callback de `resize_zoomed_windows` via `lparam`.
struct ZoomedFixContext {
    target: RECT,
    own_pid: u32,
}

/// Callback do `EnumWindows`: redimensiona janelas top-level maximizadas cujo
/// retângulo atual diverge do alvo (a nova workarea). Janelas do shell e do
/// próprio processo são ignoradas; janelas já no tamanho certo não são tocadas
/// (evita flicker). Cada ajuste é registrado no log do daemon para diagnóstico.
unsafe extern "system" fn zoomed_window_enum(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let ctx = &*(lparam.0 as *const ZoomedFixContext);
    if !IsWindowVisible(hwnd).as_bool() || !IsZoomed(hwnd).as_bool() {
        return BOOL(1);
    }
    let mut pid: u32 = 0;
    GetWindowThreadProcessId(hwnd, Some(&mut pid));
    if pid == ctx.own_pid {
        return BOOL(1);
    }
    let mut class_buf = [0u16; 64];
    let class_len = GetClassNameW(hwnd, &mut class_buf);
    let class = String::from_utf16_lossy(&class_buf[..class_len as usize]);
    match class.as_str() {
        "Shell_TrayWnd" | "Shell_SecondaryTrayWnd" | "Progman" | "WorkerW" => return BOOL(1),
        _ => {}
    }
    let mut rect = RECT::default();
    if GetWindowRect(hwnd, &mut rect).is_ok() && rect != ctx.target {
        daemon_log(&format!(
            "zoomed {hwnd:?} ({class}): {:?} -> {:?}",
            rect, ctx.target
        ));
        // hwndinsert_after é ignorado com SWP_NOZORDER; HWND::default() = NULL.
        let _ = SetWindowPos(
            hwnd,
            HWND::default(),
            ctx.target.left,
            ctx.target.top,
            ctx.target.right - ctx.target.left,
            ctx.target.bottom - ctx.target.top,
            SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }
    BOOL(1)
}

/// Força as janelas maximizadas a obedecerem `target` (a workarea): nem toda
/// janela maximizada reage ao broadcast de WM_SETTINGCHANGE/WM_DISPLAYCHANGE —
/// o Explorer (CabinetWClass), por exemplo, ignora a notificação e deixa um
/// "buraco" onde a barra de tarefas ficava. Esta passada é a rede de segurança
/// que ajusta essas janelas via SetWindowPos (mantendo-as maximizadas).
fn resize_zoomed_windows(target: &RECT) {
    let ctx = ZoomedFixContext {
        target: *target,
        own_pid: std::process::id(),
    };
    unsafe {
        let _ = EnumWindows(Some(zoomed_window_enum), LPARAM(&ctx as *const _ as isize));
    }
}

/// Timestamp (em ms) da última atividade detectada pelos hooks. Atômico
/// porque é gravado pelos callbacks e lido pelo loop de monitoramento.
static LAST_ACTIVITY_MS: AtomicU64 = AtomicU64::new(0);

/// Relógio monotônico em ms, ancorado em `Instant` no primeiro uso: evita
/// depender do relógio de parede (que pode retroceder com ajustes de hora).
fn now_ms() -> u64 {
    use std::sync::OnceLock;
    use std::time::Instant;
    static EPOCH: OnceLock<(Instant, u64)> = OnceLock::new();
    let (instant, wall_ms) = EPOCH.get_or_init(|| {
        let wall = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        (Instant::now(), wall)
    });
    wall_ms + instant.elapsed().as_millis() as u64
}

/// Callback comum aos hooks `WH_MOUSE_LL` e `WH_KEYBOARD_LL`: apenas rearma
/// o timestamp de atividade e passa o evento adiante (nunca bloqueia input).
unsafe extern "system" fn input_hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        LAST_ACTIVITY_MS.store(now_ms(), Ordering::Relaxed);
    }
    CallNextHookEx(None, code, wparam, lparam)
}

/// Logger mínimo do daemon: como ele roda sem console (subsistema Windows),
/// erros e transições são registrados em `hide-icons-daemon.log` ao lado
/// do executável, para diagnóstico. Falhas de escrita são ignoradas.
fn daemon_log(message: &str) {
    use std::io::Write;
    if let Ok(path) = exe_path() {
        let log = path
            .parent()
            .map(|p| p.join("hide-icons-daemon.log"))
            .unwrap_or_else(|| PathBuf::from("hide-icons-daemon.log"));
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log)
        {
            use std::time::SystemTime;
            let secs = SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let _ = writeln!(file, "[{secs}] {message}");
        }
    }
}

/// Handle do módulo atual (exe) ancorado no endereço do callback: hooks LL
/// globais exigem um módulo válido contendo o procedimento em alguns
/// cenários; passar `None` pode falhar silenciosamente.
fn own_module_handle() -> HMODULE {
    unsafe {
        let mut hmod = HMODULE::default();
        let _ = GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
            windows::core::PCWSTR(input_hook as *const () as usize as *const u16),
            &mut hmod,
        );
        hmod
    }
}

fn install_hook(id: windows::Win32::UI::WindowsAndMessaging::WINDOWS_HOOK_ID) -> Result<HHOOK, String> {
    let hmod = own_module_handle();
    unsafe { SetWindowsHookExW(id, Some(input_hook), hmod, 0) }.map_err(|e| format!("{e}"))
}

/// Loop de monitoramento (comando interno `hide-icons run`).
///
/// Estratégia: instala hooks globais de baixo nível (`WH_MOUSE_LL` e/ou
/// `WH_KEYBOARD_LL`) conforme `source`. Os callbacks rearmam o timestamp de
/// atividade; o thread mantém um mini loop de mensagens (`PeekMessageW`,
/// obrigatório para o sistema entregar os hooks) e, a cada ~500ms, mede a
/// ociosidade e alterna a visibilidade dos ícones.
pub fn run(source: InputSource, include_taskbar: bool) -> Result<String, String> {
    let config = load_config();
    if !config.enabled {
        return Err(t("hide_icons.disabled_error"));
    }

    let timeout = Duration::from_secs(config.timeout_secs);
    let mut icons_hidden = false;
    let mut taskbar = TaskbarState::default();
    // Começa contando a ociosidade de agora (instante do login/execução).
    LAST_ACTIVITY_MS.store(now_ms(), Ordering::Relaxed);

    daemon_log(&format!(
        "iniciado: timeout={}s, fontes={} (bits={}), taskbar={}",
        config.timeout_secs,
        source.flags(),
        source.bits(),
        include_taskbar
    ));

    let mouse_hook = match source.watches_mouse() {
        true => match install_hook(WH_MOUSE_LL) {
            Ok(h) => Some(h),
            Err(e) => {
                daemon_log(&format!("falha ao instalar WH_MOUSE_LL: {e}"));
                return Err(e);
            }
        },
        false => None,
    };
    let keyboard_hook = match source.watches_keyboard() {
        true => match install_hook(WH_KEYBOARD_LL) {
            Ok(h) => Some(h),
            Err(e) => {
                daemon_log(&format!("falha ao instalar WH_KEYBOARD_LL: {e}"));
                // Não deixa o hook de mouse órfão se o de teclado falhar.
                if let Some(h) = mouse_hook {
                    unsafe {
                        let _ = UnhookWindowsHookEx(h);
                    }
                }
                return Err(e);
            }
        },
        false => None,
    };
    daemon_log("hooks instalados com sucesso");

    let result = monitor_loop(timeout, &mut icons_hidden, &mut taskbar, include_taskbar);
    daemon_log(&format!("encerrando: {result:?}"));

    // Libera os hooks antes de retornar (sucesso ou erro).
    for hook in [mouse_hook, keyboard_hook].into_iter().flatten() {
        unsafe {
            let _ = UnhookWindowsHookEx(hook);
        }
    }
    result
}

fn monitor_loop(
    timeout: Duration,
    icons_hidden: &mut bool,
    taskbar: &mut TaskbarState,
    include_taskbar: bool,
) -> Result<String, String> {
    let mut msg = MSG::default();
    // Momento da última checagem de ociosidade: garante a cadência de
    // ~POLL_INTERVAL mesmo quando o loop acorda várias vezes seguidas por
    // eventos de input, evitando varreduras no registro a cada mensagem.
    let mut last_check_ms = 0u64;
    loop {
        // Espera bloqueante: o thread fica suspenso (CPU ~0%) mas acorda
        // IMEDIATAMENTE quando o sistema entrega uma notificação de hook LL.
        // Um `thread::sleep` aqui retém o input global por até meio segundo,
        // pois hooks WH_MOUSE_LL/WH_KEYBOARD_LL são despachados pela fila de
        // mensagens deste thread — foi a causa do sistema "travar".
        unsafe {
            let wait = MsgWaitForMultipleObjectsEx(
                None,
                POLL_INTERVAL.as_millis() as u32,
                QS_ALLINPUT,
                MWMO_INPUTAVAILABLE,
            );
            if wait == WAIT_EVENT(u32::MAX /* WAIT_FAILED */) {
                let e = windows::core::Error::from_win32();
                daemon_log(&format!("MsgWaitForMultipleObjectsEx falhou: {e}"));
                return Err(format!("{e}"));
            }
            // wake por mensagem (WAIT_OBJECT_0..+n) ou por timeout: ambos
            // caem no dreno da fila + checagem de ociosidade abaixo.
        }

        // Drena a fila de mensagens: sem isso os hooks LL não são entregues.
        // WM_QUIT encerra o loop de forma limpa.
        let mut quit = false;
        unsafe {
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                if msg.message == windows::Win32::UI::WindowsAndMessaging::WM_QUIT {
                    quit = true;
                    break;
                }
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        if quit {
            if include_taskbar {
                restore_taskbar(taskbar);
            }
            if *icons_hidden {
                set_icons_visible(true);
            }
            return Ok(t("hide_icons.run_stopped"));
        }

        // Checagem de ociosidade apenas na cadência do POLL_INTERVAL; eventos
        // frequentes de input não fazem o loop consultar o registro toda hora.
        let now = now_ms();
        if now.saturating_sub(last_check_ms) < POLL_INTERVAL.as_millis() as u64 {
            continue;
        }
        last_check_ms = now;

        // Se foi desativado pelo comando `off`, reexibe os ícones e encerra.
        if !is_enabled_in_registry() {
            if include_taskbar {
                restore_taskbar(taskbar);
            }
            if *icons_hidden {
                set_icons_visible(true);
            }
            return Ok(t("hide_icons.run_stopped"));
        }

        let idle_for = Duration::from_millis(
            now.saturating_sub(LAST_ACTIVITY_MS.load(Ordering::Relaxed)),
        );
        let just_active = idle_for < POLL_INTERVAL;

        if just_active {
            // Houve atividade: reexibe a barra e os ícones (se ocultos).
            if include_taskbar && taskbar.hidden {
                restore_taskbar(taskbar);
            }
            if *icons_hidden {
                set_icons_visible(true);
                *icons_hidden = false;
                daemon_log("atividade detectada: ícones restaurados");
            } else if include_taskbar && taskbar.hidden {
                daemon_log("atividade detectada: barra de tarefas restaurada");
            }
        } else if !*icons_hidden && idle_for >= timeout {
            if include_taskbar {
                hide_taskbar(taskbar);
            }
            set_icons_visible(false);
            *icons_hidden = true;
            daemon_log("inatividade atingiu o timeout: ícones ocultados");
        }
    }
}
