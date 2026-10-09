mod launcher;
mod pie;
mod settings;
mod shortcuts;
mod windows;

use anyhow::Result;
use gtk::Application;
use gtk::prelude::*;
use zfg::core::{config::Settings, store::Store};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiMode {
    Launcher,
    Focus,
    Later,
    Settings,
}

pub fn run(store: Store, settings: Settings, mode: UiMode) -> Result<()> {
    let app = Application::builder()
        .application_id("io.github.zfg")
        .flags(gtk::gio::ApplicationFlags::HANDLES_COMMAND_LINE)
        .build();
    let launcher_action = gtk::gio::SimpleAction::new("launcher", None);
    let data_dir = store.data_dir().to_path_buf();
    let weak_app = app.downgrade();
    launcher_action.connect_activate(move |_, _| {
        let Some(app) = weak_app.upgrade() else {
            return;
        };
        match Settings::load().and_then(|settings| {
            let store = Store::new(data_dir.clone(), settings.focus_limit)?;
            for window in app.windows() {
                window.close();
            }
            launcher::show(&app, store, settings);
            Ok(())
        }) {
            Ok(()) => {}
            Err(error) => eprintln!("Could not open launcher: {error}"),
        }
    });
    app.add_action(&launcher_action);
    app.connect_command_line(move |app, command_line| {
        let requested = command_line
            .arguments()
            .last()
            .map(|arg| arg.to_string_lossy().into_owned());
        let mode = requested
            .as_deref()
            .and_then(|value| match value {
                "launcher" => Some(UiMode::Launcher),
                "focus" => Some(UiMode::Focus),
                "later" => Some(UiMode::Later),
                "settings" => Some(UiMode::Settings),
                _ => None,
            })
            .unwrap_or(mode);
        for window in app.windows() {
            window.close();
        }
        let current_settings = match Settings::load() {
            Ok(value) => value,
            Err(error) => {
                eprintln!("Could not reload settings: {error}");
                settings.clone()
            }
        };
        let current_store =
            match Store::new(store.data_dir().to_path_buf(), current_settings.focus_limit) {
                Ok(value) => value,
                Err(error) => {
                    eprintln!("Could not open task storage: {error}");
                    return gtk::glib::ExitCode::FAILURE;
                }
            };
        if let Some(gtk_settings) = gtk::Settings::default() {
            match current_settings.appearance.theme.as_str() {
                "dark" => gtk_settings.set_gtk_application_prefer_dark_theme(true),
                "light" => gtk_settings.set_gtk_application_prefer_dark_theme(false),
                _ => gtk_settings.reset_property("gtk-application-prefer-dark-theme"),
            }
            if current_settings.appearance.animations {
                gtk_settings.reset_property("gtk-enable-animations");
            } else {
                gtk_settings.set_gtk_enable_animations(false);
            }
        }
        match mode {
            UiMode::Launcher => app.activate_action("launcher", None),
            UiMode::Focus => windows::show_list(
                app,
                current_store.clone(),
                zfg::core::store::List::Focus,
                &current_settings.language,
            ),
            UiMode::Later => windows::show_list(
                app,
                current_store.clone(),
                zfg::core::store::List::Later,
                &current_settings.language,
            ),
            UiMode::Settings => settings::show(app, current_settings.clone()),
        }
        gtk::glib::ExitCode::SUCCESS
    });
    let action = match mode {
        UiMode::Launcher => "launcher",
        UiMode::Focus => "focus",
        UiMode::Later => "later",
        UiMode::Settings => "settings",
    };
    app.run_with_args(&["zfg", action]);
    Ok(())
}
