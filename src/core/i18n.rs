use std::sync::OnceLock;

use serde_json::Value;

static EN: OnceLock<Value> = OnceLock::new();
static RU: OnceLock<Value> = OnceLock::new();

pub fn text(language: &str, key: &str) -> String {
    let detected = if language == "system" {
        std::env::var("LANG")
            .unwrap_or_default()
            .to_ascii_lowercase()
    } else {
        language.to_ascii_lowercase()
    };
    let selected = if detected.starts_with("ru") {
        "ru"
    } else {
        "en"
    };
    let localized = if selected == "ru" {
        RU.get_or_init(|| {
            serde_json::from_str(include_str!("../../locales/ru.json"))
                .expect("embedded ru.json is valid")
        })
    } else {
        EN.get_or_init(|| {
            serde_json::from_str(include_str!("../../locales/en.json"))
                .expect("embedded en.json is valid")
        })
    };
    localized
        .get(key)
        .and_then(Value::as_str)
        .or_else(|| {
            EN.get_or_init(|| {
                serde_json::from_str(include_str!("../../locales/en.json"))
                    .expect("embedded en.json is valid")
            })
            .get(key)
            .and_then(Value::as_str)
        })
        .unwrap_or(key)
        .to_owned()
}
