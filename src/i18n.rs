use std::collections::HashMap;
use std::env;
use std::fs;
use std::sync::{Mutex, MutexGuard, OnceLock};

use crate::config;

/// Dicionário achatado (chaves em notação de ponto) carregado uma única vez.
static TRANSLATIONS: OnceLock<HashMap<String, String>> = OnceLock::new();

/// Fallback embutido mínimo, usado apenas se o arquivo de idioma não existir.
fn builtin_fallback() -> HashMap<String, String> {
    HashMap::from([
        ("errors.generic".to_string(), "Error: {error}".to_string()),
        (
            "lang.current".to_string(),
            "Current language: {lang}".to_string(),
        ),
        (
            "lang.changed".to_string(),
            "Language changed to '{lang}'.".to_string(),
        ),
        (
            "lang.not_found".to_string(),
            "Language file '{lang}.json' not found in the 'langs' folder next to the executable."
                .to_string(),
        ),
    ])
}

/// Achata recursivamente um JSON aninhado em chaves "modulo.chave".
fn flatten(value: &serde_json::Value, prefix: &str, out: &mut HashMap<String, String>) {
    match value {
        serde_json::Value::Object(map) => {
            for (k, v) in map {
                let key = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };
                flatten(v, &key, out);
            }
        }
        serde_json::Value::String(s) => {
            out.insert(prefix.to_string(), s.clone());
        }
        _ => {}
    }
}

fn exe_dir() -> Option<std::path::PathBuf> {
    env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
}

/// Idiomas embutidos no binário (funcionam mesmo sem a pasta `langs/`).
const EMBEDDED: &[(&str, &str)] = &[
    (
        "pt-br",
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/langs/pt-br.json")),
    ),
    (
        "en-us",
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/langs/en-us.json")),
    ),
    (
        "ru",
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/langs/ru.json")),
    ),
    (
        "zh",
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/langs/zh.json")),
    ),
    (
        "es",
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/langs/es.json")),
    ),
];

/// Retorna o JSON embutido da sigla, se existir.
pub fn embedded_language(sigla: &str) -> Option<&'static str> {
    EMBEDDED
        .iter()
        .find(|(code, _)| code.eq_ignore_ascii_case(sigla))
        .map(|(_, json)| *json)
}

/// Textos da interface embutidos no binário (pasta `langs/ui_langs/`).
const EMBEDDED_UI: &[(&str, &str)] = &[
    (
        "pt-br",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/langs/ui_langs/pt-br.json"
        )),
    ),
    (
        "en-us",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/langs/ui_langs/en-us.json"
        )),
    ),
    (
        "ru",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/langs/ui_langs/ru.json"
        )),
    ),
    (
        "zh",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/langs/ui_langs/zh.json"
        )),
    ),
    (
        "es",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/langs/ui_langs/es.json"
        )),
    ),
];

/// Idioma usado quando uma chave não existe no idioma escolhido.
const UI_FALLBACK_LANGUAGE: &str = "en-us";

/// Retorna o JSON da interface embutido da sigla, se existir.
pub fn embedded_ui_language(sigla: &str) -> Option<&'static str> {
    EMBEDDED_UI
        .iter()
        .find(|(code, _)| code.eq_ignore_ascii_case(sigla))
        .map(|(_, json)| *json)
}

fn load_translations() -> HashMap<String, String> {
    let language = config::get_language();
    // 1) Arquivo ao lado do exe (idiomas extras ou traduções editadas).
    if let Some(dir) = exe_dir() {
        let path = dir.join("langs").join(format!("{language}.json"));
        if let Ok(text) = fs::read_to_string(&path) {
            if let Some(map) = parse_json(&text) {
                return map;
            }
        }
    }
    // 2) Idioma embutido no binário (cargo install sem pasta langs/).
    if let Some(json) = embedded_language(&language) {
        if let Some(map) = parse_json(json) {
            return map;
        }
    }
    // 3) Fallback mínimo hardcoded.
    builtin_fallback()
}

fn parse_json(text: &str) -> Option<HashMap<String, String>> {
    let json = serde_json::from_str::<serde_json::Value>(text).ok()?;
    let mut map = HashMap::new();
    flatten(&json, "", &mut map);
    Some(map)
}

fn translations() -> &'static HashMap<String, String> {
    TRANSLATIONS.get_or_init(load_translations)
}

/// Retorna o texto da chave; se não existir, retorna \"[{key}]\" (sem panic).
pub fn t(key: &str) -> String {
    translations()
        .get(key)
        .cloned()
        .unwrap_or_else(|| format!("[{key}]"))
}

/// Como `t`, mas substitui placeholders `{nome}` pelos valores fornecidos.
pub fn t_args(key: &str, args: &[(&str, &str)]) -> String {
    let mut text = t(key);
    for (name, value) in args {
        text = text.replace(&format!("{{{name}}}"), value);
    }
    text
}

// ============================================================================
// TRADUÇÕES DA INTERFACE GRÁFICA (`langs/ui_langs/`)
// ============================================================================
//
// A interface tem textos próprios, separados dos textos do CLI, e pode trocar
// de idioma em tempo de execução (aba Idioma & Sistema). Por isso o dicionário
// fica atrás de um `Mutex` recarregável em vez do `OnceLock` do CLI.

struct UiDictionary {
    language: String,
    entries: HashMap<String, String>,
    fallback: Option<HashMap<String, String>>,
}

static UI_DICTIONARY: OnceLock<Mutex<UiDictionary>> = OnceLock::new();

/// Carrega o dicionário da interface: arquivo ao lado do exe e depois o
/// JSON embutido no binário.
fn load_ui_language(language: &str) -> HashMap<String, String> {
    if let Some(dir) = exe_dir() {
        let path = dir
            .join("langs")
            .join("ui_langs")
            .join(format!("{language}.json"));
        if let Ok(text) = fs::read_to_string(&path) {
            if let Some(map) = parse_json(&text) {
                return map;
            }
        }
    }
    embedded_ui_language(language)
        .and_then(parse_json)
        .unwrap_or_default()
}

fn ui_dictionary() -> &'static Mutex<UiDictionary> {
    UI_DICTIONARY.get_or_init(|| {
        let language = config::get_ui_language();
        let entries = load_ui_language(&language);
        Mutex::new(UiDictionary {
            language,
            entries,
            fallback: None,
        })
    })
}

fn ui_entries() -> MutexGuard<'static, UiDictionary> {
    ui_dictionary()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Recarrega os textos da interface para o idioma pedido (no-op se igual).
pub fn set_ui_language(language: &str) {
    let mut dict = ui_entries();
    if dict.language.eq_ignore_ascii_case(language) {
        return;
    }
    dict.language = language.to_string();
    dict.entries = load_ui_language(language);
    dict.fallback = None;
}

/// Texto da interface; se a chave faltar, usa o inglês embutido e, por fim,
/// devolve \"[{key}]\" (sem panic).
pub fn ui_t(key: &str) -> String {
    let mut dict = ui_entries();
    if let Some(text) = dict.entries.get(key) {
        return text.clone();
    }
    if !dict.language.eq_ignore_ascii_case(UI_FALLBACK_LANGUAGE) {
        if dict.fallback.is_none() {
            dict.fallback = Some(load_ui_language(UI_FALLBACK_LANGUAGE));
        }
        if let Some(text) = dict.fallback.as_ref().and_then(|map| map.get(key)) {
            return text.clone();
        }
    }
    format!("[{key}]")
}

/// Como `ui_t`, mas substitui placeholders `{nome}` pelos valores fornecidos.
pub fn ui_t_args(key: &str, args: &[(&str, &str)]) -> String {
    let mut text = ui_t(key);
    for (name, value) in args {
        text = text.replace(&format!("{{{name}}}"), value);
    }
    text
}
