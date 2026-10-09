use gtk::{
    Align, Application, ApplicationWindow, Box as GtkBox, Button, Entry, Label, Orientation,
    ScrolledWindow, Stack, prelude::*,
};
use zfg::core::{
    config::{Action, Settings},
    i18n::text as tr,
    store::{AddFocusResult, Snapshot, Store},
};

pub(crate) fn show(app: &Application, store: Store, settings: Settings) {
    let window = ApplicationWindow::builder()
        .application(app)
        .title("ZFG")
        .default_width(340)
        .default_height(220)
        .decorated(false)
        .resizable(false)
        .modal(true)
        .build();
    window.add_css_class("zfg-launcher");
    let css = gtk::CssProvider::new();
    css.load_from_data(
        "window.zfg-launcher { background: transparent; box-shadow: none; }\n         .zfg-launcher > contents { background: transparent; box-shadow: none; }\n         .zfg-launcher button.zfg-pie-segment { background: transparent; background-image: none; box-shadow: none; border: none; padding: 0; color: #eef1f5; }\n         .zfg-launcher button.zfg-pie-segment:disabled { opacity: 0.65; }\n         .zfg-launcher button:not(.zfg-pie-segment) { background: @window_bg_color; color: @window_fg_color; border-radius: 12px; }\n         .zfg-launcher .zfg-load { background: @window_bg_color; color: @window_fg_color; border-radius: 10px; padding: 4px 10px; }\n         .zfg-launcher entry { background: @window_bg_color; color: @window_fg_color; border-radius: 12px; padding: 12px; }",
    );
    gtk::style_context_add_provider_for_display(
        &gtk::prelude::WidgetExt::display(&window),
        &css,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
    let animate = settings.appearance.animations;
    let stack = Stack::new();
    let update_transition = {
        let stack = stack.clone();
        move |gtk_settings: &gtk::Settings| {
            stack.set_transition_type(if animate && gtk_settings.is_gtk_enable_animations() {
                gtk::StackTransitionType::Crossfade
            } else {
                gtk::StackTransitionType::None
            });
            stack.set_transition_duration(140);
        }
    };
    let gtk_settings = window.settings();
    update_transition(&gtk_settings);
    gtk_settings.connect_gtk_enable_animations_notify(update_transition);
    stack.set_vhomogeneous(false);
    stack.set_hhomogeneous(false);
    let main = GtkBox::new(Orientation::Vertical, 16);
    main.set_margin_top(24);
    main.set_margin_bottom(24);
    main.set_margin_start(24);
    main.set_margin_end(24);

    let task_status = store.status().ok();
    let full_badge = task_status
        .as_ref()
        .filter(|status| status.focus_full)
        .map(|status| format!("  {}/{}", status.focus_count, status.focus_limit))
        .unwrap_or_default();
    let labels = vec![
        format!("💡 {} [1]", tr(&settings.language, "pie.note")),
        format!("▦ {} [2]", tr(&settings.language, "pie.calendar")),
        format!("✓ {} [3]{full_badge}", tr(&settings.language, "pie.task")),
    ];
    let parent_pie = crate::ui::pie::build(&labels, None, 142.0, true);
    let note = parent_pie.segments[0].clone();
    let calendar = parent_pie.segments[1].clone();
    let task = parent_pie.segments[2].clone();
    if task_status.as_ref().is_some_and(|status| status.focus_full) {
        task.add_css_class("warning");
    }
    let cancel = parent_pie.cancel.clone();
    cancel.connect_clicked({
        let window = window.clone();
        move |_| window.close()
    });
    note.connect_clicked({
        let action = settings.note_action.clone();
        let language = settings.language.clone();
        let close_after_action = settings.close_after_action;
        let window = window.clone();
        move |_| {
            if launch_action(&action, &language) && close_after_action {
                window.close();
            }
        }
    });
    calendar.connect_clicked({
        let action = settings.calendar_action.clone();
        let language = settings.language.clone();
        let close_after_action = settings.close_after_action;
        let window = window.clone();
        move |_| {
            if launch_action(&action, &language) && close_after_action {
                window.close();
            }
        }
    });
    let child = make_task_child(
        store.clone(),
        stack.clone(),
        window.clone(),
        settings.close_after_action,
        settings.language.clone(),
    );
    task.connect_clicked({
        let stack = stack.clone();
        move |_| stack.set_visible_child_name("task")
    });
    main.append(&parent_pie.root);

    stack.add_named(&main, Some("main"));
    stack.add_named(&child, Some("task"));
    stack.set_visible_child_name("main");
    stack.connect_visible_child_name_notify({
        let child = child.clone();
        move |stack| {
            if stack.visible_child_name().as_deref() == Some("main") {
                for name in ["task-input", "task-feedback", "task-add"] {
                    if let Some(widget) = find_widget(child.upcast_ref(), name) {
                        widget.set_visible(false);
                    }
                }
                for name in ["focus-target", "later-target"] {
                    if let Some(widget) = find_widget(child.upcast_ref(), name) {
                        widget.remove_css_class("selected");
                    }
                }
            }
        }
    });
    if gtk4_layer_shell::is_supported() {
        use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
        window.init_layer_shell();
        window.set_namespace(Some("zfg-launcher"));
        // GTK can identify an application surface's monitor, but not a global pointer monitor.
        let display = gtk::prelude::WidgetExt::display(&window);
        let monitor = app
            .active_window()
            .and_then(|active| active.surface())
            .and_then(|surface| display.monitor_at_surface(&surface))
            .or_else(|| {
                display
                    .monitors()
                    .item(0)
                    .and_downcast::<gtk::gdk::Monitor>()
            });
        window.set_resizable(true);
        if let Some(monitor) = monitor {
            window.set_monitor(Some(&monitor));
            let geometry = monitor.geometry();
            window.set_default_size(geometry.width(), geometry.height());
        }
        window.set_layer(Layer::Overlay);
        window.set_keyboard_mode(KeyboardMode::Exclusive);
        for edge in [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right] {
            window.set_anchor(edge, true);
        }
        // A modal, transparent catcher consumes the dismissal click, like a popup.
        // No pointer events or global hooks are observed outside this surface.
        let surface = gtk::Overlay::new();
        let background = GtkBox::new(Orientation::Vertical, 0);
        background.set_hexpand(true);
        background.set_vexpand(true);
        surface.set_child(Some(&background));
        stack.set_halign(Align::Center);
        stack.set_valign(Align::Start);
        stack.set_margin_top(52);
        surface.add_overlay(&stack);
        surface.set_measure_overlay(&stack, true);
        let dismiss = gtk::GestureClick::new();
        dismiss.set_propagation_phase(gtk::PropagationPhase::Capture);
        dismiss.connect_pressed({
            let surface = surface.downgrade();
            let stack = stack.downgrade();
            let window = window.downgrade();
            move |_, _, x, y| {
                let (Some(surface), Some(stack), Some(window)) =
                    (surface.upgrade(), stack.upgrade(), window.upgrade())
                else {
                    return;
                };
                let inside = surface
                    .pick(x, y, gtk::PickFlags::DEFAULT)
                    .is_some_and(|widget| widget == stack || widget.is_ancestor(&stack));
                if !inside {
                    window.close();
                }
            }
        });
        surface.add_controller(dismiss);
        window.set_child(Some(&surface));
    } else {
        window.set_child(Some(&stack));
    }
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    {
        let window = window.clone();
        let stack = stack.clone();
        let note = note.clone();
        let calendar = calendar.clone();
        let task = task.clone();
        let child_focus = find_button(child.upcast_ref(), "focus-target");
        let child_later = find_button(child.upcast_ref(), "later-target");
        keys.connect_key_pressed(move |_, key, _, _| {
            let typing =
                gtk::prelude::RootExt::focus(&window).is_some_and(|widget| entry_ancestor(&widget));
            if typing && !matches!(key, gtk::gdk::Key::Escape) {
                return false.into();
            }
            match key {
                gtk::gdk::Key::Escape => {
                    if stack.visible_child_name().as_deref() == Some("task") {
                        if let Some(entry) = find_widget(child.upcast_ref(), "task-input")
                            .filter(|entry| entry.is_visible())
                        {
                            entry.set_visible(false);
                            for name in ["task-feedback", "task-add"] {
                                if let Some(widget) = find_widget(child.upcast_ref(), name) {
                                    widget.set_visible(false);
                                }
                            }
                            if let Some(target) = &child_focus {
                                target.grab_focus();
                            }
                        } else {
                            stack.set_visible_child_name("main");
                            task.grab_focus();
                        }
                    } else {
                        window.close();
                    }
                    true.into()
                }
                gtk::gdk::Key::_1 => {
                    note.emit_clicked();
                    true.into()
                }
                gtk::gdk::Key::_2 => {
                    calendar.emit_clicked();
                    true.into()
                }
                gtk::gdk::Key::_3 => {
                    task.emit_clicked();
                    true.into()
                }
                gtk::gdk::Key::f | gtk::gdk::Key::F => {
                    if stack.visible_child_name().as_deref() == Some("task") {
                        if let Some(target) = &child_focus {
                            target.emit_clicked();
                        }
                    } else {
                        task.emit_clicked();
                    }
                    true.into()
                }
                gtk::gdk::Key::l | gtk::gdk::Key::L => {
                    if stack.visible_child_name().as_deref() == Some("task") {
                        if let Some(target) = &child_later {
                            target.emit_clicked();
                        }
                    } else {
                        task.emit_clicked();
                    }
                    true.into()
                }
                _ => false.into(),
            }
        });
    }
    window.add_controller(keys);
    install_fade(&window, animate);
    window.present();
}

fn make_task_child(
    store: Store,
    stack: Stack,
    window: ApplicationWindow,
    close_after_action: bool,
    language: String,
) -> GtkBox {
    let child = GtkBox::new(Orientation::Vertical, 12);
    child.set_spacing(6);
    child.set_margin_top(10);
    child.set_margin_bottom(10);
    child.set_margin_start(16);
    child.set_margin_end(16);
    let parent_labels = vec![
        format!("💡 {}", tr(&language, "pie.note")),
        format!("▦ {}", tr(&language, "pie.calendar")),
        format!("✓ {}", tr(&language, "pie.task")),
    ];
    let parent_pie = crate::ui::pie::build(&parent_labels, None, 142.0, false);
    for segment in &parent_pie.segments {
        segment.set_sensitive(false);
    }
    child.append(&parent_pie.root);
    let connector = Label::new(Some("│"));
    connector.set_halign(Align::Center);
    child.append(&connector);

    if let Ok(status) = store.status() {
        let load = Label::new(Some(
            &tr(&language, "focus.load")
                .replace("{count}", &status.focus_count.to_string())
                .replace("{limit}", &status.focus_limit.to_string()),
        ));
        load.set_halign(Align::Start);
        if status.focus_full {
            load.add_css_class("error");
        }
        load.add_css_class("zfg-load");
        child.append(&load);
    }
    let status = store.status().ok();
    let focus_is_full = status.as_ref().is_some_and(|status| status.focus_full);
    let focus_badge = status
        .as_ref()
        .filter(|status| status.focus_full)
        .map(|status| format!(" {}/{}", status.focus_count, status.focus_limit))
        .unwrap_or_default();
    let focus_label = format!("◉ {} [F]{focus_badge}", tr(&language, "pie.focus"));
    let later_label = format!("≡ {} [L]", tr(&language, "pie.later"));
    let child_pie = crate::ui::pie::build(
        &[focus_label, later_label],
        focus_is_full.then_some(0),
        105.0,
        false,
    );
    let focus = child_pie.segments[0].clone();
    let later = child_pie.segments[1].clone();
    focus.set_widget_name("focus-target");
    later.set_widget_name("later-target");
    focus.set_tooltip_text(
        focus_is_full
            .then(|| tr(&language, "focus.full_hint"))
            .as_deref(),
    );
    parent_pie.segments[2].add_css_class("selected");
    let selected_focus = std::rc::Rc::new(std::cell::Cell::new(true));
    child.append(&child_pie.root);
    let back = Button::with_label(&tr(&language, "launcher.back"));
    back.set_halign(Align::Center);
    {
        let stack = stack.clone();
        back.connect_clicked(move |_| stack.set_visible_child_name("main"));
    }
    child.append(&back);
    let entry = Entry::new();
    entry.set_visible(false);
    entry.set_widget_name("task-input");
    entry.set_placeholder_text(Some(&tr(&language, "task.placeholder")));
    child.append(&entry);

    let feedback = Label::new(None);
    feedback.set_widget_name("task-feedback");
    feedback.set_halign(Align::Start);
    feedback.set_visible(false);
    child.append(&feedback);
    let add = Button::with_label(&tr(&language, "task.add"));
    add.set_widget_name("task-add");
    add.set_halign(Align::Start);
    add.set_visible(false);
    child.append(&add);
    focus.connect_clicked({
        let selected = selected_focus.clone();
        let focus = focus.clone();
        let later = later.clone();
        let entry = entry.clone();
        let feedback = feedback.clone();
        let add = add.clone();
        move |_| {
            selected.set(true);
            entry.set_visible(true);
            focus.add_css_class("selected");
            later.remove_css_class("selected");
            feedback.set_visible(true);
            add.set_visible(true);
            entry.grab_focus();
        }
    });
    later.connect_clicked({
        let selected = selected_focus.clone();
        let focus = focus.clone();
        let later = later.clone();
        let entry = entry.clone();
        let feedback = feedback.clone();
        let add = add.clone();
        move |_| {
            selected.set(false);
            entry.set_visible(true);
            later.add_css_class("selected");
            focus.remove_css_class("selected");
            feedback.set_visible(true);
            add.set_visible(true);
            entry.grab_focus();
        }
    });

    {
        let store = store.clone();
        let focus_selected = selected_focus.clone();
        let entry = entry.clone();
        let feedback = feedback.clone();
        let window = window.clone();
        add.connect_clicked(move |_| {
            let text = entry.text().to_string();
            if text.trim().is_empty() {
                return;
            }
            if focus_selected.get() {
                match store.add_focus(&text) {
                    Ok(AddFocusResult::Added) => {
                        entry.set_text("");
                        feedback.set_text(&tr(&language, "task.added_focus"));
                        let feedback = feedback.clone();
                        let window = window.clone();
                        gtk::glib::timeout_add_local_once(
                            std::time::Duration::from_millis(850),
                            move || {
                                feedback.set_text("");
                                if close_after_action {
                                    window.close();
                                }
                            },
                        );
                    }
                    Ok(AddFocusResult::Full { current }) => replacement_chooser(
                        &store,
                        &text,
                        current,
                        Some(window.clone()),
                        &language,
                        close_after_action,
                    ),
                    Err(e) => message(&language, &tr(&language, "list.add_error"), &e.to_string()),
                }
            } else {
                match store.add_later(&text) {
                    Ok(()) => {
                        entry.set_text("");
                        feedback.set_text(&tr(&language, "task.added_later"));
                        let feedback = feedback.clone();
                        let window = window.clone();
                        gtk::glib::timeout_add_local_once(
                            std::time::Duration::from_millis(850),
                            move || {
                                feedback.set_text("");
                                if close_after_action {
                                    window.close();
                                }
                            },
                        );
                    }
                    Err(e) => message(&language, &tr(&language, "list.add_error"), &e.to_string()),
                }
            }
        });
    }
    entry.connect_activate({
        let add = add.clone();
        move |_| add.emit_clicked()
    });
    child
}

fn entry_ancestor(widget: &gtk::Widget) -> bool {
    let mut current = Some(widget.clone());
    while let Some(widget) = current {
        if widget.is::<Entry>() {
            return true;
        }
        current = widget.parent();
    }
    false
}

fn install_fade(window: &ApplicationWindow, enabled: bool) {
    let closing = std::rc::Rc::new(std::cell::Cell::new(false));
    window.connect_map(move |window| {
        if enabled && window.settings().is_gtk_enable_animations() {
            window.set_opacity(0.0);
            fade(window, false);
        }
    });
    window.connect_close_request(move |window| {
        if closing.replace(true) {
            return true.into();
        }
        if enabled && window.settings().is_gtk_enable_animations() {
            window.set_sensitive(false);
            fade(window, true);
            true.into()
        } else {
            false.into()
        }
    });
}

fn fade(window: &ApplicationWindow, closing: bool) {
    let start = std::time::Instant::now();
    let opacity = window.opacity();
    window.add_tick_callback(move |window, _| {
        // A close can interrupt the opening fade.
        if !closing && !window.is_sensitive() {
            return gtk::glib::ControlFlow::Break;
        }
        let progress = if window.settings().is_gtk_enable_animations() {
            (start.elapsed().as_secs_f64() / 0.14).min(1.0)
        } else {
            1.0
        };
        window.set_opacity(if closing {
            opacity * (1.0 - progress)
        } else {
            progress
        });
        if progress >= 1.0 {
            if closing {
                window.destroy();
            }
            gtk::glib::ControlFlow::Break
        } else {
            gtk::glib::ControlFlow::Continue
        }
    });
}

fn find_button(root: &gtk::Widget, name: &str) -> Option<Button> {
    find_widget(root, name)?.downcast::<Button>().ok()
}

fn find_widget(root: &gtk::Widget, name: &str) -> Option<gtk::Widget> {
    if root.widget_name() == name {
        return Some(root.clone());
    }
    let mut child = root.first_child();
    while let Some(widget) = child {
        if let Some(found) = find_widget(&widget, name) {
            return Some(found);
        }
        child = widget.next_sibling();
    }
    None
}

fn launch_action(action: &Action, language: &str) -> bool {
    match zfg::integrations::launch(action, language) {
        Ok(()) => true,
        Err(error) => {
            message(
                language,
                &tr(language, "integration.failed"),
                &error.to_string(),
            );
            false
        }
    }
}

pub(crate) fn replace_from_later(
    store: Store,
    later_index: usize,
    snapshot: Snapshot,
    refresh: std::rc::Rc<dyn Fn()>,
) {
    let language = Settings::load().unwrap_or_default().language;
    if snapshot.focus.is_empty() {
        message(
            &language,
            &tr(&language, "focus.empty"),
            &tr(&language, "focus.empty_hint"),
        );
        return;
    }
    let dialog = gtk::Dialog::builder()
        .title(tr(&language, "focus.full"))
        .modal(true)
        .default_width(460)
        .build();
    let body = GtkBox::new(Orientation::Vertical, 8);
    body.set_margin_top(16);
    body.set_margin_bottom(16);
    body.set_margin_start(16);
    body.set_margin_end(16);
    let heading = Label::new(Some(&tr(&language, "list.full_hint")));
    heading.set_wrap(true);
    body.append(&heading);
    let list = gtk::ListBox::new();
    list.add_css_class("boxed-list");
    let mut choices = Vec::new();
    for (focus_index, task) in snapshot.focus.iter().enumerate() {
        let button = Button::with_label(&format!("{}. {task}", focus_index + 1));
        choices.push(button.clone());
        button.set_halign(Align::Fill);
        list.append(&button);
        let store = store.clone();
        let snapshot = snapshot.clone();
        let dialog = dialog.clone();
        let refresh = refresh.clone();
        let language = language.clone();
        button.connect_clicked(move |_| {
            match store.replace_focus_from_later(focus_index, later_index, &snapshot) {
                Ok(()) => {
                    dialog.close();
                    refresh();
                }
                Err(error) => message(
                    &language,
                    &tr(&language, "focus.changed"),
                    &error.to_string(),
                ),
            }
        });
    }
    let scroll = ScrolledWindow::builder()
        .min_content_height(180)
        .child(&list)
        .build();
    body.append(&scroll);
    install_choice_keys(&dialog, &choices, None);
    dialog.content_area().append(&body);
    dialog.add_button(&tr(&language, "list.cancel"), gtk::ResponseType::Cancel);
    dialog.connect_response(|d, _| d.close());
    prepare_dialog(&dialog);
    dialog.present();
}

pub(crate) fn choose_focus_replacement(store: Store, incoming: String) {
    let settings = Settings::load().unwrap_or_default();
    let language = &settings.language;
    match store.snapshot() {
        Ok(snapshot) => replacement_chooser(
            &store,
            &incoming,
            snapshot.focus,
            None,
            language,
            settings.close_after_action,
        ),
        Err(error) => message(
            language,
            &tr(language, "focus.read_error"),
            &error.to_string(),
        ),
    }
}

fn replacement_chooser(
    store: &Store,
    incoming: &str,
    items: Vec<String>,
    window: Option<ApplicationWindow>,
    language: &str,
    close_after_action: bool,
) {
    let snapshot = match store.snapshot() {
        Ok(snapshot) if snapshot.focus == items => snapshot,
        Ok(_) => {
            message(
                language,
                &tr(language, "focus.changed"),
                &tr(language, "focus.changed_hint"),
            );
            return;
        }
        Err(error) => {
            message(
                language,
                &tr(language, "focus.read_error"),
                &error.to_string(),
            );
            return;
        }
    };
    if items.is_empty() {
        message(
            language,
            &tr(language, "focus.empty"),
            &tr(language, "focus.empty_hint"),
        );
        return;
    }
    let dialog = gtk::Dialog::builder()
        .title(tr(language, "focus.replace_title"))
        .modal(true)
        .default_width(460)
        .build();
    let body = GtkBox::new(Orientation::Vertical, 8);
    body.set_margin_top(16);
    body.set_margin_bottom(16);
    body.set_margin_start(16);
    body.set_margin_end(16);
    let description = Label::new(Some(&tr(language, "focus.replace_hint")));
    description.set_wrap(true);
    body.append(&description);
    let list = gtk::ListBox::new();
    list.add_css_class("boxed-list");
    let mut choices = Vec::new();
    for (index, task) in items.iter().enumerate() {
        let button = Button::with_label(&format!("{}. {task}", index + 1));
        choices.push(button.clone());
        button.set_halign(Align::Fill);
        list.append(&button);
        let store = store.clone();
        let incoming = incoming.to_string();
        let snapshot = snapshot.clone();
        let dialog = dialog.clone();
        let launcher = window.clone().filter(|_| close_after_action);
        let language = language.to_string();
        button.connect_clicked(
            move |_| match store.replace_focus(index, &incoming, &snapshot) {
                Ok(()) => {
                    dialog.close();
                    if let Some(window) = &launcher {
                        window.close();
                    }
                }
                Err(e) => message(
                    &language,
                    &tr(&language, "focus.replace_error"),
                    &e.to_string(),
                ),
            },
        );
    }
    let send_later = Button::with_label(&tr(language, "focus.send_later"));
    let store_for_later = store.clone();
    let incoming_for_later = incoming.to_string();
    let dialog_for_later = dialog.clone();
    let launcher_for_later = window.clone().filter(|_| close_after_action);
    let language_for_later = language.to_string();
    send_later.connect_clicked(
        move |_| match store_for_later.add_later(&incoming_for_later) {
            Ok(()) => {
                dialog_for_later.close();
                if let Some(window) = &launcher_for_later {
                    window.close();
                }
            }
            Err(error) => message(
                &language_for_later,
                &tr(&language_for_later, "list.add_error"),
                &error.to_string(),
            ),
        },
    );
    install_choice_keys(&dialog, &choices, Some(&send_later));
    send_later.set_tooltip_text(Some("L"));
    body.append(&send_later);
    let scroll = ScrolledWindow::builder()
        .min_content_height(180)
        .child(&list)
        .build();
    body.append(&scroll);
    dialog.content_area().append(&body);
    dialog.add_button(&tr(language, "list.cancel"), gtk::ResponseType::Cancel);
    dialog.connect_response(|d, _| d.close());
    prepare_dialog(&dialog);
    dialog.present();
}

fn message(language: &str, title: &str, text: &str) {
    let dialog = gtk::Dialog::builder()
        .title(title)
        .modal(true)
        .default_width(400)
        .build();
    let label = Label::new(Some(text));
    label.set_wrap(true);
    label.set_margin_top(16);
    label.set_margin_bottom(16);
    label.set_margin_start(16);
    label.set_margin_end(16);
    dialog.content_area().append(&label);
    dialog.add_button(&tr(language, "error.ok"), gtk::ResponseType::Close);
    dialog.connect_response(|d, _| d.close());
    prepare_dialog(&dialog);
    dialog.present();
}

fn install_choice_keys(dialog: &gtk::Dialog, choices: &[Button], later: Option<&Button>) {
    let choices: Vec<_> = choices.iter().map(|button| button.downgrade()).collect();
    let later = later.map(|button| button.downgrade());
    let weak_dialog = dialog.downgrade();
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    keys.connect_key_pressed(move |_, key, _, _| {
        if key == gtk::gdk::Key::Escape {
            if let Some(dialog) = weak_dialog.upgrade() {
                dialog.close();
            }
            return true.into();
        }
        let choice = key
            .to_unicode()
            .and_then(|character| character.to_digit(10))
            .and_then(|number| number.checked_sub(1))
            .and_then(|index| choices.get(index as usize))
            .and_then(|button| button.upgrade());
        let choice = choice.or_else(|| {
            if matches!(key, gtk::gdk::Key::l | gtk::gdk::Key::L) {
                later.as_ref().and_then(|button| button.upgrade())
            } else {
                None
            }
        });
        if let Some(button) = choice {
            button.emit_clicked();
            true.into()
        } else {
            false.into()
        }
    });
    dialog.add_controller(keys);
}

fn prepare_dialog(dialog: &gtk::Dialog) {
    if let Some(app) = gtk::gio::Application::default().and_downcast::<Application>() {
        if let Some(parent) = app.active_window().filter(|parent| parent != dialog) {
            dialog.set_transient_for(Some(&parent));
        }
        dialog.set_application(Some(&app));
    }
    if gtk4_layer_shell::is_supported() {
        use gtk4_layer_shell::{KeyboardMode, Layer, LayerShell};
        dialog.init_layer_shell();
        dialog.set_namespace(Some("zfg-dialog"));
        dialog.set_layer(Layer::Overlay);
        dialog.set_keyboard_mode(KeyboardMode::Exclusive);
    }
}
