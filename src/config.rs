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
    #[serde(default)]
    pub startup: Option<StartupMirror>,
    #[serde(default)]
    pub wait_drive: Option<WaitDriveMirror>,
    #[serde(default)]
    pub hide_icons: Option<HideIconsMirror>,
}

impl Default for CliConfig {
    fn default() -> Self {
        Self {
            language: default_language(),
            startup: None,
            wait_drive: None,
            hide_icons: None,
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
    match fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
        Err(_) => {
            let config = CliConfig::default();
            let _ = save_config(&config);
            config
        }
    }
}

pub fn save_config(config: &CliConfig) -> Result<(), String> {
    let path = config_path()?;
    let json = serde_json::to_string_pretty(config).map_err(|e| format!("{e}"))?;
    fs::write(&path, json).map_err(|e| format!("{e}"))
}

pub fn get_language() -> String {
    load_or_create_config().language
}

pub fn set_language(language: &str) -> Result<(), String> {
    let mut config = load_or_create_config();
    config.language = language.to_string();
    save_config(&config)
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

pub fn get_hide_icons() -> Option<HideIconsMirror> {
    load_or_create_config().hide_icons
}

pub fn set_hide_icons(mirror: Option<HideIconsMirror>) {
    let mut config = load_or_create_config();
    config.hide_icons = mirror;
    let _ = save_config(&config);
}
