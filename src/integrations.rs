//! External actions. Validation never launches a process or contacts D-Bus.
use std::{path::Path, process::Command};

use anyhow::{Context, Result, bail, ensure};
use gtk::{gio, glib, prelude::*};

use crate::core::config::Action;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capabilities {
    pub uri: bool,
    pub application: bool,
    pub command: bool,
}

pub trait ExternalLauncher {
    fn capabilities(&self) -> Capabilities;
    fn validate(&self, action: &Action) -> Result<()>;
    fn launch(&self, action: &Action, language: &str) -> Result<()>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct NativeLauncher;

#[derive(Debug, Default, Clone, Copy)]
pub struct PortalLauncher;

/// Flatpak identity is detected even when FLATPAK_ID is set to an empty value.
pub fn is_flatpak() -> bool {
    flatpak_detected(
        std::env::var_os("FLATPAK_ID").is_some(),
        Path::new("/.flatpak-info").exists(),
    )
}

fn flatpak_detected(has_id: bool, has_info: bool) -> bool {
    has_id || has_info
}

pub fn launcher() -> Box<dyn ExternalLauncher> {
    if is_flatpak() {
        Box::new(PortalLauncher)
    } else {
        Box::new(NativeLauncher)
    }
}

pub fn capabilities() -> Capabilities {
    launcher().capabilities()
}

pub fn validate(action: &Action) -> Result<()> {
    launcher().validate(action)
}

pub fn launch(action: &Action, language: &str) -> Result<()> {
    launcher().launch(action, language)
}

impl NativeLauncher {
    /// Discover installed desktop entries through GIO, not by interpreting Exec lines.
    pub fn applications(&self) -> Result<Vec<gio::AppInfo>> {
        ensure!(
            !is_flatpak(),
            "Host application discovery is unavailable in Flatpak"
        );
        let mut apps: Vec<_> = gio::AppInfo::all()
            .into_iter()
            .filter(|app| app.should_show() && app.id().is_some())
            .collect();
        apps.sort_by_key(|app| {
            (
                app.display_name().to_string(),
                app.id().map(|id| id.to_string()),
            )
        });
        apps.dedup_by(|a, b| a.id() == b.id());
        Ok(apps)
    }
}

impl ExternalLauncher for NativeLauncher {
    fn capabilities(&self) -> Capabilities {
        policy_capabilities(is_flatpak())
    }

    fn validate(&self, action: &Action) -> Result<()> {
        validate_policy(action, is_flatpak())
    }

    fn launch(&self, action: &Action, language: &str) -> Result<()> {
        self.validate(action)?;
        match action {
            Action::Uri { uri } => launch_uri(uri, language),
            Action::Application { desktop_id } => {
                let app = gio::AppInfo::all()
                    .into_iter()
                    .find(|app| app.id().as_deref() == Some(desktop_id.as_str()))
                    .with_context(|| format!("Desktop application not found: {desktop_id}"))?;
                app.launch(&[], None::<&gio::AppLaunchContext>)
                    .context("Could not launch desktop application")
            }
            Action::Command { program, args } => {
                // Arguments remain separate, literal argv entries; no shell is inserted.
                let mut child = Command::new(program)
                    .args(args)
                    .spawn()
                    .with_context(|| format!("Could not launch {program}"))?;
                std::thread::spawn(move || {
                    let _ = child.wait();
                });
                Ok(())
            }
        }
    }
}

impl ExternalLauncher for PortalLauncher {
    fn capabilities(&self) -> Capabilities {
        policy_capabilities(true)
    }

    fn validate(&self, action: &Action) -> Result<()> {
        validate_policy(action, true)
    }

    fn launch(&self, action: &Action, language: &str) -> Result<()> {
        self.validate(action)?;
        match action {
            // GIO routes URI opening through the desktop portal inside Flatpak.
            Action::Uri { uri } => launch_uri(uri, language),
            _ => bail!("Host actions are unavailable through the portal"),
        }
    }
}

fn policy_capabilities(sandboxed: bool) -> Capabilities {
    Capabilities {
        uri: true,
        application: !sandboxed,
        command: !sandboxed,
    }
}

fn validate_policy(action: &Action, sandboxed: bool) -> Result<()> {
    match action {
        Action::Uri { uri } => {
            let sample = expand_template(uri, "0", "2000-01-01", "00:00:00", "en")?;
            validate_uri(&sample)
        }
        Action::Application { desktop_id } => {
            ensure!(
                !sandboxed,
                "Host application actions are unavailable in Flatpak"
            );
            ensure!(
                desktop_id.ends_with(".desktop") && desktop_id.len() > 8,
                "Expected a desktop application ID ending in .desktop"
            );
            ensure!(
                !desktop_id.chars().any(|c| c.is_control()
                    || c.is_whitespace()
                    || matches!(c, '/' | '\\' | '{' | '}')),
                "Invalid desktop application ID"
            );
            Ok(())
        }
        Action::Command { program, args } => {
            ensure!(
                !sandboxed,
                "Custom command actions are unavailable in Flatpak"
            );
            ensure!(
                !program.trim().is_empty(),
                "Command program must not be empty"
            );
            for value in std::iter::once(program).chain(args.iter()) {
                ensure!(!value.contains('\0'), "Command contains a NUL byte");
                ensure!(
                    !value.contains(['{', '}']),
                    "Placeholders are only supported in URI actions"
                );
            }
            Ok(())
        }
    }
}

fn validate_uri(uri: &str) -> Result<()> {
    ensure!(
        !uri.chars().any(|c| c.is_control() || c.is_whitespace()),
        "URI contains whitespace or control characters"
    );
    let (scheme, _) = uri
        .split_once(':')
        .context("URI must have an absolute scheme")?;
    ensure!(
        scheme
            .bytes()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic())
            && scheme
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'+' | b'-' | b'.')),
        "Invalid URI scheme"
    );
    ensure!(
        !matches!(
            scheme.to_ascii_lowercase().as_str(),
            "javascript" | "vbscript" | "data" | "shell" | "exec"
        ),
        "Unsafe URI scheme"
    );
    // Reject malformed escapes explicitly; GLib may otherwise tolerate them.
    let bytes = uri.as_bytes();
    for (i, byte) in bytes.iter().enumerate() {
        if *byte == b'%' {
            ensure!(
                bytes.get(i + 1).is_some_and(u8::is_ascii_hexdigit)
                    && bytes.get(i + 2).is_some_and(u8::is_ascii_hexdigit),
                "Invalid URI percent escape"
            );
        }
    }
    glib::Uri::parse(uri, glib::UriFlags::NONE).context("Invalid URI")?;
    Ok(())
}

fn encode(value: &str) -> String {
    let mut output = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            output.push(char::from(byte));
        } else {
            use std::fmt::Write;
            write!(output, "%{byte:02X}").expect("writing to String cannot fail");
        }
    }
    output
}

fn expand_template(
    template: &str,
    timestamp: &str,
    date: &str,
    time: &str,
    language: &str,
) -> Result<String> {
    // Substitution is only permitted after the scheme delimiter, never in the scheme.
    let (scheme, body) = template
        .split_once(':')
        .context("URI must have an absolute scheme")?;
    ensure!(
        !scheme.contains(['{', '}']),
        "URI scheme cannot contain placeholders"
    );
    let mut output = format!("{scheme}:");
    let mut rest = body;
    while let Some(start) = rest.find(['{', '}']) {
        output.push_str(&rest[..start]);
        ensure!(
            rest.as_bytes()[start] == b'{',
            "Unmatched closing placeholder brace"
        );
        let end = rest[start + 1..]
            .find('}')
            .context("Unclosed URI placeholder")?
            + start
            + 1;
        let name = &rest[start + 1..end];
        let value = match name {
            "timestamp" => timestamp,
            "date" => date,
            "time" => time,
            "language" => language,
            _ => bail!("Unknown URI placeholder: {{{name}}}"),
        };
        output.push_str(&encode(value));
        rest = &rest[end + 1..];
    }
    output.push_str(rest);
    Ok(output)
}

/// Render local date/time, Unix timestamp (seconds), and the supplied UI language.
/// Values are percent-encoded so they cannot inject URI delimiters or new fields.
pub fn expand_uri(template: &str, language: &str) -> Result<String> {
    validate_policy(
        &Action::Uri {
            uri: template.into(),
        },
        false,
    )?;
    let now = glib::DateTime::now_local().context("Could not read local date/time")?;
    let uri = expand_template(
        template,
        &now.to_unix().to_string(),
        now.format("%Y-%m-%d")?.as_str(),
        now.format("%H:%M:%S")?.as_str(),
        language,
    )?;
    validate_uri(&uri)?;
    Ok(uri)
}

fn launch_uri(template: &str, language: &str) -> Result<()> {
    let uri = expand_uri(template, language)?;
    gio::AppInfo::launch_default_for_uri(&uri, None::<&gio::AppLaunchContext>)
        .context("Could not open URI through GIO")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn uri(value: &str) -> Action {
        Action::Uri { uri: value.into() }
    }

    #[test]
    fn templates_encode_values_without_injection() {
        let rendered = expand_template(
            "obsidian://new?stamp={timestamp}&date={date}&time={time}&lang={language}",
            "123",
            "2026-10-01",
            "12:34:56",
            "ru&x=# /{}",
        )
        .unwrap();
        assert_eq!(
            rendered,
            "obsidian://new?stamp=123&date=2026-10-01&time=12%3A34%3A56&lang=ru%26x%3D%23%20%2F%7B%7D"
        );
        validate_uri(&rendered).unwrap();
    }

    #[test]
    fn invalid_templates_and_schemes_are_rejected() {
        for value in [
            "https://example.org/{unknown}",
            "https://x/{date",
            "https://x/date}",
            "https://x/{{date}}",
            "{language}://x",
            "relative/path",
            "1bad://x",
            "https ://x",
            "javascript:alert(1)",
            "DATA:text/plain,x",
            "exec:thing",
            "https://x/%zz",
            "https://x/\n",
        ] {
            assert!(
                validate_policy(&uri(value), false).is_err(),
                "accepted {value:?}"
            );
        }
        for value in [
            "https://example.org/?date={date}",
            "obsidian://new",
            "mailto:a@example.org",
        ] {
            validate_policy(&uri(value), false).unwrap();
        }
    }

    #[test]
    fn flatpak_policy_never_permits_host_actions() {
        for action in [
            Action::Application {
                desktop_id: "org.example.App.desktop".into(),
            },
            Action::Command {
                program: "echo".into(),
                args: vec!["hello".into()],
            },
        ] {
            assert!(validate_policy(&action, true).is_err());
            validate_policy(&action, false).unwrap();
            assert!(PortalLauncher.validate(&action).is_err());
        }
        validate_policy(&uri("https://example.org/{language}"), true).unwrap();
        assert_eq!(
            PortalLauncher.capabilities(),
            Capabilities {
                uri: true,
                application: false,
                command: false
            }
        );
        assert!(!flatpak_detected(false, false));
        assert!(flatpak_detected(true, false));
        assert!(flatpak_detected(false, true));
    }

    #[test]
    fn command_placeholders_and_invalid_desktop_ids_are_rejected() {
        for action in [
            Action::Command {
                program: "{language}".into(),
                args: vec![],
            },
            Action::Command {
                program: "echo".into(),
                args: vec!["{date}".into()],
            },
            Action::Command {
                program: "".into(),
                args: vec![],
            },
            Action::Command {
                program: "echo".into(),
                args: vec!["\0".into()],
            },
            Action::Application {
                desktop_id: "../app.desktop".into(),
            },
        ] {
            assert!(validate_policy(&action, false).is_err());
        }
        validate_policy(
            &Action::Command {
                program: "echo".into(),
                args: vec!["literal; $(not-run)".into()],
            },
            false,
        )
        .unwrap();
    }
}
