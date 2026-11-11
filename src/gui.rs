// DesktopORZ — Native Rust GUI (Impeccable Operate Design System)
// Copyright (C) 2026 ThainanViniciusKatchan
//
// Interface gráfica nativa em Rust (Win32 GDI Double-Buffered) sem terminal,
// com botões únicos e dinâmicos de estado (Ativar/Desativar) e controle completo do CLI.

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};
use std::time::UNIX_EPOCH;

use crate::config::CliConfig;
use crate::shell_locator;
use crate::types::DesktopProfile;

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{COLORREF, HANDLE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, CreateFontW, CreatePen,
    CreateSolidBrush, DeleteDC, DeleteObject, DrawTextW, EndPaint, FillRect,
    InvalidateRect, RoundRect, SelectObject, SetBkMode, SetTextColor,
    CLIP_DEFAULT_PRECIS, CLEARTYPE_QUALITY, DEFAULT_CHARSET, DEFAULT_PITCH, DT_CENTER,
    DT_END_ELLIPSIS, DT_LEFT, DT_NOPREFIX, DT_RIGHT, DT_SINGLELINE, DT_VCENTER, DT_WORDBREAK,
    FF_DONTCARE, FW_BOLD, FW_NORMAL, FW_SEMIBOLD, HDC, HFONT,
    OUT_DEFAULT_PRECIS, PAINTSTRUCT, PS_SOLID, SRCCOPY, TRANSPARENT,
};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
use windows::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetClientRect, GetMessageW, LoadCursorW,
    MessageBoxW, PostQuitMessage, RegisterClassW, ShowWindow, TranslateMessage, CS_HREDRAW,
    CS_VREDRAW, CW_USEDEFAULT, IDC_ARROW, IDC_HAND, IDYES, MB_ICONQUESTION, MB_YESNO, MSG,
    SW_SHOW, WM_CHAR, WM_CREATE, WM_DESTROY, WM_ERASEBKGND, WM_KEYDOWN, WM_LBUTTONDOWN,
    WM_LBUTTONUP, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_PAINT, WM_SIZE, WNDCLASSW, WS_CLIPCHILDREN,
    WS_OVERLAPPEDWINDOW, WS_VISIBLE,
};

const CREATE_NO_WINDOW: u32 = 0x08000000;
const CF_UNICODETEXT_VAL: u32 = 13;

// ============================================================================
// PALETA DE CORES IMPECCABLE (DARK & LIGHT)
// ============================================================================
const fn rgb(r: u8, g: u8, b: u8) -> COLORREF {
    COLORREF((r as u32) | ((g as u32) << 8) | ((b as u32) << 16))
}

#[derive(Clone, Copy)]
struct ThemePalette {
    bg_main: COLORREF,
    bg_header: COLORREF,
    bg_card: COLORREF,
    bg_elevated: COLORREF,
    bg_input: COLORREF,
    border: COLORREF,
    border_focus: COLORREF,
    text_primary: COLORREF,
    text_secondary: COLORREF,
    text_muted: COLORREF,
    primary: COLORREF,
    primary_hover: COLORREF,
    success: COLORREF,
    success_hover: COLORREF,
    danger: COLORREF,
    danger_hover: COLORREF,
    selected_card_bg: COLORREF,
    selected_card_border: COLORREF,
}

impl ThemePalette {
    fn dark() -> Self {
        Self {
            bg_main: rgb(15, 15, 18),           // #0f0f12
            bg_header: rgb(24, 24, 27),         // #18181b
            bg_card: rgb(24, 24, 27),           // #18181b
            bg_elevated: rgb(39, 39, 42),       // #27272a
            bg_input: rgb(18, 18, 21),          // #121215
            border: rgb(63, 63, 70),            // #3f3f46
            border_focus: rgb(37, 99, 235),     // #2563eb
            text_primary: rgb(244, 244, 245),   // #f4f4f5
            text_secondary: rgb(212, 212, 216), // #d4d4d8
            text_muted: rgb(161, 161, 170),     // #a1a1aa
            primary: rgb(37, 99, 235),          // #2563eb
            primary_hover: rgb(29, 78, 216),    // #1d4ed8
            success: rgb(16, 185, 129),         // #10b981
            success_hover: rgb(5, 150, 105),    // #059669
            danger: rgb(239, 68, 68),           // #ef4444
            danger_hover: rgb(220, 38, 38),     // #dc2626
            selected_card_bg: rgb(30, 58, 138), // #1e3a8a
            selected_card_border: rgb(59, 130, 246),
        }
    }

    fn light() -> Self {
        Self {
            bg_main: rgb(244, 244, 245),         // #f4f4f5
            bg_header: rgb(255, 255, 255),       // #ffffff
            bg_card: rgb(255, 255, 255),         // #ffffff
            bg_elevated: rgb(228, 228, 231),     // #e4e4e7
            bg_input: rgb(255, 255, 255),        // #ffffff
            border: rgb(212, 212, 216),          // #d4d4d8
            border_focus: rgb(37, 99, 235),      // #2563eb
            text_primary: rgb(9, 9, 11),         // #09090b
            text_secondary: rgb(63, 63, 70),     // #3f3f46
            text_muted: rgb(113, 113, 122),      // #71717a
            primary: rgb(37, 99, 235),           // #2563eb
            primary_hover: rgb(29, 78, 216),     // #1d4ed8
            success: rgb(16, 185, 129),          // #10b981
            success_hover: rgb(5, 150, 105),     // #059669
            danger: rgb(239, 68, 68),            // #ef4444
            danger_hover: rgb(220, 38, 38),      // #dc2626
            selected_card_bg: rgb(219, 234, 254),// #dbeafe
            selected_card_border: rgb(37, 99, 235),
        }
    }
}

// ============================================================================
// MODELO DE INTERAÇÃO E ELEMENTOS CLICÁVEIS
// ============================================================================
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ActiveTab {
    Profiles,
    Automations,
    Settings,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum InputField {
    None,
    NewProfileName,
    IconFilter,
    DriveTimeout,
}

#[derive(Clone, PartialEq, Eq, Debug)]
enum HitId {
    ThemeToggle,
    Tab(ActiveTab),
    Input(InputField),
    SaveNewProfile,
    SelectProfile(String),
    RestoreSelected,
    UpdateSelected,
    SaveResSelected,
    DeleteSelected,
    // Aba Automações
    SliderHideTrack,
    HideSecsDec,
    HideSecsInc,
    ToggleKb,
    ToggleMou,
    ToggleCursor,
    ToggleTaskbar,
    ToggleHideIconsBtn,
    StartupPrevProfile,
    StartupNextProfile,
    ToggleStartupBtn,
    DrivePrevProfile,
    DriveNextProfile,
    DriveTimeoutDec,
    DriveTimeoutInc,
    ToggleDriveBtn,
    // Aba Configurações
    SelectLang(String),
    SaveLangBtn,
    CliHelpBtn,
    // Console
    RefreshAllBtn,
    CopyConsoleBtn,
    ClearConsoleBtn,
}

#[derive(Clone)]
struct HitZone {
    rect: RECT,
    id: HitId,
}

impl HitZone {
    fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.rect.left && x <= self.rect.right && y >= self.rect.top && y <= self.rect.bottom
    }
}

#[derive(Clone)]
struct ProfileEntry {
    name: String,
    #[allow(dead_code)]
    path: PathBuf,
    profile: DesktopProfile,
}

// ============================================================================
// ESTADO PRINCIPAL DA APLICAÇÃO
// ============================================================================
struct GuiState {
    dark_mode: bool,
    active_tab: ActiveTab,
    focused_input: InputField,

    // Caminhos e dados
    binary_path: Option<PathBuf>,
    profiles: BTreeMap<String, ProfileEntry>,
    selected_profile: Option<String>,

    // Campos de texto
    new_profile_input: String,
    icon_filter_input: String,
    drive_timeout_input: String,

    // Scroll offsets
    profiles_scroll: i32,
    icons_scroll: i32,
    console_scroll: i32,
    icons_area_rect: RECT,
    profiles_area_rect: RECT,
    console_area_rect: RECT,

    // Estados dinâmicos das automações
    hide_icons_enabled: bool,
    hide_seconds: u64,
    hide_kb: bool,
    hide_mou: bool,
    hide_cursor: bool,
    hide_taskbar: bool,
    dragging_slider: bool,
    slider_track_rect: RECT,

    startup_enabled: bool,
    startup_profile: String,

    wait_drive_enabled: bool,
    wait_drive_profile: String,

    selected_lang: String,

    // Console
    exec_status: String,
    exec_status_ok: Option<bool>,
    console_lines: Vec<String>,

    // Zonas de clique e hover
    hit_zones: Vec<HitZone>,
    hovered_id: Option<HitId>,
}

impl GuiState {
    fn new() -> Self {
        let mut s = Self {
            dark_mode: true,
            active_tab: ActiveTab::Profiles,
            focused_input: InputField::None,
            binary_path: find_desktoporz_binary(),
            profiles: BTreeMap::new(),
            selected_profile: None,
            new_profile_input: String::new(),
            icon_filter_input: String::new(),
            drive_timeout_input: "30".to_string(),
            profiles_scroll: 0,
            icons_scroll: 0,
            console_scroll: 0,
            icons_area_rect: RECT::default(),
            profiles_area_rect: RECT::default(),
            console_area_rect: RECT::default(),
            hide_icons_enabled: false,
            hide_seconds: 5,
            hide_kb: true,
            hide_mou: true,
            hide_cursor: false,
            hide_taskbar: false,
            dragging_slider: false,
            slider_track_rect: RECT::default(),
            startup_enabled: false,
            startup_profile: String::new(),
            wait_drive_enabled: false,
            wait_drive_profile: String::new(),
            selected_lang: "pt-br".to_string(),
            exec_status: "Pronto.".to_string(),
            exec_status_ok: None,
            console_lines: vec![
                "DesktopORZ GUI (Rust Native) inicializada com sucesso.".to_string(),
                "Pronto para gerenciar layouts de ícones e automações sem terminal.".to_string(),
            ],
            hit_zones: Vec::new(),
            hovered_id: None,
        };
        s.refresh_all_data(false);
        s
    }

    fn palette(&self) -> ThemePalette {
        if self.dark_mode {
            ThemePalette::dark()
        } else {
            ThemePalette::light()
        }
    }

    fn append_log(&mut self, text: &str) {
        for line in text.lines() {
            self.console_lines.push(line.to_string());
        }
        if self.console_lines.len() > 300 {
            let drain_count = self.console_lines.len() - 300;
            self.console_lines.drain(0..drain_count);
        }
        // Auto-scroll para o fim
        self.console_scroll = (self.console_lines.len() as i32 - 5).max(0);
    }

    fn execute_cli_action(&mut self, args: &[&str]) {
        let cmd_str = format!("DesktopORZ {}", args.join(" "));
        self.append_log("=========================================");
        self.append_log(&format!("▶ Executando: {cmd_str}"));
        self.append_log("=========================================");

        sync_profiles_across_dirs();

        let (ok, output) = run_cli_command(self.binary_path.as_deref(), args);
        if !output.trim().is_empty() {
            self.append_log(output.trim());
        }
        if ok {
            self.append_log("--- Fim (código 0) ---");
            self.exec_status = "✔ Comando concluído com sucesso".to_string();
            self.exec_status_ok = Some(true);
        } else {
            self.append_log("--- Fim (erro) ---");
            self.exec_status = "✖ Erro na execução".to_string();
            self.exec_status_ok = Some(false);
        }

        self.refresh_all_data(false);
    }

    fn refresh_all_data(&mut self, log_refresh: bool) {
        self.binary_path = find_desktoporz_binary();
        sync_profiles_across_dirs();
        self.load_profiles_from_disk();
        self.sync_services_status();
        if log_refresh {
            self.exec_status = "✔ Dados e status sincronizados".to_string();
            self.exec_status_ok = Some(true);
        }
    }

    fn load_profiles_from_disk(&mut self) {
        self.profiles.clear();
        for dir in get_all_profiles_dirs() {
            let Ok(entries) = fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("json") {
                    continue;
                }
                let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                    continue;
                };
                if self.profiles.contains_key(stem) {
                    continue;
                }
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(profile) = serde_json::from_str::<DesktopProfile>(&content) {
                        self.profiles.insert(
                            stem.to_string(),
                            ProfileEntry {
                                name: stem.to_string(),
                                path,
                                profile,
                            },
                        );
                    }
                }
            }
        }

        let names: Vec<String> = self.profiles.keys().cloned().collect();
        if let Some(sel) = &self.selected_profile {
            if !self.profiles.contains_key(sel) {
                self.selected_profile = names.first().cloned();
            }
        } else {
            self.selected_profile = names.first().cloned();
        }

        if self.startup_profile.is_empty() || !self.profiles.contains_key(&self.startup_profile) {
            self.startup_profile = names.first().cloned().unwrap_or_else(|| "Nenhum".to_string());
        }
        if self.wait_drive_profile.is_empty()
            || !self.profiles.contains_key(&self.wait_drive_profile)
        {
            self.wait_drive_profile =
                names.first().cloned().unwrap_or_else(|| "Nenhum".to_string());
        }
    }

    fn sync_services_status(&mut self) {
        // Garante que o config.json esteja atualizado consultando os módulos/CLI
        let (_, res_s) = run_cli_command(self.binary_path.as_deref(), &["startup", "status"]);
        let (_, res_h) = run_cli_command(self.binary_path.as_deref(), &["hide-icons", "status"]);
        let (_, res_d) = run_cli_command(self.binary_path.as_deref(), &["wait-drive", "status"]);

        let cfg = read_cli_config(self.binary_path.as_deref());

        // 1. Startup
        if let Some(st) = cfg.startup {
            self.startup_enabled = st.enabled;
            if let Some(p) = st.profile {
                if !p.is_empty() {
                    self.startup_profile = p;
                }
            }
        } else {
            let low = res_s.to_lowercase();
            self.startup_enabled = (low.contains("enabled") || low.contains("ativad"))
                && !(low.contains("disabled") || low.contains("desativad"));
        }

        // 2. Hide Icons
        if let Some(hi) = cfg.hide_icons {
            self.hide_icons_enabled = hi.enabled;
            let secs = hi.timeout_secs.unwrap_or(5).clamp(3, 60);
            self.hide_seconds = secs;
            let inputs = hi.inputs.unwrap_or(3);
            self.hide_kb = (inputs & 2) != 0;
            self.hide_mou = (inputs & 1) != 0;
            if !self.hide_kb && !self.hide_mou {
                self.hide_kb = true;
                self.hide_mou = true;
            }
            self.hide_cursor = hi.include_cursor.unwrap_or(false);
            self.hide_taskbar = hi.include_taskbar.unwrap_or(false);
        } else {
            let low = res_h.to_lowercase();
            self.hide_icons_enabled = (low.contains("enabled") || low.contains("ativad"))
                && !(low.contains("disabled") || low.contains("desativad"));
        }

        // 3. Wait Drive
        if let Some(wd) = cfg.wait_drive {
            self.wait_drive_enabled = wd.enabled;
            if let Some(p) = wd.profile {
                if !p.is_empty() {
                    self.wait_drive_profile = p;
                }
            }
            if let Some(t) = wd.timeout_secs {
                self.drive_timeout_input = t.to_string();
            }
        } else {
            let low = res_d.to_lowercase();
            self.wait_drive_enabled = (low.contains("enabled") || low.contains("ativad"))
                && !(low.contains("disabled") || low.contains("desativad"));
        }

        if !cfg.language.is_empty() {
            self.selected_lang = cfg.language;
        }
    }

    fn apply_hide_icons_on(&mut self) {
        let secs_str = self.hide_seconds.to_string();
        let mut args: Vec<&str> = vec!["hide-icons", "on", &secs_str];
        if self.hide_kb && !self.hide_mou {
            args.push("--kb");
        } else if self.hide_mou && !self.hide_kb {
            args.push("--mou");
        } else {
            args.push("--kb");
            args.push("--mou");
        }
        if self.hide_cursor {
            args.push("-include-cursor");
        }
        if self.hide_taskbar {
            args.push("-include-taskbar");
        }
        self.execute_cli_action(&args);
    }

    fn apply_startup_on(&mut self) {
        let prof = self.startup_profile.clone();
        if prof.is_empty() || prof == "Nenhum" {
            self.append_log("⚠ Crie ou selecione um perfil válido antes de ativar a inicialização.");
            return;
        }
        self.execute_cli_action(&["startup", "on", &prof]);
    }

    fn apply_drive_on(&mut self) {
        let prof = self.wait_drive_profile.clone();
        if prof.is_empty() || prof == "Nenhum" {
            self.append_log("⚠ Crie ou selecione um perfil válido antes de ativar o Wait-Drive.");
            return;
        }
        let timeout_clean = self.drive_timeout_input.trim().to_string();
        if !timeout_clean.is_empty() && timeout_clean.chars().all(|c| c.is_ascii_digit()) {
            self.execute_cli_action(&["wait-drive", "on", &prof, "--timeout", &timeout_clean]);
        } else {
            self.execute_cli_action(&["wait-drive", "on", &prof]);
        }
    }

    fn cycle_profile(&self, current: &str, forward: bool) -> String {
        let names: Vec<String> = self.profiles.keys().cloned().collect();
        if names.is_empty() {
            return "Nenhum".to_string();
        }
        let idx = names.iter().position(|n| n == current).unwrap_or(0);
        let next_idx = if forward {
            (idx + 1) % names.len()
        } else if idx == 0 {
            names.len() - 1
        } else {
            idx - 1
        };
        names[next_idx].clone()
    }
}

static STATE: OnceLock<Mutex<GuiState>> = OnceLock::new();

fn state() -> &'static Mutex<GuiState> {
    STATE.get_or_init(|| Mutex::new(GuiState::new()))
}

// ============================================================================
// FUNÇÕES DE DESCOBERTA DE ARQUIVOS E EXECUÇÃO DO CLI
// ============================================================================
fn project_dir() -> PathBuf {
    env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

fn find_desktoporz_binary() -> Option<PathBuf> {
    let proj = project_dir();
    let exe_dir = env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()));

    let mut candidates = Vec::new();
    if let Some(ed) = &exe_dir {
        candidates.push(ed.join("DesktopORZ.exe"));
    }
    candidates.push(PathBuf::from(r"C:\cargo-target\DesktopORZ\release\DesktopORZ.exe"));
    candidates.push(proj.join("DesktopORZ.exe"));
    candidates.push(proj.join("target").join("release").join("DesktopORZ.exe"));
    candidates.push(PathBuf::from(r"C:\cargo-target\DesktopORZ\debug\DesktopORZ.exe"));

    for c in candidates {
        if c.is_file() {
            return Some(c);
        }
    }
    env::current_exe().ok()
}

fn get_all_profiles_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let proj = project_dir();
    let mut candidates = vec![
        proj.join("profiles"),
        PathBuf::from(r"C:\cargo-target\DesktopORZ\release\profiles"),
    ];
    if let Ok(exe) = env::current_exe() {
        if let Some(parent) = exe.parent() {
            candidates.insert(0, parent.join("profiles"));
        }
    }
    for p in candidates {
        if !dirs.contains(&p) {
            dirs.push(p);
        }
    }
    dirs
}

fn sync_profiles_across_dirs() {
    let dirs = get_all_profiles_dirs();
    let mut valid_dirs = Vec::new();
    for d in &dirs {
        if fs::create_dir_all(d).is_ok() {
            valid_dirs.push(d.clone());
        }
    }

    let mut latest_files: BTreeMap<String, (u64, PathBuf)> = BTreeMap::new();
    let mut search_dirs = vec![project_dir()];
    search_dirs.extend(valid_dirs.clone());

    for d in &search_dirs {
        let Ok(entries) = fs::read_dir(d) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let Some(fname) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            let lower = fname.to_lowercase();
            if !lower.ends_with(".json")
                || lower == "config.json"
                || lower == "package.json"
                || lower == "tsconfig.json"
            {
                continue;
            }
            let Ok(content) = fs::read_to_string(&path) else {
                continue;
            };
            if serde_json::from_str::<DesktopProfile>(&content).is_err() {
                continue;
            }
            let mtime = fs::metadata(&path)
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);

            match latest_files.get(fname) {
                Some((prev_mtime, _)) if *prev_mtime >= mtime => {}
                _ => {
                    latest_files.insert(fname.to_string(), (mtime, path));
                }
            }
        }
    }

    for (fname, (src_mtime, src_path)) in &latest_files {
        for d in &valid_dirs {
            let dst_path = d.join(fname);
            if src_path == &dst_path {
                continue;
            }
            let dst_mtime = fs::metadata(&dst_path)
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            if !dst_path.exists() || *src_mtime > dst_mtime {
                let _ = fs::copy(src_path, &dst_path);
            }
        }
    }
}

fn read_cli_config(bin_path: Option<&Path>) -> CliConfig {
    let mut candidates = Vec::new();
    if let Some(bin) = bin_path {
        if let Some(parent) = bin.parent() {
            candidates.push(parent.join("config.json"));
        }
    }
    candidates.push(project_dir().join("config.json"));
    candidates.push(PathBuf::from(r"C:\cargo-target\DesktopORZ\release\config.json"));

    for c in candidates {
        if let Ok(text) = fs::read_to_string(&c) {
            if let Ok(cfg) = serde_json::from_str::<CliConfig>(&text) {
                return cfg;
            }
        }
    }
    crate::config::load_or_create_config()
}

fn run_cli_command(bin_path: Option<&Path>, args: &[&str]) -> (bool, String) {
    let Some(bin) = bin_path else {
        return (false, "Executável DesktopORZ.exe não encontrado.".to_string());
    };
    let work_dir = bin.parent().unwrap_or_else(|| Path::new("."));
    match Command::new(bin)
        .args(args)
        .current_dir(work_dir)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
    {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let stderr = String::from_utf8_lossy(&out.stderr);
            let mut combined = String::new();
            if !stdout.trim().is_empty() {
                combined.push_str(stdout.trim());
            }
            if !stderr.trim().is_empty() {
                if !combined.is_empty() {
                    combined.push('\n');
                }
                combined.push_str(stderr.trim());
            }
            (out.status.success(), combined)
        }
        Err(e) => (false, format!("Falha ao executar DesktopORZ: {e}")),
    }
}

// ============================================================================
// FORMATAÇÃO DE DATA/HORA (UTC -> DD/MM/YYYY HH:MM)
// ============================================================================
fn format_timestamp(ts: u64) -> String {
    if ts == 0 {
        return "Data desc.".to_string();
    }
    // Ajusta para UTC-4 (horário local aproximado)
    let local_secs = ts.saturating_sub(4 * 3600);
    let days = local_secs / 86400;
    let rem = local_secs % 86400;
    let hour = rem / 3600;
    let min = (rem % 3600) / 60;

    // Algoritmo civil_from_days (Howard Hinnant)
    let z = days as i64 + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };

    format!("{d:02}/{m:02}/{year:04} {hour:02}:{min:02}")
}

// ============================================================================
// AUXILIARES DE DESENHO WIN32 GDI
// ============================================================================
fn make_rect(left: i32, top: i32, right: i32, bottom: i32) -> RECT {
    RECT {
        left,
        top,
        right,
        bottom,
    }
}

fn create_ui_font(size: i32, weight: i32, mono: bool) -> HFONT {
    let face = if mono {
        w!("Consolas")
    } else {
        w!("Segoe UI")
    };
    unsafe {
        CreateFontW(
            -size,
            0,
            0,
            0,
            weight,
            0,
            0,
            0,
            DEFAULT_CHARSET.0 as u32,
            OUT_DEFAULT_PRECIS.0 as u32,
            CLIP_DEFAULT_PRECIS.0 as u32,
            CLEARTYPE_QUALITY.0 as u32,
            (DEFAULT_PITCH.0 | FF_DONTCARE.0) as u32,
            face,
        )
    }
}

fn fill_solid_rect(hdc: HDC, r: RECT, color: COLORREF) {
    unsafe {
        let brush = CreateSolidBrush(color);
        FillRect(hdc, &r, brush);
        let _ = DeleteObject(brush);
    }
}

fn draw_round_box(hdc: HDC, r: RECT, fill: COLORREF, border: COLORREF, radius: i32) {
    unsafe {
        let brush = CreateSolidBrush(fill);
        let pen = CreatePen(PS_SOLID, 1, border);
        let old_b = SelectObject(hdc, brush);
        let old_p = SelectObject(hdc, pen);
        let _ = RoundRect(hdc, r.left, r.top, r.right, r.bottom, radius, radius);
        SelectObject(hdc, old_b);
        SelectObject(hdc, old_p);
        let _ = DeleteObject(brush);
        let _ = DeleteObject(pen);
    }
}

fn draw_text_in(hdc: HDC, text: &str, mut r: RECT, font: HFONT, color: COLORREF, flags: u32) {
    unsafe {
        let old_f = SelectObject(hdc, font);
        SetBkMode(hdc, TRANSPARENT);
        SetTextColor(hdc, color);
        let mut wide: Vec<u16> = text.encode_utf16().collect();
        DrawTextW(
            hdc,
            &mut wide,
            &mut r,
            windows::Win32::Graphics::Gdi::DRAW_TEXT_FORMAT(flags | DT_NOPREFIX.0),
        );
        SelectObject(hdc, old_f);
    }
}

fn draw_button(
    hdc: HDC,
    r: RECT,
    text: &str,
    font: HFONT,
    bg: COLORREF,
    bg_hover: COLORREF,
    fg: COLORREF,
    hovered: bool,
    radius: i32,
) {
    let fill = if hovered { bg_hover } else { bg };
    draw_round_box(hdc, r, fill, fill, radius);
    draw_text_in(
        hdc,
        text,
        r,
        font,
        fg,
        DT_CENTER.0 | DT_VCENTER.0 | DT_SINGLELINE.0 | DT_END_ELLIPSIS.0,
    );
}

fn draw_checkbox(
    hdc: HDC,
    r: RECT,
    label: &str,
    checked: bool,
    hovered: bool,
    font: HFONT,
    pal: &ThemePalette,
) {
    let box_size = 18;
    let cy = (r.top + r.bottom) / 2;
    let box_rect = make_rect(r.left, cy - box_size / 2, r.left + box_size, cy + box_size / 2);

    let fill = if checked {
        pal.primary
    } else if hovered {
        pal.bg_elevated
    } else {
        pal.bg_input
    };
    let border = if checked || hovered {
        pal.primary
    } else {
        pal.border
    };
    draw_round_box(hdc, box_rect, fill, border, 5);

    if checked {
        draw_text_in(
            hdc,
            "✓",
            box_rect,
            font,
            rgb(255, 255, 255),
            DT_CENTER.0 | DT_VCENTER.0 | DT_SINGLELINE.0,
        );
    }

    let lbl_rect = make_rect(r.left + box_size + 8, r.top, r.right, r.bottom);
    draw_text_in(
        hdc,
        label,
        lbl_rect,
        font,
        pal.text_primary,
        DT_LEFT.0 | DT_VCENTER.0 | DT_SINGLELINE.0,
    );
}

// ============================================================================
// RENDERIZAÇÃO COMPLETA DA INTERFACE
// ============================================================================
fn render_gui(hdc: HDC, width: i32, height: i32, st: &mut GuiState) {
    let pal = st.palette();
    st.hit_zones.clear();

    // Fontes tipográficas Impeccable
    let f_title = create_ui_font(20, FW_BOLD.0 as i32, false);
    let f_sec = create_ui_font(15, FW_BOLD.0 as i32, false);
    let f_sub_bold = create_ui_font(13, FW_BOLD.0 as i32, false);
    let f_body = create_ui_font(13, FW_NORMAL.0 as i32, false);
    let f_small_bold = create_ui_font(11, FW_SEMIBOLD.0 as i32, false);
    let f_small = create_ui_font(11, FW_NORMAL.0 as i32, false);
    let f_mono = create_ui_font(12, FW_NORMAL.0 as i32, true);

    // Fundo principal
    fill_solid_rect(hdc, make_rect(0, 0, width, height), pal.bg_main);

    // ------------------------------------------------------------------------
    // 1. HEADER SUPERIOR (0..64)
    // ------------------------------------------------------------------------
    let header_h = 64;
    fill_solid_rect(hdc, make_rect(0, 0, width, header_h), pal.bg_header);
    // Linha divisória sutil no header
    fill_solid_rect(hdc, make_rect(0, header_h - 1, width, header_h), pal.border);

    draw_text_in(
        hdc,
        "DesktopORZ",
        make_rect(20, 8, 340, 34),
        f_title,
        pal.text_primary,
        DT_LEFT.0 | DT_VCENTER.0 | DT_SINGLELINE.0,
    );
    draw_text_in(
        hdc,
        "Gerenciador de Layouts e Ícones do Windows (Rust GUI)",
        make_rect(20, 34, 420, 56),
        f_small,
        pal.text_muted,
        DT_LEFT.0 | DT_VCENTER.0 | DT_SINGLELINE.0,
    );

    // Botão de Tema no canto superior direito
    let theme_btn_rect = make_rect(width - 125, 18, width - 20, 46);
    let theme_hovered = st.hovered_id == Some(HitId::ThemeToggle);
    let theme_txt = if st.dark_mode {
        "🌙 Tema: Dark"
    } else {
        "☀️ Tema: Light"
    };
    draw_round_box(
        hdc,
        theme_btn_rect,
        if theme_hovered { pal.border } else { pal.bg_elevated },
        pal.border,
        6,
    );
    draw_text_in(
        hdc,
        theme_txt,
        theme_btn_rect,
        f_small_bold,
        pal.text_primary,
        DT_CENTER.0 | DT_VCENTER.0 | DT_SINGLELINE.0,
    );
    st.hit_zones.push(HitZone {
        rect: theme_btn_rect,
        id: HitId::ThemeToggle,
    });

    // Badges de Status no Header
    let mut bx = width - 135;
    let badges = [
        (
            if st.wait_drive_enabled {
                format!("● Wait-Drive: Ativo ({})", st.wait_drive_profile)
            } else {
                "○ Wait-Drive: Inativo".to_string()
            },
            if st.wait_drive_enabled {
                pal.success
            } else {
                pal.text_muted
            },
            155,
        ),
        (
            if st.hide_icons_enabled {
                format!("● Ocultar: Ativo ({}s)", st.hide_seconds)
            } else {
                "○ Ocultar: Inativo".to_string()
            },
            if st.hide_icons_enabled {
                pal.success
            } else {
                pal.text_muted
            },
            145,
        ),
        (
            if st.startup_enabled {
                format!("● Startup: Ativo ({})", st.startup_profile)
            } else {
                "○ Startup: Inativo".to_string()
            },
            if st.startup_enabled {
                pal.success
            } else {
                pal.text_muted
            },
            145,
        ),
        (
            if st.binary_path.is_some() {
                "● Binário OK".to_string()
            } else {
                "● Binário Ausente".to_string()
            },
            if st.binary_path.is_some() {
                pal.success
            } else {
                pal.danger
            },
            105,
        ),
    ];

    for (b_text, b_color, b_w) in badges {
        if bx - b_w < 380 {
            break;
        }
        let br = make_rect(bx - b_w, 18, bx, 46);
        draw_round_box(hdc, br, pal.bg_elevated, pal.border, 6);
        draw_text_in(
            hdc,
            &b_text,
            br,
            f_small_bold,
            b_color,
            DT_CENTER.0 | DT_VCENTER.0 | DT_SINGLELINE.0 | DT_END_ELLIPSIS.0,
        );
        bx -= b_w + 8;
    }

    // ------------------------------------------------------------------------
    // 2. BARRA DE ABAS (72..108)
    // ------------------------------------------------------------------------
    let tabs_y = 74;
    let tabs_h = 34;
    let tabs = [
        (ActiveTab::Profiles, "📁 Perfis & Área de Trabalho", 230),
        (
            ActiveTab::Automations,
            "⚙️ Automações (Ocultar, Startup, Nuvem)",
            280,
        ),
        (ActiveTab::Settings, "🌐 Idioma & Sistema", 180),
    ];
    let mut tx = 16;
    for (tab_enum, tab_label, tab_w) in tabs {
        let tr = make_rect(tx, tabs_y, tx + tab_w, tabs_y + tabs_h);
        let is_active = st.active_tab == tab_enum;
        let is_hov = st.hovered_id == Some(HitId::Tab(tab_enum));
        let bg = if is_active {
            pal.primary
        } else if is_hov {
            pal.bg_elevated
        } else {
            pal.bg_card
        };
        let fg = if is_active {
            rgb(255, 255, 255)
        } else {
            pal.text_secondary
        };
        draw_round_box(hdc, tr, bg, if is_active { pal.primary } else { pal.border }, 8);
        draw_text_in(
            hdc,
            tab_label,
            tr,
            f_sub_bold,
            fg,
            DT_CENTER.0 | DT_VCENTER.0 | DT_SINGLELINE.0,
        );
        st.hit_zones.push(HitZone {
            rect: tr,
            id: HitId::Tab(tab_enum),
        });
        tx += tab_w + 8;
    }

    // ------------------------------------------------------------------------
    // 3. ÁREA CENTRAL DA ABA ATIVA
    // ------------------------------------------------------------------------
    let console_h = 155;
    let content_top = 116;
    let content_bottom = (height - console_h - 14).max(content_top + 200);
    let content_rect = make_rect(16, content_top, width - 16, content_bottom);

    match st.active_tab {
        ActiveTab::Profiles => render_profiles_tab(
            hdc,
            content_rect,
            st,
            &pal,
            f_sec,
            f_sub_bold,
            f_body,
            f_small_bold,
            f_small,
            f_mono,
        ),
        ActiveTab::Automations => render_automations_tab(
            hdc,
            content_rect,
            st,
            &pal,
            f_sec,
            f_sub_bold,
            f_body,
            f_small_bold,
            f_small,
        ),
        ActiveTab::Settings => render_settings_tab(
            hdc,
            content_rect,
            st,
            &pal,
            f_sec,
            f_sub_bold,
            f_body,
            f_mono,
        ),
    }

    // ------------------------------------------------------------------------
    // 4. CONSOLE & LOG EM TEMPO REAL (RODAPÉ)
    // ------------------------------------------------------------------------
    let con_rect = make_rect(16, content_bottom + 8, width - 16, height - 12);
    st.console_area_rect = con_rect;
    draw_round_box(hdc, con_rect, pal.bg_card, pal.border, 8);

    // Barra superior do console
    let status_color = match st.exec_status_ok {
        Some(true) => pal.success,
        Some(false) => pal.danger,
        None => pal.text_muted,
    };
    draw_text_in(
        hdc,
        &st.exec_status,
        make_rect(con_rect.left + 12, con_rect.top + 6, con_rect.right - 290, con_rect.top + 32),
        f_sub_bold,
        status_color,
        DT_LEFT.0 | DT_VCENTER.0 | DT_SINGLELINE.0 | DT_END_ELLIPSIS.0,
    );

    // Botões do console: Atualizar Tudo, Copiar, Limpar
    let btn_clear_r = make_rect(con_rect.right - 82, con_rect.top + 6, con_rect.right - 12, con_rect.top + 30);
    draw_button(
        hdc,
        btn_clear_r,
        "Limpar",
        f_small_bold,
        pal.bg_elevated,
        pal.border,
        pal.text_primary,
        st.hovered_id == Some(HitId::ClearConsoleBtn),
        6,
    );
    st.hit_zones.push(HitZone {
        rect: btn_clear_r,
        id: HitId::ClearConsoleBtn,
    });

    let btn_copy_r = make_rect(con_rect.right - 166, con_rect.top + 6, con_rect.right - 88, con_rect.top + 30);
    draw_button(
        hdc,
        btn_copy_r,
        "📋 Copiar",
        f_small_bold,
        pal.primary,
        pal.primary_hover,
        rgb(255, 255, 255),
        st.hovered_id == Some(HitId::CopyConsoleBtn),
        6,
    );
    st.hit_zones.push(HitZone {
        rect: btn_copy_r,
        id: HitId::CopyConsoleBtn,
    });

    let btn_ref_r = make_rect(con_rect.right - 288, con_rect.top + 6, con_rect.right - 172, con_rect.top + 30);
    draw_button(
        hdc,
        btn_ref_r,
        "🔄 Atualizar Tudo",
        f_small_bold,
        pal.primary,
        pal.primary_hover,
        rgb(255, 255, 255),
        st.hovered_id == Some(HitId::RefreshAllBtn),
        6,
    );
    st.hit_zones.push(HitZone {
        rect: btn_ref_r,
        id: HitId::RefreshAllBtn,
    });

    // Caixa de texto do console
    let log_box = make_rect(con_rect.left + 10, con_rect.top + 36, con_rect.right - 10, con_rect.bottom - 8);
    draw_round_box(hdc, log_box, pal.bg_input, pal.border, 6);

    let line_h = 17;
    let visible_lines = ((log_box.bottom - log_box.top - 8) / line_h).max(1) as usize;
    let total_lines = st.console_lines.len();
    let max_scroll = (total_lines as i32 - visible_lines as i32).max(0);
    st.console_scroll = st.console_scroll.clamp(0, max_scroll);

    let start_idx = st.console_scroll as usize;
    let mut ly = log_box.top + 4;
    for line in st.console_lines.iter().skip(start_idx).take(visible_lines) {
        let line_color = if line.contains("Error") || line.contains("Erro") || line.contains("✖") {
            pal.danger
        } else if line.contains("▶ Executando") {
            pal.primary
        } else {
            pal.text_secondary
        };
        draw_text_in(
            hdc,
            line,
            make_rect(log_box.left + 8, ly, log_box.right - 8, ly + line_h),
            f_mono,
            line_color,
            DT_LEFT.0 | DT_VCENTER.0 | DT_SINGLELINE.0 | DT_END_ELLIPSIS.0,
        );
        ly += line_h;
    }

    // Libera objetos de fonte GDI
    unsafe {
        let _ = DeleteObject(f_title);
        let _ = DeleteObject(f_sec);
        let _ = DeleteObject(f_sub_bold);
        let _ = DeleteObject(f_body);
        let _ = DeleteObject(f_small_bold);
        let _ = DeleteObject(f_small);
        let _ = DeleteObject(f_mono);
    }
}

// ============================================================================
// ABA 1: PERFIS & ÁREA DE TRABALHO
// ============================================================================
#[allow(clippy::too_many_arguments)]
fn render_profiles_tab(
    hdc: HDC,
    area: RECT,
    st: &mut GuiState,
    pal: &ThemePalette,
    f_sec: HFONT,
    f_sub_bold: HFONT,
    f_body: HFONT,
    f_small_bold: HFONT,
    f_small: HFONT,
    f_mono: HFONT,
) {
    // 1. Barra Superior: Salvar Layout Atual
    let top_bar = make_rect(area.left, area.top, area.right, area.top + 52);
    draw_round_box(hdc, top_bar, pal.bg_elevated, pal.border, 8);

    draw_text_in(
        hdc,
        "💾 Salvar Layout Atual:",
        make_rect(top_bar.left + 14, top_bar.top, top_bar.left + 185, top_bar.bottom),
        f_sub_bold,
        pal.text_primary,
        DT_LEFT.0 | DT_VCENTER.0 | DT_SINGLELINE.0,
    );

    let save_btn_w = 185;
    let save_btn_r = make_rect(
        top_bar.right - save_btn_w - 12,
        top_bar.top + 9,
        top_bar.right - 12,
        top_bar.bottom - 9,
    );
    let inp_r = make_rect(
        top_bar.left + 190,
        top_bar.top + 9,
        save_btn_r.left - 10,
        top_bar.bottom - 9,
    );

    let is_inp_focus = st.focused_input == InputField::NewProfileName;
    draw_round_box(
        hdc,
        inp_r,
        pal.bg_input,
        if is_inp_focus {
            pal.border_focus
        } else {
            pal.border
        },
        6,
    );
    let (inp_text, inp_col) = if st.new_profile_input.is_empty() && !is_inp_focus {
        (
            "Digite o nome do novo perfil (ex: PC, Notebook, Monitor_Externo)".to_string(),
            pal.text_muted,
        )
    } else if is_inp_focus {
        (format!("{}|", st.new_profile_input), pal.text_primary)
    } else {
        (st.new_profile_input.clone(), pal.text_primary)
    };
    draw_text_in(
        hdc,
        &inp_text,
        make_rect(inp_r.left + 10, inp_r.top, inp_r.right - 10, inp_r.bottom),
        f_body,
        inp_col,
        DT_LEFT.0 | DT_VCENTER.0 | DT_SINGLELINE.0 | DT_END_ELLIPSIS.0,
    );
    st.hit_zones.push(HitZone {
        rect: inp_r,
        id: HitId::Input(InputField::NewProfileName),
    });

    draw_button(
        hdc,
        save_btn_r,
        "Gravar Posições Atuais",
        f_sub_bold,
        pal.primary,
        pal.primary_hover,
        rgb(255, 255, 255),
        st.hovered_id == Some(HitId::SaveNewProfile),
        6,
    );
    st.hit_zones.push(HitZone {
        rect: save_btn_r,
        id: HitId::SaveNewProfile,
    });

    // 2. Colunas Esquerda (Perfis) e Direita (Detalhes + Tabela de Ícones)
    let cols_top = top_bar.bottom + 10;
    let total_w = area.right - area.left;
    let left_w = (total_w * 42) / 100;
    let left_col = make_rect(area.left, cols_top, area.left + left_w, area.bottom);
    let right_col = make_rect(left_col.right + 10, cols_top, area.right, area.bottom);

    st.profiles_area_rect = left_col;
    draw_round_box(hdc, left_col, pal.bg_card, pal.border, 8);
    draw_round_box(hdc, right_col, pal.bg_card, pal.border, 8);

    // --- Coluna Esquerda: Lista de Perfis Armazenados ---
    draw_text_in(
        hdc,
        "Perfis Armazenados",
        make_rect(left_col.left + 14, left_col.top + 10, left_col.right - 14, left_col.top + 34),
        f_sec,
        pal.text_primary,
        DT_LEFT.0 | DT_VCENTER.0 | DT_SINGLELINE.0,
    );

    let list_top = left_col.top + 40;
    let list_bottom = left_col.bottom - 10;
    let card_h = 56;
    let card_gap = 6;

    if st.profiles.is_empty() {
        draw_text_in(
            hdc,
            "Nenhum perfil salvo ainda.\nDigite um nome acima e clique em 'Gravar Posições Atuais'.",
            make_rect(left_col.left + 16, list_top + 30, left_col.right - 16, list_top + 110),
            f_body,
            pal.text_muted,
            DT_CENTER.0 | DT_WORDBREAK.0,
        );
    } else {
        let names: Vec<String> = st.profiles.keys().cloned().collect();
        let max_cards = ((list_bottom - list_top) / (card_h + card_gap)).max(1) as usize;
        let max_p_scroll = (names.len() as i32 - max_cards as i32).max(0);
        st.profiles_scroll = st.profiles_scroll.clamp(0, max_p_scroll);

        let mut cy = list_top;
        for name in names.iter().skip(st.profiles_scroll as usize).take(max_cards) {
            let Some(entry) = st.profiles.get(name) else {
                continue;
            };
            let is_sel = st.selected_profile.as_deref() == Some(name.as_str());
            let is_hov = st.hovered_id == Some(HitId::SelectProfile(name.clone()));

            let cr = make_rect(left_col.left + 10, cy, left_col.right - 10, cy + card_h);
            let c_bg = if is_sel {
                pal.selected_card_bg
            } else if is_hov {
                pal.border
            } else {
                pal.bg_elevated
            };
            let c_border = if is_sel {
                pal.selected_card_border
            } else {
                pal.border
            };
            draw_round_box(hdc, cr, c_bg, c_border, 8);

            draw_text_in(
                hdc,
                &format!("📁 {}", entry.name),
                make_rect(cr.left + 12, cr.top + 6, cr.right - 12, cr.top + 28),
                f_sub_bold,
                if is_sel && st.dark_mode {
                    rgb(255, 255, 255)
                } else {
                    pal.text_primary
                },
                DT_LEFT.0 | DT_VCENTER.0 | DT_SINGLELINE.0 | DT_END_ELLIPSIS.0,
            );

            let dt_str = format_timestamp(entry.profile.timestamp_utc);
            let meta = format!(
                "{}x{}  •  {} ícones  •  {}",
                entry.profile.resolution.width,
                entry.profile.resolution.height,
                entry.profile.icons.len(),
                dt_str
            );
            draw_text_in(
                hdc,
                &meta,
                make_rect(cr.left + 12, cr.top + 28, cr.right - 12, cr.bottom - 6),
                f_small,
                if is_sel && st.dark_mode {
                    rgb(219, 234, 254)
                } else {
                    pal.text_muted
                },
                DT_LEFT.0 | DT_VCENTER.0 | DT_SINGLELINE.0 | DT_END_ELLIPSIS.0,
            );

            st.hit_zones.push(HitZone {
                rect: cr,
                id: HitId::SelectProfile(name.clone()),
            });
            cy += card_h + card_gap;
        }
    }

    // --- Coluna Direita: Detalhes do Perfil Selecionado ---
    let selected_entry = st
        .selected_profile
        .as_ref()
        .and_then(|n| st.profiles.get(n))
        .cloned();

    if let Some(entry) = selected_entry {
        let dt_str = format_timestamp(entry.profile.timestamp_utc);
        draw_text_in(
            hdc,
            &format!("Perfil: {}", entry.name),
            make_rect(right_col.left + 14, right_col.top + 10, right_col.right - 14, right_col.top + 32),
            f_sec,
            pal.text_primary,
            DT_LEFT.0 | DT_VCENTER.0 | DT_SINGLELINE.0,
        );
        draw_text_in(
            hdc,
            &format!(
                "Resolução: {}x{}  |  Total de Ícones: {}  |  Salvo em: {}",
                entry.profile.resolution.width,
                entry.profile.resolution.height,
                entry.profile.icons.len(),
                dt_str
            ),
            make_rect(right_col.left + 14, right_col.top + 32, right_col.right - 14, right_col.top + 52),
            f_small,
            pal.text_muted,
            DT_LEFT.0 | DT_VCENTER.0 | DT_SINGLELINE.0 | DT_END_ELLIPSIS.0,
        );

        // Botão Primário de Restauração (Verde #10b981)
        let btn_rest_r = make_rect(right_col.left + 14, right_col.top + 58, right_col.right - 14, right_col.top + 96);
        draw_button(
            hdc,
            btn_rest_r,
            &format!("⚡ RESTAURAR ÍCONES DE '{}'", entry.name.to_uppercase()),
            f_sub_bold,
            pal.success,
            pal.success_hover,
            rgb(255, 255, 255),
            st.hovered_id == Some(HitId::RestoreSelected),
            8,
        );
        st.hit_zones.push(HitZone {
            rect: btn_rest_r,
            id: HitId::RestoreSelected,
        });

        // Sub-botões: Atualizar, Save-Res, Excluir
        let sub_y = right_col.top + 102;
        let sub_h = 30;
        let del_w = 88;
        let avail_w = (right_col.right - right_col.left - 28) - del_w - 12;
        let half_w = avail_w / 2;

        let btn_upd_r = make_rect(right_col.left + 14, sub_y, right_col.left + 14 + half_w, sub_y + sub_h);
        draw_button(
            hdc,
            btn_upd_r,
            "🔄 Atualizar Posições (profile-update)",
            f_small_bold,
            pal.primary,
            pal.primary_hover,
            rgb(255, 255, 255),
            st.hovered_id == Some(HitId::UpdateSelected),
            6,
        );
        st.hit_zones.push(HitZone {
            rect: btn_upd_r,
            id: HitId::UpdateSelected,
        });

        let btn_res_r = make_rect(btn_upd_r.right + 6, sub_y, btn_upd_r.right + 6 + half_w, sub_y + sub_h);
        draw_button(
            hdc,
            btn_res_r,
            "📐 Gravar Resolução Atual (save-res)",
            f_small_bold,
            pal.primary,
            pal.primary_hover,
            rgb(255, 255, 255),
            st.hovered_id == Some(HitId::SaveResSelected),
            6,
        );
        st.hit_zones.push(HitZone {
            rect: btn_res_r,
            id: HitId::SaveResSelected,
        });

        let btn_del_r = make_rect(right_col.right - 14 - del_w, sub_y, right_col.right - 14, sub_y + sub_h);
        draw_button(
            hdc,
            btn_del_r,
            "🗑️ Excluir",
            f_small_bold,
            pal.danger,
            pal.danger_hover,
            rgb(255, 255, 255),
            st.hovered_id == Some(HitId::DeleteSelected),
            6,
        );
        st.hit_zones.push(HitZone {
            rect: btn_del_r,
            id: HitId::DeleteSelected,
        });

        // Caixa da Tabela de Ícones
        let tbl_box = make_rect(right_col.left + 14, sub_y + sub_h + 10, right_col.right - 14, right_col.bottom - 12);
        st.icons_area_rect = tbl_box;
        draw_round_box(hdc, tbl_box, pal.bg_elevated, pal.border, 6);

        draw_text_in(
            hdc,
            "Ícones salvos:",
            make_rect(tbl_box.left + 10, tbl_box.top + 6, tbl_box.left + 105, tbl_box.top + 32),
            f_small_bold,
            pal.text_primary,
            DT_LEFT.0 | DT_VCENTER.0 | DT_SINGLELINE.0,
        );

        let flt_r = make_rect(tbl_box.left + 108, tbl_box.top + 6, tbl_box.right - 10, tbl_box.top + 32);
        let is_flt_focus = st.focused_input == InputField::IconFilter;
        draw_round_box(
            hdc,
            flt_r,
            pal.bg_input,
            if is_flt_focus {
                pal.border_focus
            } else {
                pal.border
            },
            5,
        );
        let (flt_txt, flt_col) = if st.icon_filter_input.is_empty() && !is_flt_focus {
            ("Filtrar por nome...".to_string(), pal.text_muted)
        } else if is_flt_focus {
            (format!("{}|", st.icon_filter_input), pal.text_primary)
        } else {
            (st.icon_filter_input.clone(), pal.text_primary)
        };
        draw_text_in(
            hdc,
            &flt_txt,
            make_rect(flt_r.left + 8, flt_r.top, flt_r.right - 8, flt_r.bottom),
            f_small,
            flt_col,
            DT_LEFT.0 | DT_VCENTER.0 | DT_SINGLELINE.0 | DT_END_ELLIPSIS.0,
        );
        st.hit_zones.push(HitZone {
            rect: flt_r,
            id: HitId::Input(InputField::IconFilter),
        });

        // Linhas da tabela de ícones
        let query = st.icon_filter_input.trim().to_lowercase();
        let filtered: Vec<_> = entry
            .profile
            .icons
            .iter()
            .filter(|ic| query.is_empty() || ic.name.to_lowercase().contains(&query))
            .collect();

        let rows_top = tbl_box.top + 38;
        let rows_bottom = tbl_box.bottom - 6;
        let row_h = 24;
        let row_gap = 2;
        let max_rows = ((rows_bottom - rows_top) / (row_h + row_gap)).max(1) as usize;
        let max_i_scroll = (filtered.len() as i32 - max_rows as i32).max(0);
        st.icons_scroll = st.icons_scroll.clamp(0, max_i_scroll);

        if filtered.is_empty() {
            draw_text_in(
                hdc,
                "Nenhum ícone encontrado para o filtro atual.",
                make_rect(tbl_box.left + 10, rows_top + 16, tbl_box.right - 10, rows_top + 44),
                f_small,
                pal.text_muted,
                DT_CENTER.0 | DT_VCENTER.0 | DT_SINGLELINE.0,
            );
        } else {
            let mut ry = rows_top;
            for ic in filtered.iter().skip(st.icons_scroll as usize).take(max_rows) {
                let rr = make_rect(tbl_box.left + 8, ry, tbl_box.right - 8, ry + row_h);
                draw_round_box(hdc, rr, pal.bg_card, pal.bg_card, 4);
                draw_text_in(
                    hdc,
                    &ic.name,
                    make_rect(rr.left + 8, rr.top, rr.right - 135, rr.bottom),
                    f_small,
                    pal.text_primary,
                    DT_LEFT.0 | DT_VCENTER.0 | DT_SINGLELINE.0 | DT_END_ELLIPSIS.0,
                );
                draw_text_in(
                    hdc,
                    &format!("X: {:>5}   Y: {:>5}", ic.x, ic.y),
                    make_rect(rr.right - 135, rr.top, rr.right - 8, rr.bottom),
                    f_mono,
                    pal.text_muted,
                    DT_RIGHT.0 | DT_VCENTER.0 | DT_SINGLELINE.0,
                );
                ry += row_h + row_gap;
            }
        }
    } else {
        draw_text_in(
            hdc,
            "Nenhum perfil selecionado",
            make_rect(right_col.left + 14, right_col.top + 20, right_col.right - 14, right_col.top + 48),
            f_sec,
            pal.text_muted,
            DT_CENTER.0 | DT_VCENTER.0 | DT_SINGLELINE.0,
        );
    }
}

// ============================================================================
// ABA 2: AUTOMAÇÕES (BOTÕES ÚNICOS E DINÂMICOS DE ATIVAR / DESATIVAR)
// ============================================================================
#[allow(clippy::too_many_arguments)]
fn render_automations_tab(
    hdc: HDC,
    area: RECT,
    st: &mut GuiState,
    pal: &ThemePalette,
    f_sec: HFONT,
    f_sub_bold: HFONT,
    f_body: HFONT,
    f_small_bold: HFONT,
    f_small: HFONT,
) {
    let total_w = area.right - area.left;
    let half_w = (total_w - 12) / 2;
    let top_row_h = ((area.bottom - area.top) * 62) / 100;

    let card_hide = make_rect(area.left, area.top, area.left + half_w, area.top + top_row_h);
    let card_startup = make_rect(card_hide.right + 12, area.top, area.right, area.top + top_row_h);
    let card_drive = make_rect(area.left, card_hide.bottom + 12, area.right, area.bottom);

    draw_round_box(hdc, card_hide, pal.bg_card, pal.border, 8);
    draw_round_box(hdc, card_startup, pal.bg_card, pal.border, 8);
    draw_round_box(hdc, card_drive, pal.bg_card, pal.border, 8);

    // ------------------------------------------------------------------------
    // CARD 1: OCULTAR ÍCONES POR INATIVIDADE (HIDE-ICONS)
    // ------------------------------------------------------------------------
    draw_text_in(
        hdc,
        "👁️ Ocultar Ícones por Inatividade",
        make_rect(card_hide.left + 16, card_hide.top + 12, card_hide.right - 130, card_hide.top + 36),
        f_sec,
        pal.text_primary,
        DT_LEFT.0 | DT_VCENTER.0 | DT_SINGLELINE.0,
    );
    let (hide_st_txt, hide_st_col) = if st.hide_icons_enabled {
        (format!("● ATIVO ({}s)", st.hide_seconds), pal.success)
    } else {
        ("● DESATIVADO".to_string(), pal.text_muted)
    };
    draw_text_in(
        hdc,
        &hide_st_txt,
        make_rect(card_hide.right - 130, card_hide.top + 12, card_hide.right - 16, card_hide.top + 36),
        f_small_bold,
        hide_st_col,
        DT_RIGHT.0 | DT_VCENTER.0 | DT_SINGLELINE.0,
    );

    draw_text_in(
        hdc,
        "Oculta os ícones do desktop quando não houver atividade do usuário, restaurando-os instantaneamente ao mexer o mouse ou digitar.",
        make_rect(card_hide.left + 16, card_hide.top + 38, card_hide.right - 16, card_hide.top + 74),
        f_small,
        pal.text_muted,
        DT_LEFT.0 | DT_WORDBREAK.0,
    );

    // Slider de Tempo de Inatividade (3s a 60s)
    let sl_y = card_hide.top + 82;
    draw_text_in(
        hdc,
        "Tempo de Inatividade:",
        make_rect(card_hide.left + 16, sl_y, card_hide.left + 165, sl_y + 28),
        f_body,
        pal.text_primary,
        DT_LEFT.0 | DT_VCENTER.0 | DT_SINGLELINE.0,
    );

    let btn_dec_r = make_rect(card_hide.left + 170, sl_y + 2, card_hide.left + 196, sl_y + 26);
    draw_button(
        hdc,
        btn_dec_r,
        "-",
        f_sub_bold,
        pal.bg_elevated,
        pal.border,
        pal.text_primary,
        st.hovered_id == Some(HitId::HideSecsDec),
        5,
    );
    st.hit_zones.push(HitZone {
        rect: btn_dec_r,
        id: HitId::HideSecsDec,
    });

    let track_r = make_rect(btn_dec_r.right + 10, sl_y + 10, btn_dec_r.right + 150, sl_y + 18);
    st.slider_track_rect = make_rect(track_r.left, sl_y, track_r.right, sl_y + 28);
    draw_round_box(hdc, track_r, pal.bg_elevated, pal.border, 4);

    let ratio = (st.hide_seconds.saturating_sub(3)) as f32 / 57.0;
    let fill_w = ((track_r.right - track_r.left) as f32 * ratio) as i32;
    let active_track = make_rect(track_r.left, track_r.top, track_r.left + fill_w, track_r.bottom);
    draw_round_box(hdc, active_track, pal.primary, pal.primary, 4);

    let knob_x = track_r.left + fill_w;
    let knob_r = make_rect(knob_x - 7, sl_y + 6, knob_x + 7, sl_y + 22);
    draw_round_box(hdc, knob_r, rgb(255, 255, 255), pal.primary, 14);
    st.hit_zones.push(HitZone {
        rect: st.slider_track_rect,
        id: HitId::SliderHideTrack,
    });

    let btn_inc_r = make_rect(track_r.right + 10, sl_y + 2, track_r.right + 36, sl_y + 26);
    draw_button(
        hdc,
        btn_inc_r,
        "+",
        f_sub_bold,
        pal.bg_elevated,
        pal.border,
        pal.text_primary,
        st.hovered_id == Some(HitId::HideSecsInc),
        5,
    );
    st.hit_zones.push(HitZone {
        rect: btn_inc_r,
        id: HitId::HideSecsInc,
    });

    draw_text_in(
        hdc,
        &format!("{} seg", st.hide_seconds),
        make_rect(btn_inc_r.right + 10, sl_y, card_hide.right - 12, sl_y + 28),
        f_sub_bold,
        pal.primary,
        DT_LEFT.0 | DT_VCENTER.0 | DT_SINGLELINE.0,
    );

    // Fontes de Atividade
    let src_y = sl_y + 34;
    draw_text_in(
        hdc,
        "Fontes de Atividade:",
        make_rect(card_hide.left + 16, src_y, card_hide.left + 165, src_y + 26),
        f_body,
        pal.text_primary,
        DT_LEFT.0 | DT_VCENTER.0 | DT_SINGLELINE.0,
    );

    let chk_kb_r = make_rect(card_hide.left + 170, src_y, card_hide.left + 300, src_y + 26);
    draw_checkbox(
        hdc,
        chk_kb_r,
        "Teclado (--kb)",
        st.hide_kb,
        st.hovered_id == Some(HitId::ToggleKb),
        f_small,
        pal,
    );
    st.hit_zones.push(HitZone {
        rect: chk_kb_r,
        id: HitId::ToggleKb,
    });

    let chk_mou_r = make_rect(card_hide.left + 308, src_y, card_hide.right - 16, src_y + 26);
    draw_checkbox(
        hdc,
        chk_mou_r,
        "Mouse (--mou)",
        st.hide_mou,
        st.hovered_id == Some(HitId::ToggleMou),
        f_small,
        pal,
    );
    st.hit_zones.push(HitZone {
        rect: chk_mou_r,
        id: HitId::ToggleMou,
    });

    // Opções Extras
    let ext_y = src_y + 30;
    draw_text_in(
        hdc,
        "Recursos Avançados:",
        make_rect(card_hide.left + 16, ext_y, card_hide.left + 165, ext_y + 24),
        f_body,
        pal.text_primary,
        DT_LEFT.0 | DT_VCENTER.0 | DT_SINGLELINE.0,
    );

    let chk_cur_r = make_rect(card_hide.left + 170, ext_y, card_hide.right - 16, ext_y + 24);
    draw_checkbox(
        hdc,
        chk_cur_r,
        "Ocultar Cursor (-include-cursor)",
        st.hide_cursor,
        st.hovered_id == Some(HitId::ToggleCursor),
        f_small,
        pal,
    );
    st.hit_zones.push(HitZone {
        rect: chk_cur_r,
        id: HitId::ToggleCursor,
    });

    let chk_tb_r = make_rect(card_hide.left + 170, ext_y + 25, card_hide.right - 16, ext_y + 49);
    draw_checkbox(
        hdc,
        chk_tb_r,
        "Ocultar Barra de Tarefas (-include-taskbar)",
        st.hide_taskbar,
        st.hovered_id == Some(HitId::ToggleTaskbar),
        f_small,
        pal,
    );
    st.hit_zones.push(HitZone {
        rect: chk_tb_r,
        id: HitId::ToggleTaskbar,
    });

    // Botão Único e Dinâmico: Hide-Icons
    let btn_hide_r = make_rect(
        card_hide.left + 16,
        card_hide.bottom - 48,
        card_hide.right - 16,
        card_hide.bottom - 12,
    );
    let (hide_btn_txt, hide_bg, hide_hov) = if st.hide_icons_enabled {
        (
            format!("⏹ Desativar Ocultação Automática (Ativo: {}s)", st.hide_seconds),
            pal.danger,
            pal.danger_hover,
        )
    } else {
        (
            format!("▶ Ativar Ocultação Automática ({}s)", st.hide_seconds),
            pal.success,
            pal.success_hover,
        )
    };
    draw_button(
        hdc,
        btn_hide_r,
        &hide_btn_txt,
        f_sub_bold,
        hide_bg,
        hide_hov,
        rgb(255, 255, 255),
        st.hovered_id == Some(HitId::ToggleHideIconsBtn),
        8,
    );
    st.hit_zones.push(HitZone {
        rect: btn_hide_r,
        id: HitId::ToggleHideIconsBtn,
    });

    // ------------------------------------------------------------------------
    // CARD 2: INICIALIZAÇÃO COM O WINDOWS (STARTUP)
    // ------------------------------------------------------------------------
    draw_text_in(
        hdc,
        "🚀 Inicialização com o Windows",
        make_rect(card_startup.left + 16, card_startup.top + 12, card_startup.right - 140, card_startup.top + 36),
        f_sec,
        pal.text_primary,
        DT_LEFT.0 | DT_VCENTER.0 | DT_SINGLELINE.0,
    );
    let (st_lbl_txt, st_lbl_col) = if st.startup_enabled {
        (format!("● ATIVO ({})", st.startup_profile), pal.success)
    } else {
        ("● DESATIVADO".to_string(), pal.text_muted)
    };
    draw_text_in(
        hdc,
        &st_lbl_txt,
        make_rect(card_startup.right - 140, card_startup.top + 12, card_startup.right - 16, card_startup.top + 36),
        f_small_bold,
        st_lbl_col,
        DT_RIGHT.0 | DT_VCENTER.0 | DT_SINGLELINE.0,
    );

    draw_text_in(
        hdc,
        "Restaura a disposição dos ícones automaticamente sempre que o computador for ligado ou após fazer login no Windows.",
        make_rect(card_startup.left + 16, card_startup.top + 38, card_startup.right - 16, card_startup.top + 78),
        f_small,
        pal.text_muted,
        DT_LEFT.0 | DT_WORDBREAK.0,
    );

    let sel_y = card_startup.top + 96;
    draw_text_in(
        hdc,
        "Perfil para Inicialização:",
        make_rect(card_startup.left + 16, sel_y, card_startup.left + 190, sel_y + 32),
        f_body,
        pal.text_primary,
        DT_LEFT.0 | DT_VCENTER.0 | DT_SINGLELINE.0,
    );

    // Seletor de perfil com botões ◀ e ▶
    let prev_st_r = make_rect(card_startup.left + 195, sel_y, card_startup.left + 227, sel_y + 32);
    let next_st_r = make_rect(card_startup.right - 48, sel_y, card_startup.right - 16, sel_y + 32);
    let mid_st_r = make_rect(prev_st_r.right + 6, sel_y, next_st_r.left - 6, sel_y + 32);

    draw_button(
        hdc,
        prev_st_r,
        "◀",
        f_small_bold,
        pal.bg_elevated,
        pal.border,
        pal.text_primary,
        st.hovered_id == Some(HitId::StartupPrevProfile),
        6,
    );
    st.hit_zones.push(HitZone {
        rect: prev_st_r,
        id: HitId::StartupPrevProfile,
    });

    draw_round_box(hdc, mid_st_r, pal.bg_input, pal.border, 6);
    draw_text_in(
        hdc,
        &format!("📁 {}", st.startup_profile),
        mid_st_r,
        f_sub_bold,
        pal.text_primary,
        DT_CENTER.0 | DT_VCENTER.0 | DT_SINGLELINE.0 | DT_END_ELLIPSIS.0,
    );
    st.hit_zones.push(HitZone {
        rect: mid_st_r,
        id: HitId::StartupNextProfile,
    });

    draw_button(
        hdc,
        next_st_r,
        "▶",
        f_small_bold,
        pal.bg_elevated,
        pal.border,
        pal.text_primary,
        st.hovered_id == Some(HitId::StartupNextProfile),
        6,
    );
    st.hit_zones.push(HitZone {
        rect: next_st_r,
        id: HitId::StartupNextProfile,
    });

    // Botão Único e Dinâmico: Startup
    let btn_st_r = make_rect(
        card_startup.left + 16,
        card_startup.bottom - 48,
        card_startup.right - 16,
        card_startup.bottom - 12,
    );
    let (st_btn_txt, st_bg, st_hov) = if st.startup_enabled {
        (
            format!(
                "⏹ Desativar Inicialização com o Windows (Perfil: {})",
                st.startup_profile
            ),
            pal.danger,
            pal.danger_hover,
        )
    } else {
        (
            format!(
                "▶ Ativar no Início do Windows (Perfil: {})",
                st.startup_profile
            ),
            pal.success,
            pal.success_hover,
        )
    };
    draw_button(
        hdc,
        btn_st_r,
        &st_btn_txt,
        f_sub_bold,
        st_bg,
        st_hov,
        rgb(255, 255, 255),
        st.hovered_id == Some(HitId::ToggleStartupBtn),
        8,
    );
    st.hit_zones.push(HitZone {
        rect: btn_st_r,
        id: HitId::ToggleStartupBtn,
    });

    // ------------------------------------------------------------------------
    // CARD 3: SINCRONIZAÇÃO EM NUVEM (GOOGLE DRIVE / WAIT-DRIVE)
    // ------------------------------------------------------------------------
    draw_text_in(
        hdc,
        "☁️ Sincronização em Nuvem (Google Drive / Wait-Drive)",
        make_rect(card_drive.left + 16, card_drive.top + 10, card_drive.right - 160, card_drive.top + 34),
        f_sec,
        pal.text_primary,
        DT_LEFT.0 | DT_VCENTER.0 | DT_SINGLELINE.0,
    );
    let (dr_lbl_txt, dr_lbl_col) = if st.wait_drive_enabled {
        (format!("● ATIVO ({})", st.wait_drive_profile), pal.success)
    } else {
        ("● DESATIVADO".to_string(), pal.text_muted)
    };
    draw_text_in(
        hdc,
        &dr_lbl_txt,
        make_rect(card_drive.right - 160, card_drive.top + 10, card_drive.right - 16, card_drive.top + 34),
        f_small_bold,
        dr_lbl_col,
        DT_RIGHT.0 | DT_VCENTER.0 | DT_SINGLELINE.0,
    );

    draw_text_in(
        hdc,
        "Se a pasta Desktop for sincronizada em nuvem (Google Drive), aguarda a sincronização completar antes de reorganizar os ícones.",
        make_rect(card_drive.left + 16, card_drive.top + 34, card_drive.right - 16, card_drive.top + 56),
        f_small,
        pal.text_muted,
        DT_LEFT.0 | DT_VCENTER.0 | DT_SINGLELINE.0 | DT_END_ELLIPSIS.0,
    );

    let dc_y = card_drive.bottom - 46;
    draw_text_in(
        hdc,
        "Perfil:",
        make_rect(card_drive.left + 16, dc_y, card_drive.left + 64, dc_y + 34),
        f_body,
        pal.text_primary,
        DT_LEFT.0 | DT_VCENTER.0 | DT_SINGLELINE.0,
    );

    let dp_prev = make_rect(card_drive.left + 68, dc_y + 2, card_drive.left + 96, dc_y + 32);
    let dp_mid = make_rect(dp_prev.right + 4, dc_y + 2, dp_prev.right + 144, dc_y + 32);
    let dp_next = make_rect(dp_mid.right + 4, dc_y + 2, dp_mid.right + 32, dc_y + 32);

    draw_button(
        hdc,
        dp_prev,
        "◀",
        f_small_bold,
        pal.bg_elevated,
        pal.border,
        pal.text_primary,
        st.hovered_id == Some(HitId::DrivePrevProfile),
        5,
    );
    st.hit_zones.push(HitZone {
        rect: dp_prev,
        id: HitId::DrivePrevProfile,
    });

    draw_round_box(hdc, dp_mid, pal.bg_input, pal.border, 6);
    draw_text_in(
        hdc,
        &format!("📁 {}", st.wait_drive_profile),
        dp_mid,
        f_sub_bold,
        pal.text_primary,
        DT_CENTER.0 | DT_VCENTER.0 | DT_SINGLELINE.0 | DT_END_ELLIPSIS.0,
    );
    st.hit_zones.push(HitZone {
        rect: dp_mid,
        id: HitId::DriveNextProfile,
    });

    draw_button(
        hdc,
        dp_next,
        "▶",
        f_small_bold,
        pal.bg_elevated,
        pal.border,
        pal.text_primary,
        st.hovered_id == Some(HitId::DriveNextProfile),
        5,
    );
    st.hit_zones.push(HitZone {
        rect: dp_next,
        id: HitId::DriveNextProfile,
    });

    // Timeout (segundos)
    draw_text_in(
        hdc,
        "Timeout (seg):",
        make_rect(dp_next.right + 16, dc_y, dp_next.right + 115, dc_y + 34),
        f_body,
        pal.text_primary,
        DT_LEFT.0 | DT_VCENTER.0 | DT_SINGLELINE.0,
    );

    let dt_dec = make_rect(dp_next.right + 118, dc_y + 2, dp_next.right + 144, dc_y + 32);
    let dt_inp = make_rect(dt_dec.right + 4, dc_y + 2, dt_dec.right + 68, dc_y + 32);
    let dt_inc = make_rect(dt_inp.right + 4, dc_y + 2, dt_inp.right + 30, dc_y + 32);

    draw_button(
        hdc,
        dt_dec,
        "-",
        f_sub_bold,
        pal.bg_elevated,
        pal.border,
        pal.text_primary,
        st.hovered_id == Some(HitId::DriveTimeoutDec),
        5,
    );
    st.hit_zones.push(HitZone {
        rect: dt_dec,
        id: HitId::DriveTimeoutDec,
    });

    let is_dt_focus = st.focused_input == InputField::DriveTimeout;
    draw_round_box(
        hdc,
        dt_inp,
        pal.bg_input,
        if is_dt_focus {
            pal.border_focus
        } else {
            pal.border
        },
        5,
    );
    let dt_txt = if is_dt_focus {
        format!("{}|", st.drive_timeout_input)
    } else {
        st.drive_timeout_input.clone()
    };
    draw_text_in(
        hdc,
        &dt_txt,
        dt_inp,
        f_sub_bold,
        pal.text_primary,
        DT_CENTER.0 | DT_VCENTER.0 | DT_SINGLELINE.0,
    );
    st.hit_zones.push(HitZone {
        rect: dt_inp,
        id: HitId::Input(InputField::DriveTimeout),
    });

    draw_button(
        hdc,
        dt_inc,
        "+",
        f_sub_bold,
        pal.bg_elevated,
        pal.border,
        pal.text_primary,
        st.hovered_id == Some(HitId::DriveTimeoutInc),
        5,
    );
    st.hit_zones.push(HitZone {
        rect: dt_inc,
        id: HitId::DriveTimeoutInc,
    });

    // Botão Único e Dinâmico: Wait-Drive
    let btn_dr_r = make_rect(dt_inc.right + 16, dc_y, card_drive.right - 16, dc_y + 34);
    let t_disp = if st.drive_timeout_input.trim().is_empty() {
        "sem limite".to_string()
    } else {
        format!("{}s", st.drive_timeout_input.trim())
    };
    let (dr_btn_txt, dr_bg, dr_hov) = if st.wait_drive_enabled {
        (
            format!(
                "⏹ Desativar Wait-Drive (Ativo: {}, {})",
                st.wait_drive_profile, t_disp
            ),
            pal.danger,
            pal.danger_hover,
        )
    } else {
        (
            format!("▶ Ativar Wait-Drive (Perfil: {})", st.wait_drive_profile),
            pal.success,
            pal.success_hover,
        )
    };
    draw_button(
        hdc,
        btn_dr_r,
        &dr_btn_txt,
        f_sub_bold,
        dr_bg,
        dr_hov,
        rgb(255, 255, 255),
        st.hovered_id == Some(HitId::ToggleDriveBtn),
        8,
    );
    st.hit_zones.push(HitZone {
        rect: btn_dr_r,
        id: HitId::ToggleDriveBtn,
    });
}

// ============================================================================
// ABA 3: IDIOMA & SISTEMA
// ============================================================================
#[allow(clippy::too_many_arguments)]
fn render_settings_tab(
    hdc: HDC,
    area: RECT,
    st: &mut GuiState,
    pal: &ThemePalette,
    f_sec: HFONT,
    f_sub_bold: HFONT,
    f_body: HFONT,
    f_mono: HFONT,
) {
    draw_round_box(hdc, area, pal.bg_card, pal.border, 8);

    draw_text_in(
        hdc,
        "Preferências e Sistema",
        make_rect(area.left + 20, area.top + 16, area.right - 20, area.top + 42),
        f_sec,
        pal.text_primary,
        DT_LEFT.0 | DT_VCENTER.0 | DT_SINGLELINE.0,
    );

    // Seletor de Idioma
    let lang_y = area.top + 56;
    draw_text_in(
        hdc,
        "Idioma do CLI:",
        make_rect(area.left + 20, lang_y, area.left + 220, lang_y + 32),
        f_body,
        pal.text_primary,
        DT_LEFT.0 | DT_VCENTER.0 | DT_SINGLELINE.0,
    );

    let langs = ["pt-br", "en-us", "es", "ru", "zh"];
    let mut lx = area.left + 230;
    for l in langs {
        let lr = make_rect(lx, lang_y, lx + 72, lang_y + 32);
        let is_sel = st.selected_lang == l;
        let is_hov = st.hovered_id == Some(HitId::SelectLang(l.to_string()));
        let bg = if is_sel {
            pal.primary
        } else if is_hov {
            pal.border
        } else {
            pal.bg_elevated
        };
        let fg = if is_sel {
            rgb(255, 255, 255)
        } else {
            pal.text_primary
        };
        draw_round_box(hdc, lr, bg, pal.border, 6);
        draw_text_in(
            hdc,
            l,
            lr,
            f_sub_bold,
            fg,
            DT_CENTER.0 | DT_VCENTER.0 | DT_SINGLELINE.0,
        );
        st.hit_zones.push(HitZone {
            rect: lr,
            id: HitId::SelectLang(l.to_string()),
        });
        lx += 80;
    }

    let btn_save_lang = make_rect(lx + 12, lang_y, lx + 145, lang_y + 32);
    draw_button(
        hdc,
        btn_save_lang,
        "Salvar Idioma",
        f_sub_bold,
        pal.success,
        pal.success_hover,
        rgb(255, 255, 255),
        st.hovered_id == Some(HitId::SaveLangBtn),
        6,
    );
    st.hit_zones.push(HitZone {
        rect: btn_save_lang,
        id: HitId::SaveLangBtn,
    });

    // Informações do Sistema
    let info_y = lang_y + 52;
    draw_text_in(
        hdc,
        "Executável do DesktopORZ:",
        make_rect(area.left + 20, info_y, area.left + 220, info_y + 28),
        f_body,
        pal.text_primary,
        DT_LEFT.0 | DT_VCENTER.0 | DT_SINGLELINE.0,
    );
    let bin_str = st
        .binary_path
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "Não encontrado".to_string());
    draw_text_in(
        hdc,
        &bin_str,
        make_rect(area.left + 230, info_y, area.right - 20, info_y + 28),
        f_mono,
        pal.text_muted,
        DT_LEFT.0 | DT_VCENTER.0 | DT_SINGLELINE.0 | DT_END_ELLIPSIS.0,
    );

    let res = shell_locator::current_resolution();
    draw_text_in(
        hdc,
        "Resolução Atual da Tela:",
        make_rect(area.left + 20, info_y + 36, area.left + 220, info_y + 64),
        f_body,
        pal.text_primary,
        DT_LEFT.0 | DT_VCENTER.0 | DT_SINGLELINE.0,
    );
    draw_text_in(
        hdc,
        &format!("{}x{} pixels", res.width, res.height),
        make_rect(area.left + 230, info_y + 36, area.right - 20, info_y + 64),
        f_mono,
        pal.text_muted,
        DT_LEFT.0 | DT_VCENTER.0 | DT_SINGLELINE.0,
    );

    draw_text_in(
        hdc,
        "Pastas de Perfis:",
        make_rect(area.left + 20, info_y + 72, area.left + 220, info_y + 100),
        f_body,
        pal.text_primary,
        DT_LEFT.0 | DT_VCENTER.0 | DT_SINGLELINE.0,
    );
    let p_dirs: Vec<String> = get_all_profiles_dirs()
        .into_iter()
        .map(|d| d.display().to_string())
        .collect();
    draw_text_in(
        hdc,
        &p_dirs.join("\n"),
        make_rect(area.left + 230, info_y + 76, area.right - 20, info_y + 130),
        f_mono,
        pal.text_muted,
        DT_LEFT.0 | DT_WORDBREAK.0,
    );

    // Botão Ajuda do CLI
    let help_btn = make_rect(area.left + 20, info_y + 145, area.left + 300, info_y + 181);
    draw_button(
        hdc,
        help_btn,
        "📖 Exibir Ajuda Completa do CLI (help)",
        f_sub_bold,
        pal.primary,
        pal.primary_hover,
        rgb(255, 255, 255),
        st.hovered_id == Some(HitId::CliHelpBtn),
        6,
    );
    st.hit_zones.push(HitZone {
        rect: help_btn,
        id: HitId::CliHelpBtn,
    });
}

// ============================================================================
// COPIAR PARA CLIPBOARD DO WINDOWS
// ============================================================================
fn copy_to_clipboard(hwnd: HWND, text: &str) {
    unsafe {
        if OpenClipboard(hwnd).is_err() {
            return;
        }
        let _ = EmptyClipboard();
        let mut wide: Vec<u16> = text.encode_utf16().collect();
        wide.push(0);
        let bytes_len = wide.len() * 2;
        if let Ok(hmem) = GlobalAlloc(GMEM_MOVEABLE, bytes_len) {
            let ptr = GlobalLock(hmem);
            if !ptr.is_null() {
                std::ptr::copy_nonoverlapping(wide.as_ptr() as *const u8, ptr as *mut u8, bytes_len);
                let _ = GlobalUnlock(hmem);
                let _ = SetClipboardData(CF_UNICODETEXT_VAL, HANDLE(hmem.0));
            }
        }
        let _ = CloseClipboard();
    }
}

// ============================================================================
// TRATAMENTO DE EVENTOS E CLIQUES
// ============================================================================
fn update_slider_from_mouse(st: &mut GuiState, mouse_x: i32) {
    let tr = st.slider_track_rect;
    let width = (tr.right - tr.left).max(1);
    let clamped = (mouse_x - tr.left).clamp(0, width);
    let secs = 3 + ((clamped as f32 / width as f32) * 57.0).round() as u64;
    st.hide_seconds = secs.clamp(3, 60);
}

fn handle_click(hwnd: HWND, hit: HitId, mouse_x: i32, st: &mut GuiState) {
    match hit {
        HitId::ThemeToggle => {
            st.dark_mode = !st.dark_mode;
        }
        HitId::Tab(tab) => {
            st.active_tab = tab;
            st.focused_input = InputField::None;
        }
        HitId::Input(field) => {
            st.focused_input = field;
        }
        HitId::SaveNewProfile => {
            let name = st.new_profile_input.trim().to_string();
            if name.is_empty() {
                st.append_log("⚠ Digite um nome válido para o novo perfil.");
                return;
            }
            if name.chars().any(|c| "\\/:*?\"<>|".contains(c)) {
                st.append_log("⚠ O nome do perfil contém caracteres inválidos.");
                return;
            }
            st.new_profile_input.clear();
            st.focused_input = InputField::None;
            st.selected_profile = Some(name.clone());
            st.execute_cli_action(&["save", &name]);
        }
        HitId::SelectProfile(name) => {
            st.selected_profile = Some(name);
            st.icons_scroll = 0;
        }
        HitId::RestoreSelected => {
            if let Some(name) = st.selected_profile.clone() {
                st.execute_cli_action(&["restore", &name]);
            }
        }
        HitId::UpdateSelected => {
            if let Some(name) = st.selected_profile.clone() {
                st.execute_cli_action(&["profile-update", &name]);
            }
        }
        HitId::SaveResSelected => {
            if let Some(name) = st.selected_profile.clone() {
                st.execute_cli_action(&["save-res", &name]);
            }
        }
        HitId::DeleteSelected => {
            if let Some(name) = st.selected_profile.clone() {
                let msg: Vec<u16> = format!("Deseja realmente excluir o perfil '{name}'?\0")
                    .encode_utf16()
                    .collect();
                let cap: Vec<u16> = "Confirmar Exclusão\0".encode_utf16().collect();
                let resp = unsafe {
                    MessageBoxW(
                        hwnd,
                        PCWSTR(msg.as_ptr()),
                        PCWSTR(cap.as_ptr()),
                        MB_YESNO | MB_ICONQUESTION,
                    )
                };
                if resp == IDYES {
                    for d in get_all_profiles_dirs() {
                        let _ = fs::remove_file(d.join(format!("{name}.json")));
                    }
                    let _ = fs::remove_file(project_dir().join(format!("{name}.json")));
                    st.append_log(&format!("🗑️ Perfil '{name}' removido com sucesso."));
                    st.selected_profile = None;
                    st.refresh_all_data(false);
                }
            }
        }
        HitId::SliderHideTrack => {
            st.dragging_slider = true;
            unsafe {
                SetCapture(hwnd);
            }
            update_slider_from_mouse(st, mouse_x);
        }
        HitId::HideSecsDec => {
            st.hide_seconds = st.hide_seconds.saturating_sub(1).max(3);
            if st.hide_icons_enabled {
                st.apply_hide_icons_on();
            }
        }
        HitId::HideSecsInc => {
            st.hide_seconds = (st.hide_seconds + 1).min(60);
            if st.hide_icons_enabled {
                st.apply_hide_icons_on();
            }
        }
        HitId::ToggleKb => {
            st.hide_kb = !st.hide_kb;
            if !st.hide_kb && !st.hide_mou {
                st.hide_mou = true;
            }
            if st.hide_icons_enabled {
                st.apply_hide_icons_on();
            }
        }
        HitId::ToggleMou => {
            st.hide_mou = !st.hide_mou;
            if !st.hide_kb && !st.hide_mou {
                st.hide_kb = true;
            }
            if st.hide_icons_enabled {
                st.apply_hide_icons_on();
            }
        }
        HitId::ToggleCursor => {
            st.hide_cursor = !st.hide_cursor;
            if st.hide_icons_enabled {
                st.apply_hide_icons_on();
            }
        }
        HitId::ToggleTaskbar => {
            st.hide_taskbar = !st.hide_taskbar;
            if st.hide_icons_enabled {
                st.apply_hide_icons_on();
            }
        }
        HitId::ToggleHideIconsBtn => {
            if st.hide_icons_enabled {
                st.execute_cli_action(&["hide-icons", "off"]);
            } else {
                st.apply_hide_icons_on();
            }
        }
        HitId::StartupPrevProfile => {
            st.startup_profile = st.cycle_profile(&st.startup_profile, false);
            if st.startup_enabled {
                st.apply_startup_on();
            }
        }
        HitId::StartupNextProfile => {
            st.startup_profile = st.cycle_profile(&st.startup_profile, true);
            if st.startup_enabled {
                st.apply_startup_on();
            }
        }
        HitId::ToggleStartupBtn => {
            if st.startup_enabled {
                st.execute_cli_action(&["startup", "off"]);
            } else {
                st.apply_startup_on();
            }
        }
        HitId::DrivePrevProfile => {
            st.wait_drive_profile = st.cycle_profile(&st.wait_drive_profile, false);
            if st.wait_drive_enabled {
                st.apply_drive_on();
            }
        }
        HitId::DriveNextProfile => {
            st.wait_drive_profile = st.cycle_profile(&st.wait_drive_profile, true);
            if st.wait_drive_enabled {
                st.apply_drive_on();
            }
        }
        HitId::DriveTimeoutDec => {
            let cur: u64 = st.drive_timeout_input.trim().parse().unwrap_or(30);
            st.drive_timeout_input = cur.saturating_sub(5).max(5).to_string();
            if st.wait_drive_enabled {
                st.apply_drive_on();
            }
        }
        HitId::DriveTimeoutInc => {
            let cur: u64 = st.drive_timeout_input.trim().parse().unwrap_or(30);
            st.drive_timeout_input = (cur + 5).min(600).to_string();
            if st.wait_drive_enabled {
                st.apply_drive_on();
            }
        }
        HitId::ToggleDriveBtn => {
            if st.wait_drive_enabled {
                st.execute_cli_action(&["wait-drive", "off"]);
            } else {
                st.apply_drive_on();
            }
        }
        HitId::SelectLang(lang) => {
            st.selected_lang = lang;
        }
        HitId::SaveLangBtn => {
            let lang = st.selected_lang.clone();
            st.execute_cli_action(&["lang", &lang]);
        }
        HitId::CliHelpBtn => {
            st.execute_cli_action(&["help"]);
        }
        HitId::RefreshAllBtn => {
            st.refresh_all_data(true);
        }
        HitId::CopyConsoleBtn => {
            let full = st.console_lines.join("\r\n");
            copy_to_clipboard(hwnd, &full);
            st.exec_status = "📋 Saída copiada para a área de transferência".to_string();
            st.exec_status_ok = Some(true);
        }
        HitId::ClearConsoleBtn => {
            st.console_lines.clear();
            st.console_scroll = 0;
            st.exec_status = "Pronto.".to_string();
            st.exec_status_ok = None;
        }
    }
}

// ============================================================================
// WINDOW PROCEDURE (WIN32)
// ============================================================================
unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_CREATE => LRESULT(0),
        WM_ERASEBKGND => LRESULT(1), // Evita flicker; pintamos tudo no double buffer
        WM_SIZE => {
            let _ = InvalidateRect(hwnd, None, false);
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            let x = (lparam.0 & 0xFFFF) as i16 as i32;
            let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
            let mut need_repaint = false;
            let mut is_interactive = false;

            if let Ok(mut st) = state().lock() {
                if st.dragging_slider {
                    update_slider_from_mouse(&mut st, x);
                    need_repaint = true;
                    is_interactive = true;
                } else {
                    let found = st
                        .hit_zones
                        .iter()
                        .rev()
                        .find(|z| z.contains(x, y))
                        .map(|z| z.id.clone());
                    is_interactive = found.is_some();
                    if st.hovered_id != found {
                        st.hovered_id = found;
                        need_repaint = true;
                    }
                }
            }

            if is_interactive {
                if let Ok(cur) = LoadCursorW(None, IDC_HAND) {
                    windows::Win32::UI::WindowsAndMessaging::SetCursor(cur);
                }
            } else if let Ok(cur) = LoadCursorW(None, IDC_ARROW) {
                windows::Win32::UI::WindowsAndMessaging::SetCursor(cur);
            }

            if need_repaint {
                let _ = InvalidateRect(hwnd, None, false);
            }
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            let x = (lparam.0 & 0xFFFF) as i16 as i32;
            let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
            if let Ok(mut st) = state().lock() {
                let hit = st
                    .hit_zones
                    .iter()
                    .rev()
                    .find(|z| z.contains(x, y))
                    .map(|z| z.id.clone());
                if let Some(h) = hit {
                    if !matches!(h, HitId::Input(_)) {
                        st.focused_input = InputField::None;
                    }
                    handle_click(hwnd, h, x, &mut st);
                } else {
                    st.focused_input = InputField::None;
                }
            }
            let _ = InvalidateRect(hwnd, None, false);
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            let mut finished_drag = false;
            if let Ok(mut st) = state().lock() {
                if st.dragging_slider {
                    st.dragging_slider = false;
                    finished_drag = true;
                    let _ = ReleaseCapture();
                    if st.hide_icons_enabled {
                        st.apply_hide_icons_on();
                    }
                }
            }
            if finished_drag {
                let _ = InvalidateRect(hwnd, None, false);
            }
            LRESULT(0)
        }
        WM_MOUSEWHEEL => {
            let delta = ((wparam.0 >> 16) & 0xFFFF) as i16;
            let step = if delta > 0 { -2 } else { 2 };

            let mut pt = POINT {
                x: (lparam.0 & 0xFFFF) as i16 as i32,
                y: ((lparam.0 >> 16) & 0xFFFF) as i16 as i32,
            };
            let _ = windows::Win32::Graphics::Gdi::ScreenToClient(hwnd, &mut pt);

            if let Ok(mut st) = state().lock() {
                let in_rect = |r: RECT, p: POINT| {
                    p.x >= r.left && p.x <= r.right && p.y >= r.top && p.y <= r.bottom
                };
                if in_rect(st.console_area_rect, pt) {
                    st.console_scroll = (st.console_scroll + step).max(0);
                } else if st.active_tab == ActiveTab::Profiles && in_rect(st.icons_area_rect, pt) {
                    st.icons_scroll = (st.icons_scroll + step).max(0);
                } else if st.active_tab == ActiveTab::Profiles && in_rect(st.profiles_area_rect, pt)
                {
                    st.profiles_scroll = (st.profiles_scroll + step).max(0);
                }
            }
            let _ = InvalidateRect(hwnd, None, false);
            LRESULT(0)
        }
        WM_CHAR => {
            let ch = (wparam.0 as u32) as u16;
            if let Some(c) = char::from_u32(ch as u32) {
                if let Ok(mut st) = state().lock() {
                    match st.focused_input {
                        InputField::NewProfileName => {
                            if c == '\u{8}' {
                                st.new_profile_input.pop();
                            } else if c == '\r' || c == '\n' {
                                let name = st.new_profile_input.trim().to_string();
                                if !name.is_empty() {
                                    st.new_profile_input.clear();
                                    st.focused_input = InputField::None;
                                    st.selected_profile = Some(name.clone());
                                    st.execute_cli_action(&["save", &name]);
                                }
                            } else if !c.is_control() && st.new_profile_input.len() < 48 {
                                st.new_profile_input.push(c);
                            }
                        }
                        InputField::IconFilter => {
                            if c == '\u{8}' {
                                st.icon_filter_input.pop();
                                st.icons_scroll = 0;
                            } else if !c.is_control() && st.icon_filter_input.len() < 48 {
                                st.icon_filter_input.push(c);
                                st.icons_scroll = 0;
                            }
                        }
                        InputField::DriveTimeout => {
                            if c == '\u{8}' {
                                st.drive_timeout_input.pop();
                            } else if c.is_ascii_digit() && st.drive_timeout_input.len() < 5 {
                                st.drive_timeout_input.push(c);
                            }
                        }
                        InputField::None => {}
                    }
                }
                let _ = InvalidateRect(hwnd, None, false);
            }
            LRESULT(0)
        }
        WM_KEYDOWN => {
            let vk = wparam.0 as u32;
            // VK_ESCAPE = 0x1B
            if vk == 0x1B {
                if let Ok(mut st) = state().lock() {
                    st.focused_input = InputField::None;
                }
                let _ = InvalidateRect(hwnd, None, false);
            }
            LRESULT(0)
        }
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);
            let mut rc = RECT::default();
            let _ = GetClientRect(hwnd, &mut rc);
            let w = (rc.right - rc.left).max(1);
            let h = (rc.bottom - rc.top).max(1);

            // Double buffering para zero flicker
            let mem_dc = CreateCompatibleDC(hdc);
            let mem_bmp = CreateCompatibleBitmap(hdc, w, h);
            let old_bmp = SelectObject(mem_dc, mem_bmp);

            if let Ok(mut st) = state().lock() {
                render_gui(mem_dc, w, h, &mut st);
            }

            let _ = BitBlt(hdc, 0, 0, w, h, mem_dc, 0, 0, SRCCOPY);

            SelectObject(mem_dc, old_bmp);
            let _ = DeleteObject(mem_bmp);
            let _ = DeleteDC(mem_dc);
            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

/// Ponto de entrada público para iniciar a interface gráfica nativa em Rust.
pub fn run_gui() -> Result<(), String> {
    unsafe {
        let hinstance = GetModuleHandleW(None).map_err(|e| format!("{e}"))?;
        let class_name = w!("DesktopORZ_RustGUI_Class");

        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wnd_proc),
            hInstance: hinstance.into(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            lpszClassName: class_name,
            ..Default::default()
        };

        RegisterClassW(&wc);

        let hwnd = CreateWindowExW(
            Default::default(),
            class_name,
            w!("DesktopORZ — Gerenciador de Ícones da Área de Trabalho (Rust GUI)"),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE | WS_CLIPCHILDREN,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            1080,
            780,
            None,
            None,
            hinstance,
            None,
        )
        .map_err(|e| format!("Falha ao criar janela: {e}"))?;

        let _ = ShowWindow(hwnd, SW_SHOW);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).into() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_timestamp_zero() {
        assert_eq!(format_timestamp(0), "Data desc.");
    }

    #[test]
    fn test_format_timestamp_valid() {
        let formatted = format_timestamp(1700000000);
        assert!(formatted.contains("/2023 "));
    }

    #[test]
    fn test_theme_palettes_distinct() {
        let dark = ThemePalette::dark();
        let light = ThemePalette::light();
        assert_ne!(dark.bg_main, light.bg_main);
        assert_eq!(dark.success, light.success);
        assert_eq!(dark.danger, light.danger);
    }
}