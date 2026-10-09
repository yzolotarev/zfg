use std::{fs, path::PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub schema_version: u32,
    pub language: String,
    pub focus_limit: usize,
    pub close_after_action: bool,
    pub appearance: Appearance,
    pub note_action: Action,
    pub calendar_action: Action,
    pub panel: PanelSettings,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Appearance {
    pub theme: String,
    pub animations: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    Uri { uri: String },
    Application { desktop_id: String },
    Command { program: String, args: Vec<String> },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PanelSettings {
    pub truncate_at: usize,
    pub show_focus_counter: bool,
    pub show_later_counter: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: 1,
            language: "system".into(),
            focus_limit: 5,
            close_after_action: true,
            appearance: Appearance::default(),
            note_action: Action::Uri {
                uri: "obsidian://new".into(),
            },
            calendar_action: Action::Uri {
                uri: "https://calendar.google.com/calendar/u/0/r/eventedit".into(),
            },
            panel: PanelSettings::default(),
        }
    }
}
impl Default for Appearance {
    fn default() -> Self {
        Self {
            theme: "system".into(),
            animations: true,
        }
    }
}
impl Default for PanelSettings {
    fn default() -> Self {
        Self {
            truncate_at: 28,
            show_focus_counter: true,
            show_later_counter: true,
        }
    }
}
impl Default for Action {
    fn default() -> Self {
        Self::Uri { uri: String::new() }
    }
}

impl Settings {
    pub fn path() -> Result<PathBuf> {
        let base = dirs::config_dir().context("Could not determine XDG config directory")?;
        Ok(base.join("zfg/settings.json"))
    }
    pub fn load() -> Result<Self> {
        Self::load_from_path(&Self::path()?)
    }
    pub fn load_from_path(path: &std::path::Path) -> Result<Self> {
        let bytes = match fs::read(path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(e.into()),
        };
        match serde_json::from_slice(&bytes) {
            Ok(settings) => Ok(settings),
            Err(error) => {
                let stamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_nanos();
                let backup = path.with_file_name(format!("settings.json.corrupt-{stamp}"));
                fs::copy(path, &backup).with_context(|| {
                    format!(
                        "Could not back up corrupted settings to {}",
                        backup.display()
                    )
                })?;
                eprintln!(
                    "Warning: invalid settings were backed up to {}: {error}",
                    backup.display()
                );
                Ok(Self::default())
            }
        }
    }
    pub fn save(&self) -> Result<()> {
        let path = Self::path()?;
        let parent = path.parent().context("settings path has no parent")?;
        fs::create_dir_all(parent)?;
        let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
        serde_json::to_writer_pretty(&mut tmp, self)?;
        use std::io::Write;
        tmp.write_all(b"\n")?;
        tmp.as_file().sync_all()?;
        tmp.persist(&path).map_err(|e| e.error)?;
        fs::File::open(parent)?.sync_all()?;
        Ok(())
    }
}
