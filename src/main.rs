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
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use desktoporz::{
    config, hide_icons, i18n, layout, process_watcher, shell_locator, startup, wait_drive,
};
use desktoporz::i18n::{t, t_args};
use desktoporz::types::{DesktopProfile, Resolution};

const PROFILES_DIR: &str = "profiles";

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    match run(&args) {
        Ok(message) => {
            println!("{message}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{}", t_args("errors.generic", &[("error", &error)]));
            ExitCode::FAILURE
        }
    }
}

fn run(args: &[String]) -> Result<String, String> {
    match args.first().map(String::as_str) {
        Some("save") => {
            let name = profile_name_arg(args)?;
            let current = shell_locator::current_resolution();
            let mut profile = layout::save_layout(&name, current).map_err(|e| format!("{e}"))?;
            if let Ok(existing) = load_profile(&name) {
                profile.resolutions = existing.resolutions;
            }
            save_profile(&profile)?;
            Ok(t_args(
                "save.success",
                &[
                    ("name", &profile.profile_name),
                    ("count", &profile.icons.len().to_string()),
                    ("width", &profile.resolution.width.to_string()),
                    ("height", &profile.resolution.height.to_string()),
                ],
            ))
        }
        Some("save-res") => {
            let name = profile_name_arg(args)?;
            let resolution = match args.get(2) {
                Some(raw) => parse_resolution(raw)?,
                None => shell_locator::current_resolution(),
            };
            let current = layout::save_layout(&name, resolution).map_err(|e| format!("{e}"))?;
            let key = format!("{}x{}", resolution.width, resolution.height);
            let count = current.icons.len();
            let mut profile = match load_profile(&name) {
                Ok(existing) => existing,
                Err(_) => current.clone(),
            };
            profile.resolutions.insert(key.clone(), current.icons);
            save_profile(&profile)?;
            Ok(t_args(
                "save_res.success",
                &[
                    ("name", &name),
                    ("count", &count.to_string()),
                    ("resolution", &key),
                ],
            ))
        }
        Some("profile-update") => {
            let name = profile_name_arg(args)?;
            let mut profile = load_profile(&name)
                .map_err(|_| t_args("profile_update.not_found", &[("name", &name)]))?;
            let current = shell_locator::current_resolution();
            let captured = layout::save_layout(&name, current).map_err(|e| format!("{e}"))?;
            profile.resolution = captured.resolution;
            profile.timestamp_utc = captured.timestamp_utc;
            profile.icons = captured.icons.clone();
            let key = format!("{}x{}", current.width, current.height);
            if profile.resolutions.contains_key(&key) {
                profile.resolutions.insert(key.clone(), captured.icons);
            }
            let count = profile.icons.len();
            save_profile(&profile)?;
            Ok(t_args(
                "profile_update.success",
                &[
                    ("name", &name),
                    ("count", &count.to_string()),
                    ("width", &current.width.to_string()),
                    ("height", &current.height.to_string()),
                ],
            ))
        }
        Some("restore") => {
            let name = profile_name_arg(args)?;
            let profile = load_profile(&name)?;
            let _ = shell_locator::refresh_desktop();
            let moved = layout::restore_layout(&profile).map_err(|e| format!("{e}"))?;
            // Rede de segurança: se este `restore` veio do login (entrada
            // desk0k), o monitor de ocultar-ícones sobe junto, quando ativo.
            if hide_icons::load_config().enabled {
                hide_icons::spawn_daemon();
            }
            Ok(t_args(
                "restore.success",
                &[
                    ("name", &name),
                    ("moved", &moved.to_string()),
                    ("count", &profile.icons.len().to_string()),
                ],
            ))
        }
        Some("list") => {
            let profiles = list_profiles()?;
            if profiles.is_empty() {
                Ok(t("list.empty"))
            } else {
                Ok(t_args("list.some", &[("list", &profiles.join("\n"))]))
            }
        }
        Some("lang") => run_lang(args),
        Some("startup") => match args.get(1).map(String::as_str) {
            Some("on") => {
                let name = args
                    .get(2)
                    .filter(|n| !n.trim().is_empty())
                    .ok_or_else(|| t("startup.missing_profile"))?;
                if profile_path(name).exists() {
                    startup::enable(name)
                } else {
                    startup::enable(name).map(|msg| {
                        format!(
                            "{msg}\n{}",
                            t_args("startup.warn_profile_missing", &[("name", name)])
                        )
                    })
                }
            }
            Some("off") => startup::disable(),
            Some("status") | None => startup::status(),
            Some(_) => Err(t("startup.usage")),
        },
        Some("wait-drive") => match args.get(1).map(String::as_str) {
            Some("on") | Some("true") => {
                let nome = args
                    .get(2)
                    .filter(|n| !n.trim().is_empty())
                    .ok_or_else(|| t("wait_drive.missing_profile"))?;
                validate_profile_name(nome)?;
                let timeout = timeout_arg(args)?;
                if profile_path(nome).exists() {
                    wait_drive::enable(nome, timeout.map(|t| t.as_secs()))
                } else {
                    wait_drive::enable(nome, timeout.map(|t| t.as_secs())).map(|msg| {
                        format!(
                            "{msg}\n{}",
                            t_args("wait_drive.warn_profile_missing", &[("name", nome)])
                        )
                    })
                }
            }
            Some("off") | Some("false") => wait_drive::disable(),
            Some("status") | None => wait_drive::status(),
            Some("run") => wait_drive_run(),
            Some(_) => Err(t("wait_drive.usage")),
        },
        Some("hide-icons") => match args.get(1).map(String::as_str) {
            Some("on") => {
                let segundos: u64 = args
                    .get(2)
                    .and_then(|v| v.parse().ok())
                    .filter(|s| *s > 0)
                    .ok_or_else(|| t("hide_icons.usage"))?;
                hide_icons::enable(segundos)
            }
            Some("off") => hide_icons::disable(),
            Some("status") | None => hide_icons::status(),
            Some("run") => hide_icons::run(),
            Some(_) => Err(t("hide_icons.usage")),
        },
        Some("help") | Some("--help") | Some("-h") | None => Ok(help_text()),
        Some(outro) => Err(t_args(
            "errors.unknown_command",
            &[("command", outro), ("help", &help_text())],
        )),
    }
}

/// Comando `lang [sigla]`: sem argumento mostra o idioma atual; com
/// argumento valida se `langs/<sigla>.json` existe ao lado do exe e o salva.
fn run_lang(args: &[String]) -> Result<String, String> {
    match args
        .get(1)
        .map(String::as_str)
        .filter(|s| !s.trim().is_empty())
    {
        None => Ok(t_args("lang.current", &[("lang", &config::get_language())])),
        Some(sigla) => {
            let path = exe_dir().join("langs").join(format!("{sigla}.json"));
            if !path.exists() && i18n::embedded_language(sigla).is_none() {
                return Err(t_args("lang.not_found", &[("lang", sigla)]));
            }
            config::set_language(sigla)?;
            Ok(t_args("lang.changed", &[("lang", sigla)]))
        }
    }
}

fn exe_dir() -> PathBuf {
    env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
}

fn help_text() -> String {
    t("help.text")
}

fn timeout_arg(args: &[String]) -> Result<Option<std::time::Duration>, String> {
    match args.iter().position(|a| a == "--timeout" || a == "-t") {
        Some(pos) => {
            let segundos: u64 = args
                .get(pos + 1)
                .and_then(|v| v.parse().ok())
                .ok_or_else(|| t("errors.invalid_timeout"))?;
            Ok(Some(std::time::Duration::from_secs(segundos)))
        }
        None => Ok(None),
    }
}

fn validate_profile_name(name: &str) -> Result<(), String> {
    if name.chars().any(|c| "\\/:*?\"<>|".contains(c)) {
        return Err(t("errors.profile_name_invalid"));
    }
    Ok(())
}

fn profile_name_arg(args: &[String]) -> Result<String, String> {
    let name = args
        .get(1)
        .filter(|n| !n.trim().is_empty())
        .ok_or_else(|| t("errors.profile_name_missing"))?;
    validate_profile_name(name)?;
    Ok(name.clone())
}

/// Execução interna acionada no login do Windows (chave Run), quando o
/// aguardar-drive está ativado: espera o Google Drive e restaura o perfil.
fn wait_drive_run() -> Result<String, String> {
    let config = wait_drive::load_config();
    if !config.enabled {
        return Err(t("wait_drive.disabled_error"));
    }
    let perfil = config.profile.unwrap();
    let timeout = config.timeout_secs.map(std::time::Duration::from_secs);
    println!(
        "{}",
        t_args(
            "wait_drive.run_waiting",
            &[
                ("process", process_watcher::GOOGLE_DRIVE_DESCRIPTION),
                (
                    "timeout",
                    &timeout
                        .map(|t| t_args(
                            "wait_drive.run_timeout_suffix",
                            &[("seconds", &t.as_secs().to_string())]
                        ))
                        .unwrap_or_else(|| t("wait_drive.run_no_timeout_suffix")),
                ),
            ],
        )
    );
    let esperou = process_watcher::wait_for_process(
        process_watcher::GOOGLE_DRIVE_DESCRIPTION,
        timeout,
        std::time::Duration::from_secs(5),
    )?;
    println!(
        "{}",
        t_args(
            "wait_drive.run_detected",
            &[
                ("seconds", &esperou.as_secs().to_string()),
                ("name", &perfil)
            ],
        )
    );
    let perfil_dados = load_profile(&perfil)?;
    let _ = shell_locator::refresh_desktop();
    let movidos = layout::restore_layout(&perfil_dados).map_err(|e| format!("{e}"))?;
    Ok(t_args(
        "wait_drive.run_restored",
        &[
            ("name", &perfil),
            ("moved", &movidos.to_string()),
            ("count", &perfil_dados.icons.len().to_string()),
        ],
    ))
}

fn profiles_base_dir() -> PathBuf {
    exe_dir().join(PROFILES_DIR)
}

fn profile_path(name: &str) -> PathBuf {
    profiles_base_dir().join(format!("{name}.json"))
}

fn parse_resolution(raw: &str) -> Result<Resolution, String> {
    let (w, h) = raw
        .split_once(['x', 'X'])
        .ok_or_else(|| t_args("errors.resolution_invalid", &[("raw", raw)]))?;
    let width: u32 = w
        .parse()
        .map_err(|_| t_args("errors.resolution_width_invalid", &[("raw", raw)]))?;
    let height: u32 = h
        .parse()
        .map_err(|_| t_args("errors.resolution_height_invalid", &[("raw", raw)]))?;
    if width == 0 || height == 0 {
        return Err(t("errors.resolution_zero"));
    }
    Ok(Resolution { width, height })
}

fn save_profile(profile: &DesktopProfile) -> Result<(), String> {
    fs::create_dir_all(profiles_base_dir()).map_err(|e| format!("{e}"))?;
    let json = serde_json::to_string_pretty(profile).map_err(|e| format!("{e}"))?;
    fs::write(profile_path(&profile.profile_name), json).map_err(|e| format!("{e}"))
}

fn load_profile(name: &str) -> Result<DesktopProfile, String> {
    let path = profile_path(name);
    let data = fs::read_to_string(&path).map_err(|_| {
        t_args(
            "errors.profile_not_found",
            &[("name", name), ("path", &path.display().to_string())],
        )
    })?;
    serde_json::from_str(&data)
        .map_err(|e| t_args("errors.profile_invalid", &[("error", &e.to_string())]))
}

fn list_profiles() -> Result<Vec<String>, String> {
    let mut names = Vec::new();
    let dir = match fs::read_dir(profiles_base_dir()) {
        Ok(dir) => dir,
        Err(_) => return Ok(names),
    };
    for entry in dir.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("json") {
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                names.push(stem.to_string());
            }
        }
    }
    names.sort();
    Ok(names)
}
