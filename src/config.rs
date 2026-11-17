use std::env;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

fn default_language() -> String {
    "en-us".to_string()
}

/// Configuração persistente do CLI, salva em `config.json` ao lado do exe.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliConfig {
    #[serde(default = "default_language")]
    pub language: String,
    /// Idioma escolhido para os textos da interface gráfica (`langs/ui_langs`).
    /// Quando ausente, a interface segue o `language` do CLI.
    #[serde(default)]
    pub ui_language: Option<String>,
    #[serde(default)]
    pub startup: Option<StartupMirror>,
    #[serde(default)]
    pub wait_drive: Option<WaitDriveMirror>,
    #[serde(default)]
    pub hide_icons: Option<HideIconsMirror>,
    /// Tema escolhido na interface gráfica (`"dark"` ou `"light"`). O CLI
    /// ignora o campo, mas o preserva ao regravar o arquivo.
    #[serde(default)]
    pub ui_theme: Option<String>,
    /// Quando true, fechar a janela apenas esconde a interface na bandeja do
    /// sistema em vez de encerrar o programa.
    #[serde(default)]
    pub close_to_tray: Option<bool>,
    /// Espelho da entrada que abre a interface junto com o Windows
    /// (fonte da verdade: `HKCU\...\Run`, ver `ui_startup`).
    #[serde(default)]
    pub ui_startup: Option<bool>,
    /// Controla se já existe a tarefa agendada para iniciar como administrador.
    #[serde(default)]
    pub scheduled_task: bool,
}

impl Default for CliConfig {
    fn default() -> Self {
        Self {
            language: default_language(),
            ui_language: None,
            startup: None,
            wait_drive: None,
            hide_icons: None,
            ui_theme: None,
            close_to_tray: None,
            ui_startup: None,
            scheduled_task: false,
        }
    }
}

/// Espelho da configuração de inicialização com o sistema (source of truth: registro).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartupMirror {
    pub enabled: bool,
    #[serde(default)]
    pub profile: Option<String>,
    #[serde(default)]
    pub command: Option<String>,
}

/// Espelho da configuração do hide-icons (source of truth: registro).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HideIconsMirror {
    pub enabled: bool,
    #[serde(default)]
    pub timeout_secs: Option<u64>,
    /// Bitmask da fonte de atividade (bit0 = mouse, bit1 = teclado; 3 = ambos).
    #[serde(default)]
    pub inputs: Option<u32>,
    /// Quando true, o cursor do mouse também é ocultado junto com os ícones.
    #[serde(default)]
    pub include_cursor: Option<bool>,
    /// Quando true, as janelas abertas são expandidas para a resolução
    /// física total do monitor junto com a barra de tarefas oculta.
    #[serde(default)]
    pub include_taskbar: Option<bool>,
}

/// Espelho da configuração do wait-drive (source of truth: registro).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaitDriveMirror {
    pub enabled: bool,
    #[serde(default)]
    pub profile: Option<String>,
    #[serde(default)]
    pub timeout_secs: Option<u64>,
}

/// Caminho do `config.json`: mesma pasta do executável.
pub fn config_path() -> Result<PathBuf, String> {
    let exe = env::current_exe().map_err(|e| format!("{e}"))?;
    let dir = exe
        .parent()
        .ok_or_else(|| "Could not determine the executable's folder.".to_string())?;
    Ok(dir.join("config.json"))
}

/// Carrega a configuração; cria o arquivo com os padrões se não existir.
/// Se o arquivo existir mas estiver inválido, retorna os padrões.
pub fn load_or_create_config() -> CliConfig {
    let path = match config_path() {
        Ok(p) => p,
        Err(_) => return CliConfig::default(),
    };
    if let Ok(text) = fs::read_to_string(&path) {
        return serde_json::from_str(&text).unwrap_or_default();
    }
    if let Ok(cwd) = env::current_dir() {
        let cwd_path = cwd.join("config.json");
        if cwd_path != path {
            if let Ok(text) = fs::read_to_string(&cwd_path) {
                if let Ok(cfg) = serde_json::from_str::<CliConfig>(&text) {
                    return cfg;
                }
            }
        }
    }
    let config = CliConfig::default();
    let _ = save_config(&config);
    config
}

pub fn save_config(config: &CliConfig) -> Result<(), String> {
    let path = config_path()?;
    let json = serde_json::to_string_pretty(config).map_err(|e| format!("{e}"))?;
    fs::write(&path, &json).map_err(|e| format!("{e}"))?;
    if let Ok(cwd) = env::current_dir() {
        let cwd_path = cwd.join("config.json");
        if cwd_path != path && cwd_path.is_file() {
            let _ = fs::write(&cwd_path, &json);
        }
    }
    Ok(())
}

pub fn get_language() -> String {
    load_or_create_config().language
}

pub fn set_language(language: &str) -> Result<(), String> {
    let mut config = load_or_create_config();
    config.language = language.to_string();
    save_config(&config)
}

/// Idioma dos textos da interface: `ui_language` quando definido; caso
/// contrário, o idioma do CLI.
pub fn get_ui_language() -> String {
    let config = load_or_create_config();
    match config.ui_language {
        Some(language) if !language.trim().is_empty() => language,
        _ => config.language,
    }
}

pub fn get_startup() -> Option<StartupMirror> {
    load_or_create_config().startup
}

pub fn set_startup(mirror: Option<StartupMirror>) {
    let mut config = load_or_create_config();
    config.startup = mirror;
    let _ = save_config(&config);
}

pub fn get_wait_drive() -> Option<WaitDriveMirror> {
    load_or_create_config().wait_drive
}

pub fn set_wait_drive(mirror: Option<WaitDriveMirror>) {
    let mut config = load_or_create_config();
    config.wait_drive = mirror;
    let _ = save_config(&config);
}

/// Lê o espelho da inicialização da interface no `config.json`.
pub fn get_ui_startup() -> Option<bool> {
    load_or_create_config().ui_startup
}

/// Grava o espelho da inicialização da interface no `config.json`.
pub fn set_ui_startup(enabled: bool) {
    let mut config = load_or_create_config();
    config.ui_startup = Some(enabled);
    let _ = save_config(&config);
}

/// Retorna se já existe tarefa agendada para iniciar como administrador.
pub fn get_scheduled_task() -> bool {
    load_or_create_config().scheduled_task
}

/// Grava no `config.json` se a tarefa agendada existe ou não.
pub fn set_scheduled_task(exists: bool) {
    let mut config = load_or_create_config();
    config.scheduled_task = exists;
    let _ = save_config(&config);
}

pub fn get_hide_icons() -> Option<HideIconsMirror> {
    load_or_create_config().hide_icons
}

pub fn set_hide_icons(mirror: Option<HideIconsMirror>) {
    let mut config = load_or_create_config();
    config.hide_icons = mirror;
    let _ = save_config(&config);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scheduled_task_default_false() {
        let cfg = CliConfig::default();
        assert!(!cfg.scheduled_task);
    }

    #[test]
    fn test_scheduled_task_missing_in_json_defaults_to_false() {
        let json = r#"{ "language": "pt-br" }"#;
        let cfg: CliConfig = serde_json::from_str(json).expect("JSON válido");
        assert!(!cfg.scheduled_task);
    }

    #[test]
    fn test_scheduled_task_roundtrip_json() {
        let json = r#"{
            "language": "pt-br",
            "ui_startup": true,
            "scheduled_task": true
        }"#;
        let cfg: CliConfig = serde_json::from_str(json).expect("JSON válido");
        assert!(cfg.scheduled_task);
        assert_eq!(cfg.ui_startup, Some(true));

        let serialized = serde_json::to_string_pretty(&cfg).expect("serialização válida");
        assert!(serialized.contains("\"scheduled_task\": true"));

        let reparsed: CliConfig = serde_json::from_str(&serialized).expect("reparse válido");
        assert!(reparsed.scheduled_task);
    }
}
