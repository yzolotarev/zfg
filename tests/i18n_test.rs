use zfg::core::i18n::text;

#[test]
fn translations_cover_the_same_initial_keys_and_fallback() {
    let en: serde_json::Value = serde_json::from_str(include_str!("../locales/en.json")).unwrap();
    let ru: serde_json::Value = serde_json::from_str(include_str!("../locales/ru.json")).unwrap();
    let mut en_keys: Vec<_> = en.as_object().unwrap().keys().collect();
    let mut ru_keys: Vec<_> = ru.as_object().unwrap().keys().collect();
    en_keys.sort();
    ru_keys.sort();
    assert_eq!(en_keys, ru_keys);
    assert_eq!(text("ru", "pie.note"), "Заметка");
    assert_eq!(text("en", "pie.note"), "Note");
    assert_eq!(text("ru", "missing.key"), "missing.key");
}
