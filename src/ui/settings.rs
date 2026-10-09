use gtk::{Box as GtkBox, Orientation, gio, prelude::*};
use zfg::core::{
    config::{Action, Settings},
    i18n::text,
};

const HYPRLAND_BINDING: &str = "bind = SUPER, Z, exec, zfg launcher";

fn row(root: &GtkBox, language: &str, key: &str, widget: &impl IsA<gtk::Widget>) {
    let line = GtkBox::new(Orientation::Horizontal, 12);
    let label = gtk::Label::new(Some(&text(language, key)));
    label.set_halign(gtk::Align::Start);
    label.set_hexpand(true);
    label.set_wrap(true);
    line.append(&label);
    line.append(widget);
    root.append(&line);
}

fn choices(items: &[(&str, String)], selected: &str) -> gtk::ComboBoxText {
    let combo = gtk::ComboBoxText::new();
    for (id, label) in items {
        combo.append(Some(id), label);
    }
    combo.set_active_id(Some(selected));
    if combo.active_id().is_none() {
        combo.set_active(Some(0));
    }
    combo
}

struct ActionEditor {
    kind: gtk::ComboBoxText,
    uri: gtk::Entry,
    application: gtk::ComboBoxText,
    program: gtk::Entry,
    args: gtk::Entry,
    original: Action,
    sandboxed: bool,
}

impl ActionEditor {
    fn new(root: &GtkBox, language: &str, key: &str, original: &Action, sandboxed: bool) -> Self {
        let title = gtk::Label::new(Some(&text(language, key)));
        title.set_halign(gtk::Align::Start);
        title.add_css_class("heading");
        root.append(&title);
        let kind = choices(
            &[
                ("uri", text(language, "settings.uri")),
                ("application", text(language, "settings.application")),
                ("command", text(language, "settings.command")),
            ],
            match original {
                Action::Uri { .. } => "uri",
                Action::Application { .. } => "application",
                Action::Command { .. } => "command",
            },
        );
        // In a sandbox, only URI actions can be edited. Existing host actions are
        // retained until the user explicitly replaces them with a URI.
        if sandboxed {
            kind.remove_all();
            kind.append(Some("uri"), &text(language, "settings.uri"));
            if !matches!(original, Action::Uri { .. }) {
                kind.append(
                    Some("preserve"),
                    &text(language, "settings.preserve_action"),
                );
                kind.set_active_id(Some("preserve"));
            } else {
                kind.set_active(Some(0));
            }
        }
        row(root, language, "settings.action_type", &kind);
        let stack = gtk::Stack::new();
        let uri = gtk::Entry::new();
        uri.set_hexpand(true);
        if let Action::Uri { uri: value } = original {
            uri.set_text(value);
        }
        stack.add_named(&uri, Some("uri"));
        let application = gtk::ComboBoxText::new();
        if !sandboxed {
            let mut apps: Vec<_> = gio::AppInfo::all()
                .into_iter()
                .filter_map(|app| {
                    app.id()
                        .map(|id| (id.to_string(), app.display_name().to_string()))
                })
                .collect();
            apps.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(&b.0)));
            apps.dedup_by(|a, b| a.0 == b.0);
            for (id, name) in apps {
                application.append(Some(&id), &format!("{name} ({id})"));
            }
            if let Action::Application { desktop_id } = original {
                if !application.set_active_id(Some(desktop_id)) {
                    application.append(Some(desktop_id), desktop_id);
                    application.set_active_id(Some(desktop_id));
                }
            } else {
                application.set_active(Some(0));
            }
        }
        stack.add_named(&application, Some("application"));
        let command = GtkBox::new(Orientation::Vertical, 8);
        let program = gtk::Entry::new();
        let args = gtk::Entry::new();
        args.set_placeholder_text(Some("[\"--flag\", \"argument\"]"));
        args.set_text("[]");
        if let Action::Command {
            program: value,
            args: values,
        } = original
        {
            program.set_text(value);
            args.set_text(&serde_json::to_string(values).expect("string vector serializes"));
        }
        row(&command, language, "settings.program", &program);
        row(&command, language, "settings.args", &args);
        stack.add_named(&command, Some("command"));
        let preserved = gtk::Label::new(Some(&text(language, "settings.flatpak")));
        preserved.set_wrap(true);
        stack.add_named(&preserved, Some("preserve"));
        stack.set_visible_child_name(kind.active_id().as_deref().unwrap_or("uri"));
        let weak_stack = stack.downgrade();
        kind.connect_changed(move |combo| {
            if let Some(stack) = weak_stack.upgrade() {
                stack.set_visible_child_name(combo.active_id().as_deref().unwrap_or("uri"));
            }
        });
        root.append(&stack);
        Self {
            kind,
            uri,
            application,
            program,
            args,
            original: original.clone(),
            sandboxed,
        }
    }

    fn read(&self, language: &str) -> Result<Action, String> {
        let action = match self.kind.active_id().as_deref() {
            Some("uri") if !self.uri.text().trim().is_empty() => Ok(Action::Uri {
                uri: self.uri.text().trim().into(),
            }),
            Some("application") if !self.sandboxed => self
                .application
                .active_id()
                .filter(|id| !id.is_empty())
                .map(|id| Action::Application {
                    desktop_id: id.to_string(),
                })
                .ok_or_else(|| text(language, "settings.invalid_application")),
            Some("command") if !self.sandboxed => {
                let program = self.program.text().trim().to_owned();
                if program.is_empty() {
                    return Err(text(language, "settings.invalid_program"));
                }
                let args = serde_json::from_str::<Vec<String>>(&self.args.text())
                    .map_err(|_| text(language, "settings.invalid_args"))?;
                Ok(Action::Command { program, args })
            }
            Some("preserve") if self.sandboxed => Ok(self.original.clone()),
            _ => Err(text(language, "settings.invalid_uri")),
        }?;
        if self.kind.active_id().as_deref() != Some("preserve") {
            zfg::integrations::validate(&action).map_err(|error| error.to_string())?;
        }
        Ok(action)
    }
}

pub(crate) fn show(app: &gtk::Application, settings: Settings) {
    let language = settings.language.clone();
    let window = gtk::ApplicationWindow::builder()
        .application(app)
        .title(text(&language, "launcher.settings"))
        .default_width(620)
        .default_height(720)
        .build();
    let root = GtkBox::new(Orientation::Vertical, 12);
    root.set_margin_top(18);
    root.set_margin_bottom(18);
    root.set_margin_start(18);
    root.set_margin_end(18);
    let scroll = gtk::ScrolledWindow::builder()
        .vexpand(true)
        .child(&root)
        .build();
    window.set_child(Some(&scroll));
    let language_choice = choices(
        &[
            ("system", text(&language, "settings.system")),
            ("en", "English".into()),
            ("ru", "Русский".into()),
        ],
        &settings.language,
    );
    row(&root, &language, "settings.language", &language_choice);
    let focus = gtk::Entry::new();
    focus.set_text(&settings.focus_limit.to_string());
    row(&root, &language, "settings.focus_limit", &focus);
    let close = gtk::CheckButton::new();
    close.set_active(settings.close_after_action);
    row(&root, &language, "settings.close_after_action", &close);
    let theme = choices(
        &[
            ("system", text(&language, "settings.system")),
            ("dark", text(&language, "settings.dark")),
            ("light", text(&language, "settings.light")),
        ],
        &settings.appearance.theme,
    );
    row(&root, &language, "settings.theme", &theme);
    let animations = gtk::CheckButton::new();
    animations.set_active(settings.appearance.animations);
    row(&root, &language, "settings.animations", &animations);
    let sandboxed =
        std::env::var_os("FLATPAK_ID").is_some() || std::path::Path::new("/.flatpak-info").exists();
    if sandboxed {
        let limitation = gtk::Label::new(Some(&text(&language, "settings.flatpak")));
        limitation.set_wrap(true);
        root.append(&limitation);
    }
    let note = ActionEditor::new(
        &root,
        &language,
        "settings.note_action",
        &settings.note_action,
        sandboxed,
    );
    let calendar = ActionEditor::new(
        &root,
        &language,
        "settings.calendar_action",
        &settings.calendar_action,
        sandboxed,
    );
    let truncate = gtk::Entry::new();
    truncate.set_text(&settings.panel.truncate_at.to_string());
    row(&root, &language, "settings.truncate_at", &truncate);
    let focus_counter = gtk::CheckButton::new();
    focus_counter.set_active(settings.panel.show_focus_counter);
    row(&root, &language, "settings.focus_counter", &focus_counter);
    let later_counter = gtk::CheckButton::new();
    later_counter.set_active(settings.panel.show_later_counter);
    row(&root, &language, "settings.later_counter", &later_counter);
    let binding = gtk::Label::new(Some(HYPRLAND_BINDING));
    binding.set_selectable(true);
    binding.set_wrap(true);
    root.append(&binding);
    let copy = gtk::Button::with_label(&text(&language, "settings.copy_binding"));
    copy.connect_clicked(|button| {
        button.display().clipboard().set_text(HYPRLAND_BINDING);
    });
    root.append(&copy);
    let hint = gtk::Label::new(Some(&text(&language, "settings.restart_hint")));
    hint.set_wrap(true);
    root.append(&hint);
    let status = gtk::Label::new(None);
    status.set_wrap(true);
    status.set_selectable(true);
    root.append(&status);
    let portal = gtk::Button::with_label(&text(&language, "shortcuts.enable"));
    portal.connect_clicked({
        let weak_app = app.downgrade();
        let status = status.clone();
        let language = language.clone();
        move |button| {
            let Some(app) = weak_app.upgrade() else {
                return;
            };
            button.set_sensitive(false);
            status.set_text(&text(&language, "shortcuts.waiting"));
            let status = status.clone();
            let language = language.clone();
            let button = button.downgrade();
            crate::ui::shortcuts::enable(&app, move |result| {
                status.set_text(&match result {
                    Ok(()) => text(&language, "shortcuts.ready"),
                    Err(error) => {
                        if let Some(button) = button.upgrade() {
                            button.set_sensitive(true);
                        }
                        error
                    }
                });
            });
        }
    });
    gtk::glib::spawn_future_local({
        let portal = portal.downgrade();
        async move {
            if let Err(error) = crate::ui::shortcuts::detect().await
                && let Some(portal) = portal.upgrade()
            {
                portal.set_sensitive(false);
                portal.set_tooltip_text(Some(&error));
            }
        }
    });
    root.append(&portal);
    let disable_shortcut = gtk::Button::with_label(&text(&language, "shortcuts.disable"));
    disable_shortcut.connect_clicked({
        let status = status.clone();
        let portal = portal.downgrade();
        let language = language.clone();
        move |_| {
            crate::ui::shortcuts::disable();
            if let Some(portal) = portal.upgrade() {
                portal.set_sensitive(true);
            }
            status.set_text(&text(&language, "shortcuts.disabled"));
        }
    });
    root.append(&disable_shortcut);
    let save = gtk::Button::with_label(&text(&language, "settings.save"));
    save.add_css_class("suggested-action");
    root.append(&save);
    let weak_app = app.downgrade();
    let weak_window = window.downgrade();
    save.connect_clicked(move |_| {
        let result = (|| -> Result<Settings, String> {
            let mut updated = settings.clone();
            updated.focus_limit = focus
                .text()
                .trim()
                .parse::<usize>()
                .ok()
                .filter(|n| *n > 0)
                .ok_or_else(|| text(&language, "settings.invalid_focus"))?;
            updated.panel.truncate_at = truncate
                .text()
                .trim()
                .parse::<usize>()
                .map_err(|_| text(&language, "settings.invalid_truncate"))?;
            updated.language = language_choice
                .active_id()
                .unwrap_or_else(|| "system".into())
                .to_string();
            updated.close_after_action = close.is_active();
            updated.appearance.theme = theme
                .active_id()
                .unwrap_or_else(|| "system".into())
                .to_string();
            updated.appearance.animations = animations.is_active();
            updated.note_action = note.read(&language)?;
            updated.calendar_action = calendar.read(&language)?;
            updated.panel.show_focus_counter = focus_counter.is_active();
            updated.panel.show_later_counter = later_counter.is_active();
            updated
                .save()
                .map_err(|error| format!("{}: {error}", text(&language, "settings.save_failed")))?;
            if let Some(gtk_settings) = gtk::Settings::default() {
                match updated.appearance.theme.as_str() {
                    "dark" => gtk_settings.set_gtk_application_prefer_dark_theme(true),
                    "light" => gtk_settings.set_gtk_application_prefer_dark_theme(false),
                    _ => gtk_settings.reset_property("gtk-application-prefer-dark-theme"),
                }
                if updated.appearance.animations {
                    gtk_settings.reset_property("gtk-enable-animations");
                } else {
                    gtk_settings.set_gtk_enable_animations(false);
                }
            }
            Ok(updated)
        })();
        status.set_text(&match result {
            Ok(updated) => {
                if updated.language != language
                    && let (Some(app), Some(window)) = (weak_app.upgrade(), weak_window.upgrade())
                {
                    window.close();
                    show(&app, updated);
                }
                text(&language, "settings.saved")
            }
            Err(error) => error,
        });
    });
    window.present();
}
