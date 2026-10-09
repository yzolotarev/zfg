use std::process::{Command, Output};
fn cli(root: &std::path::Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_zfg"))
        .env("XDG_DATA_HOME", root)
        .env("XDG_CONFIG_HOME", root.join("config"))
        .args(args)
        .output()
        .unwrap()
}
#[test]
fn status_exposes_localized_panel_preferences() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config/zfg");
    std::fs::create_dir_all(&config).unwrap();
    std::fs::write(config.join("settings.json"), r#"{"language":"ru","panel":{"truncate_at":18,"show_focus_counter":false,"show_later_counter":false}}"#).unwrap();
    let output = cli(dir.path(), &["status", "--json"]);
    assert!(output.status.success());
    let status: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(status["focus_empty_label"], "В Фокусе нет задач");
    assert_eq!(status["panel"]["truncate_at"], 18);
    assert_eq!(status["panel"]["show_focus_counter"], false);
    assert_eq!(status["panel"]["show_later_counter"], false);
}

#[test]
fn isolated_cli_workflow() {
    let dir = tempfile::tempdir().unwrap();
    for task in ["One", "Two", "Three", "Four", "Five"] {
        assert!(
            cli(dir.path(), &["task", "add", "--focus", task])
                .status
                .success()
        );
    }
    let full = cli(dir.path(), &["task", "add", "--focus", "Six"]);
    assert!(!full.status.success());
    let error: serde_json::Value = serde_json::from_slice(&full.stderr).unwrap();
    assert_eq!(error["error"], "focus_full");
    assert!(
        cli(
            dir.path(),
            &["task", "add", "--focus", "New", "--replace", "1"]
        )
        .status
        .success()
    );
    let output = cli(dir.path(), &["status", "--json"]);
    assert!(output.status.success());
    let status: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(status["focus_first"], "New");
    assert_eq!(status["later_count"], 1);
    assert_eq!(status["focus_full"], true);
    std::fs::write(dir.path().join("zfg/focus.txt"), "External\n").unwrap();
    let status: serde_json::Value =
        serde_json::from_slice(&cli(dir.path(), &["status", "--json"]).stdout).unwrap();
    assert_eq!(status["focus_first"], "External");
    assert!(
        !cli(dir.path(), &["task", "complete", "--focus", "0"])
            .status
            .success()
    );
}
