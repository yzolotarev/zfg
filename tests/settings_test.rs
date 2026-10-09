use zfg::core::config::{Action, Settings};

#[test]
fn settings_defaults_and_partial_future_keys() {
    let defaults = Settings::default();
    assert_eq!(defaults.focus_limit, 5);
    assert_eq!(defaults.language, "system");
    assert!(defaults.close_after_action);
    assert!(matches!(defaults.note_action, Action::Uri { .. }));

    let partial: Settings =
        serde_json::from_str(r#"{"focus_limit":7,"future_option":{"enabled":true}}"#).unwrap();
    assert_eq!(partial.focus_limit, 7);
    assert_eq!(partial.panel.truncate_at, 28);
    assert_eq!(partial.schema_version, 1);
}

#[test]
fn corrupt_settings_are_backed_up_without_destroying_original() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    let original = b"{ broken";
    std::fs::write(&path, original).unwrap();
    let settings = Settings::load_from_path(&path).unwrap();
    assert_eq!(settings.focus_limit, 5);
    assert_eq!(std::fs::read(&path).unwrap(), original);
    let backups: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().contains(".corrupt-"))
        .collect();
    assert_eq!(backups.len(), 1);
    assert_eq!(std::fs::read(backups[0].path()).unwrap(), original);
}

#[test]
fn settings_actions_round_trip_as_argv_not_shell() {
    let value = serde_json::json!({"type":"command","program":"/usr/bin/example","args":["--new","{text}"]});
    let action: Action = serde_json::from_value(value.clone()).unwrap();
    assert!(
        matches!(action, Action::Command { ref program, ref args } if program == "/usr/bin/example" && args[0] == "--new")
    );
    assert_eq!(serde_json::to_value(action).unwrap(), value);
}
