use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};

use gtk::{
    Align, Application, ApplicationWindow, Box as GtkBox, Button, Dialog, Entry, Label, ListBox,
    ListBoxRow, Orientation, ResponseType, ScrolledWindow, gio, glib, prelude::*,
};
use zfg::core::{
    i18n::text as tr,
    store::{List, Snapshot, Store},
};

type Refresh = Rc<dyn Fn()>;
type RefreshSlot = Rc<RefCell<Option<Weak<dyn Fn()>>>>;

pub(crate) fn show_list(app: &Application, store: Store, list: List, language: &str) {
    let title_key = if list == List::Focus {
        "list.focus"
    } else {
        "list.later"
    };
    let title = tr(language, title_key);
    let window = ApplicationWindow::builder()
        .application(app)
        .title(&title)
        .default_width(520)
        .default_height(560)
        .build();
    let root = GtkBox::new(Orientation::Vertical, 12);
    root.set_margin_top(18);
    root.set_margin_bottom(18);
    root.set_margin_start(18);
    root.set_margin_end(18);
    let heading = Label::new(Some(&title));
    heading.add_css_class("title-1");
    heading.set_halign(Align::Start);
    root.append(&heading);
    let status = Label::new(None);
    status.set_halign(Align::Start);
    root.append(&status);
    let rows = ListBox::new();
    rows.add_css_class("boxed-list");
    rows.set_selection_mode(gtk::SelectionMode::Single);
    let scroll = ScrolledWindow::builder().vexpand(true).child(&rows).build();
    root.append(&scroll);
    let add_row = GtkBox::new(Orientation::Horizontal, 8);
    let entry = Entry::new();
    entry.set_placeholder_text(Some(&tr(language, "list.add_placeholder")));
    entry.set_hexpand(true);
    let add = Button::with_label(&tr(language, "list.add"));
    add_row.append(&entry);
    add_row.append(&add);
    root.append(&add_row);
    window.set_child(Some(&root));

    let store = Rc::new(store);
    let list = Rc::new(list);
    let refresh_slot: RefreshSlot = Rc::new(RefCell::new(None));
    let displayed_snapshot = Rc::new(RefCell::new(None::<Snapshot>));
    let language = language.to_owned();
    let refresh: Refresh = Rc::new({
        let language = language.clone();
        let displayed_snapshot = displayed_snapshot.clone();
        let refresh_slot = refresh_slot.clone();
        let rows = rows.clone();
        let status = status.clone();
        let store = store.clone();
        let list = list.clone();
        let last_snapshot = RefCell::new(None::<Snapshot>);
        move || match store.snapshot() {
            Ok(snapshot) => {
                if last_snapshot.borrow().as_ref() == Some(&snapshot) {
                    return;
                }
                while let Some(child) = rows.first_child() {
                    rows.remove(&child);
                }
                let items = if *list == List::Focus {
                    &snapshot.focus
                } else {
                    &snapshot.later
                };
                if *list == List::Focus {
                    if let Ok(s) = store.status() {
                        status.set_text(
                            &tr(&language, "list.load")
                                .replace("{count}", &s.focus_count.to_string())
                                .replace("{limit}", &s.focus_limit.to_string()),
                        );
                    }
                } else {
                    status.set_text(
                        &tr(&language, "list.count").replace("{count}", &items.len().to_string()),
                    );
                }
                for (index, task) in items.iter().enumerate() {
                    let refresh_slot = refresh_slot.clone();
                    let refresh: Rc<dyn Fn()> = Rc::new(move || {
                        let refresh = refresh_slot.borrow().as_ref().and_then(Weak::upgrade);
                        if let Some(refresh) = refresh {
                            refresh();
                        }
                    });
                    rows.append(&task_row(
                        &store,
                        *list,
                        index,
                        task,
                        &snapshot,
                        refresh,
                        language.clone(),
                    ));
                }
                *displayed_snapshot.borrow_mut() = Some(snapshot.clone());
                *last_snapshot.borrow_mut() = Some(snapshot);
            }
            Err(error) => {
                *displayed_snapshot.borrow_mut() = None;
                *last_snapshot.borrow_mut() = None;
                status.set_text(
                    &tr(&language, "list.read_error").replace("{error}", &error.to_string()),
                );
            }
        }
    });
    *refresh_slot.borrow_mut() = Some(Rc::downgrade(&refresh));
    let monitor = gio::File::for_path(store.data_dir())
        .monitor_directory(gio::FileMonitorFlags::WATCH_MOVES, gio::Cancellable::NONE);
    let monitor = match monitor {
        Ok(monitor) => {
            let refresh_slot = refresh_slot.clone();
            monitor.connect_changed(move |_, file, other_file, _| {
                let is_task_file = |file: &gio::File| {
                    file.basename().is_some_and(|name| {
                        name == std::path::Path::new("focus.txt")
                            || name == std::path::Path::new("later.txt")
                    })
                };
                if is_task_file(file) || other_file.is_some_and(is_task_file) {
                    let refresh = refresh_slot.borrow().as_ref().and_then(Weak::upgrade);
                    if let Some(refresh) = refresh {
                        refresh();
                    }
                }
            });
            Some(monitor)
        }
        Err(error) => {
            error_dialog(&language, "list.monitor_error", &error.to_string());
            None
        }
    };
    refresh();
    window.connect_close_request({
        let retained = RefCell::new(Some((refresh.clone(), monitor)));
        move |_| {
            refresh_slot.borrow_mut().take();
            if let Some((_, Some(monitor))) = retained.borrow_mut().take() {
                monitor.cancel();
            }
            glib::Propagation::Proceed
        }
    });
    {
        let store = store.clone();
        let list = list.clone();
        let entry = entry.clone();
        let language = language.clone();
        let refresh = Rc::downgrade(&refresh);
        add.connect_clicked(move |_| {
            let text = entry.text().to_string();
            if text.trim().is_empty() {
                return;
            }
            if *list == List::Focus {
                match store.add_focus(&text) {
                    Ok(zfg::core::store::AddFocusResult::Added) => {
                        entry.set_text("");
                        if let Some(refresh) = refresh.upgrade() {
                            refresh();
                        }
                    }
                    Ok(zfg::core::store::AddFocusResult::Full { .. }) => {
                        crate::ui::launcher::choose_focus_replacement((*store).clone(), text)
                    }
                    Err(error) => error_dialog(&language, "list.add_error", &error.to_string()),
                }
            } else {
                match store.add_later(&text) {
                    Ok(()) => {
                        entry.set_text("");
                        if let Some(refresh) = refresh.upgrade() {
                            refresh();
                        }
                    }
                    Err(error) => error_dialog(&language, "list.add_error", &error.to_string()),
                }
            }
        });
    }
    entry.connect_activate({
        let add = add.clone();
        move |_| add.emit_clicked()
    });
    let keys = gtk::EventControllerKey::new();
    keys.connect_key_pressed({
        let rows = rows.clone();
        let list = list.clone();
        let store = store.clone();
        let refresh = Rc::downgrade(&refresh);
        let displayed_snapshot = displayed_snapshot.clone();
        let language = language.clone();
        move |_, key, _, modifiers| {
            let selected = rows.selected_row();
            let Some(row) = selected else {
                if key == gtk::gdk::Key::Down && rows.row_at_index(0).is_some() {
                    rows.select_row(rows.row_at_index(0).as_ref());
                    return glib::Propagation::Stop;
                }
                return glib::Propagation::Proceed;
            };
            let index = row.index();
            let count = rows.observe_children().n_items() as i32;
            let control = modifiers.contains(gtk::gdk::ModifierType::CONTROL_MASK);
            if control
                && *list == List::Focus
                && (key == gtk::gdk::Key::Up || key == gtk::gdk::Key::Down)
            {
                let to = if key == gtk::gdk::Key::Up {
                    index - 1
                } else {
                    index + 1
                };
                if to >= 0
                    && to < count
                    && let Some(refresh_fn) = refresh.upgrade()
                    && let Some(snapshot) = displayed_snapshot.borrow().as_ref()
                {
                    match store.reorder_focus(index as usize, to as usize, snapshot) {
                        Ok(()) => refresh_fn(),
                        Err(error) => {
                            error_dialog(&language, "list.reorder_error", &error.to_string())
                        }
                    }
                }
                return glib::Propagation::Stop;
            }
            match key {
                gtk::gdk::Key::Up | gtk::gdk::Key::Down => {
                    let to = if key == gtk::gdk::Key::Up {
                        index - 1
                    } else {
                        index + 1
                    };
                    if to >= 0 && to < count {
                        rows.select_row(rows.row_at_index(to).as_ref());
                    }
                    glib::Propagation::Stop
                }
                gtk::gdk::Key::Return | gtk::gdk::Key::KP_Enter => {
                    activate_row_button(&row, 1);
                    glib::Propagation::Stop
                }
                gtk::gdk::Key::space => {
                    activate_row_button(&row, 3);
                    glib::Propagation::Stop
                }
                gtk::gdk::Key::Delete => {
                    activate_row_button(&row, 4);
                    glib::Propagation::Stop
                }
                _ => glib::Propagation::Proceed,
            }
        }
    });
    window.add_controller(keys);
    window.present();
}

fn task_row(
    store: &Rc<Store>,
    list: List,
    index: usize,
    task: &str,
    snapshot: &Snapshot,
    refresh: Rc<dyn Fn()>,
    language: String,
) -> ListBoxRow {
    let row = ListBoxRow::new();
    let line = GtkBox::new(Orientation::Horizontal, 8);
    line.set_margin_top(8);
    line.set_margin_bottom(8);
    line.set_margin_start(10);
    line.set_margin_end(10);
    let label = Label::new(Some(task));
    label.set_halign(Align::Start);
    label.set_hexpand(true);
    label.set_wrap(true);
    line.append(&label);
    let edit = Button::with_label(&tr(&language, "list.edit"));
    let move_button = Button::with_label(&tr(
        &language,
        if list == List::Focus {
            "list.move_later"
        } else {
            "list.move_focus"
        },
    ));
    let done = Button::with_label(&tr(&language, "list.done"));
    let delete = Button::with_label(&tr(&language, "list.delete"));
    for button in [&edit, &move_button, &done, &delete] {
        button.set_valign(Align::Center);
        line.append(button);
    }
    {
        let store = store.clone();
        let original = task.to_string();
        let snap = snapshot.clone();
        let refresh = refresh.clone();
        let language = language.clone();
        edit.connect_clicked(move |_| {
            let refresh = refresh.clone();
            let store = store.clone();
            let snap = snap.clone();
            let error_language = language.clone();
            edit_dialog(&language, &original, move |text| {
                match store.edit(list, index, &text, &snap) {
                    Ok(()) => refresh(),
                    Err(e) => error_dialog(&error_language, "list.edit_error", &e.to_string()),
                }
            })
        });
    }
    {
        let store = store.clone();
        let snap = snapshot.clone();
        let refresh = refresh.clone();
        let language = language.clone();
        move_button.connect_clicked(move |_| match store.move_task(list, index, &snap) {
            Ok(zfg::core::store::AddFocusResult::Added) => refresh(),
            Ok(zfg::core::store::AddFocusResult::Full { .. }) => {
                if list == List::Later {
                    crate::ui::launcher::replace_from_later(
                        (*store).clone(),
                        index,
                        snap.clone(),
                        refresh.clone(),
                    );
                }
            }
            Err(e) => error_dialog(&language, "list.move_error", &e.to_string()),
        });
    }
    {
        let done_store = store.clone();
        let done_snap = snapshot.clone();
        let done_refresh = refresh.clone();
        let done_language = language.clone();
        done.connect_clicked(
            move |_| match done_store.complete(list, index, &done_snap) {
                Ok(()) => done_refresh(),
                Err(e) => error_dialog(&done_language, "list.complete_error", &e.to_string()),
            },
        );
        let store = store.clone();
        let snap = snapshot.clone();
        let text = task.to_string();
        let refresh = refresh.clone();
        let language = language.clone();
        delete.connect_clicked(move |_| {
            let refresh = refresh.clone();
            let store = store.clone();
            let snap = snap.clone();
            let delete_language = language.clone();
            confirm_delete(&language, &text, move || {
                match store.complete(list, index, &snap) {
                    Ok(()) => refresh(),
                    Err(e) => error_dialog(&delete_language, "list.delete_error", &e.to_string()),
                }
            })
        });
    }
    row.set_child(Some(&line));
    row
}

fn activate_row_button(row: &ListBoxRow, index: i32) {
    if let Some(line) = row.child().and_downcast::<GtkBox>()
        && let Some(button) = line
            .observe_children()
            .item(index as u32)
            .and_downcast::<Button>()
    {
        button.emit_clicked();
    }
}

fn edit_dialog(language: &str, initial: &str, save: impl Fn(String) + 'static) {
    let dialog = Dialog::builder()
        .title(tr(language, "list.edit_title"))
        .modal(true)
        .default_width(400)
        .build();
    let entry = Entry::new();
    entry.set_text(initial);
    entry.set_margin_top(16);
    entry.set_margin_bottom(16);
    entry.set_margin_start(16);
    entry.set_margin_end(16);
    dialog.content_area().append(&entry);
    dialog.add_button(&tr(language, "list.cancel"), ResponseType::Cancel);
    dialog.add_button(&tr(language, "list.save"), ResponseType::Accept);
    dialog.connect_response(move |dialog, response| {
        if response == ResponseType::Accept {
            save(entry.text().to_string());
        }
        dialog.close();
    });
    dialog.present();
}

fn confirm_delete(language: &str, task: &str, delete: impl Fn() + 'static) {
    let dialog = Dialog::builder()
        .title(tr(language, "list.delete_title"))
        .modal(true)
        .default_width(380)
        .build();
    let label = Label::new(Some(
        &tr(language, "list.delete_confirm").replace("{task}", task),
    ));
    label.set_wrap(true);
    label.set_margin_top(18);
    label.set_margin_bottom(18);
    label.set_margin_start(18);
    label.set_margin_end(18);
    dialog.content_area().append(&label);
    dialog.add_button(&tr(language, "list.cancel"), ResponseType::Cancel);
    dialog.add_button(&tr(language, "list.delete"), ResponseType::Accept);
    dialog.connect_response(move |dialog, response| {
        if response == ResponseType::Accept {
            delete();
        }
        dialog.close();
    });
    dialog.present();
}

fn error_dialog(language: &str, title_key: &str, message: &str) {
    let dialog = Dialog::builder()
        .title(tr(language, "error.title"))
        .modal(true)
        .default_width(400)
        .build();
    let summary = tr(language, title_key);
    let message = if summary.contains("{error}") {
        summary.replace("{error}", message)
    } else {
        format!("{summary}: {message}")
    };
    let label = Label::new(Some(&message));
    label.set_wrap(true);
    label.set_margin_top(16);
    label.set_margin_bottom(16);
    label.set_margin_start(16);
    label.set_margin_end(16);
    dialog.content_area().append(&label);
    dialog.add_button(&tr(language, "error.ok"), ResponseType::Close);
    dialog.connect_response(|dialog, _| dialog.close());
    dialog.present();
}
