//! Explicit opt-in XDG Desktop Portal GlobalShortcuts support (interface version >= 1).
//!
//! After registering the application's `launcher` Gio action, call
//! `shortcuts::enable(&app, |status| { /* display Err(message) in your UI */ });`
//! on the GTK main thread (Settings already calls this explicitly).
//! Repeated enables share one pending/active session per application. Pending
//! callers receive the same completion; active callers receive Ok immediately.
//! Call `shortcuts::disable()` to cancel pending setup and release all sessions.
//! The callback reports readiness or failure, not activations.
//!
//! The portal/backend must implement GlobalShortcuts; GTK alone cannot supply it.
//! BindShortcuts may prompt the user, who can reject or change the suggested key.
//! An empty parent_window is intentional (no exported window handle available).
//! The application is held during setup and while enabled. Disable, shutdown,
//! session close, bus loss and portal restart release that hold. Failures are
//! reported once per caller; a subsequent enable can retry. No global key
//! interception or fallback is attempted.

use gtk::{gio, glib, prelude::*};
use std::{
    cell::RefCell,
    collections::HashMap,
    rc::Rc,
    task::{Poll, Waker},
};

const BUS: &str = "org.freedesktop.portal.Desktop";
const ROOT: &str = "/org/freedesktop/portal/desktop";
const IFACE: &str = "org.freedesktop.portal.GlobalShortcuts";
type Options = HashMap<String, glib::Variant>;
type Status = Rc<dyn Fn(Result<(), String>)>;

struct Lifetime {
    connection: gio::DBusConnection,
    session: Option<String>,
    subscriptions: Vec<gio::SignalSubscription>,
    closed_handler: Option<glib::SignalHandlerId>,
    _hold: gio::ApplicationHoldGuard,
    _app: gtk::Application,
}

impl Drop for Lifetime {
    fn drop(&mut self) {
        self.subscriptions.clear();
        if let Some(handler) = self.closed_handler.take() {
            self.connection.disconnect(handler);
        }
        if let Some(path) = self.session.take() {
            self.connection.call(
                Some(BUS),
                &path,
                "org.freedesktop.portal.Session",
                "Close",
                None,
                None,
                gio::DBusCallFlags::NONE,
                5000,
                None::<&gio::Cancellable>,
                |_| {},
            );
        }
    }
}

async fn call(
    connection: &gio::DBusConnection,
    interface: &str,
    method: &str,
    parameters: glib::Variant,
) -> Result<glib::Variant, String> {
    connection
        .call_future(
            Some(BUS),
            ROOT,
            interface,
            method,
            Some(&parameters),
            None,
            gio::DBusCallFlags::NONE,
            30_000,
        )
        .await
        .map_err(|e| format!("{method}: {e}"))
}

/// Probe GlobalShortcuts availability without creating a session or binding keys.
/// Await on the GTK main context. Returns the advertised interface version;
/// unavailable services/interfaces and unsupported versions return an error.
pub async fn detect() -> Result<u32, String> {
    let connection = gio::bus_get_future(gio::BusType::Session)
        .await
        .map_err(|e| format!("Session bus: {e}"))?;
    detect_on(&connection).await
}

async fn detect_on(connection: &gio::DBusConnection) -> Result<u32, String> {
    let reply = call(
        connection,
        "org.freedesktop.DBus.Properties",
        "Get",
        (IFACE, "version").to_variant(),
    )
    .await?;
    let version = reply
        .get::<(glib::Variant,)>()
        .and_then(|(value,)| value.get::<u32>())
        .ok_or("Invalid GlobalShortcuts version property")?;
    if version < 1 {
        return Err("GlobalShortcuts version 1 is required".into());
    }
    Ok(version)
}

#[derive(Default)]
struct Registration {
    generation: u64,
    pending: bool,
    invalidated: bool,
    task: Option<glib::JoinHandle<()>>,
    lifetime: Option<Lifetime>,
    callbacks: Vec<Status>,
}

thread_local! {
    static REGISTRATIONS: RefCell<HashMap<usize, Rc<RefCell<Registration>>>> = RefCell::default();
}

fn stop(registration: &Rc<RefCell<Registration>>, error: &str) {
    // Move resources out before dropping them or invoking arbitrary caller code.
    let (task, lifetime, callbacks) = {
        let mut state = registration.borrow_mut();
        state.generation = state.generation.wrapping_add(1);
        state.pending = false;
        state.invalidated = false;
        (
            state.task.take(),
            state.lifetime.take(),
            std::mem::take(&mut state.callbacks),
        )
    };
    if let Some(task) = task {
        task.abort();
    }
    drop(lifetime);
    for callback in callbacks {
        callback(Err(error.into()));
    }
}

/// Cancel pending setup and close enabled sessions for all applications on this
/// GTK main thread. Idempotent; registered callbacks receive Err("Global shortcuts disabled").
/// The Settings UI currently exposes enable only; this is an opt-in teardown API.
#[allow(dead_code)]
pub fn disable() {
    let registrations =
        REGISTRATIONS.with(|registry| registry.borrow().values().cloned().collect::<Vec<_>>());
    for registration in registrations {
        stop(&registration, "Global shortcuts disabled");
    }
}

/// Enable one portal-managed `launcher` shortcut asynchronously.
/// Errors MUST be surfaced by the caller's status callback. Requires a registered
/// application with the `launcher` action and a running default GLib main context.
pub fn enable(app: &gtk::Application, callback: impl Fn(Result<(), String>) + 'static) {
    let status: Status = Rc::new(callback);
    if !app.is_registered() || !app.has_action("launcher") {
        status(Err(
            "Global shortcuts require a registered application and a launcher action".into(),
        ));
        return;
    }
    let key = app.as_ptr() as usize;
    let registration = REGISTRATIONS.with(|registry| {
        let mut registry = registry.borrow_mut();
        registry
            .entry(key)
            .or_insert_with(|| {
                let registration = Rc::new(RefCell::new(Registration::default()));
                app.connect_shutdown(move |_| {
                    let registration =
                        REGISTRATIONS.with(|registry| registry.borrow_mut().remove(&key));
                    if let Some(registration) = registration {
                        stop(&registration, "Application shutting down");
                    }
                });
                registration
            })
            .clone()
    });
    if registration.borrow().invalidated {
        status(Err(
            "Global shortcuts session is closing; retry enable shortly".into(),
        ));
        return;
    }
    let (active, pending, generation) = {
        let mut state = registration.borrow_mut();
        state.callbacks.push(status.clone());
        let active = state.lifetime.is_some();
        let pending = state.pending;
        if !active && !pending {
            state.pending = true;
        }
        (active, pending, state.generation)
    };
    if active {
        status(Ok(()));
        return;
    }
    if pending {
        return;
    }
    let app = app.clone();
    let hold = app.hold();
    let weak = Rc::downgrade(&registration);
    let lost: Status = Rc::new(move |result| {
        if let Err(error) = result {
            let Some(registration) = weak.upgrade() else {
                return;
            };
            let invalid_generation = {
                let mut state = registration.borrow_mut();
                if state.generation != generation {
                    return;
                }
                state.invalidated = true;
                state.generation = state.generation.wrapping_add(1);
                state.generation
            };
            let weak = weak.clone();
            // Invalidate immediately, but defer destruction until signal delivery ends.
            glib::idle_add_local_once(move || {
                if let Some(registration) = weak.upgrade()
                    && registration.borrow().generation == invalid_generation
                {
                    stop(&registration, &error);
                }
            });
        }
    });
    let weak = Rc::downgrade(&registration);
    let task = glib::MainContext::default().spawn_local(async move {
        let result = setup(&app, hold, lost).await;
        let Some(registration) = weak.upgrade() else {
            return;
        };
        if registration.borrow().generation != generation {
            return;
        }
        registration.borrow_mut().task.take();
        match result {
            Ok(lifetime) => {
                let callbacks = {
                    let mut state = registration.borrow_mut();
                    state.pending = false;
                    state.lifetime = Some(lifetime);
                    state.callbacks.clone()
                };
                for callback in callbacks {
                    if registration.borrow().generation != generation {
                        break;
                    }
                    callback(Ok(()));
                }
            }
            Err(error) => {
                stop(&registration, &error);
            }
        }
    });
    registration.borrow_mut().task = Some(task);
}

async fn setup(
    app: &gtk::Application,
    hold: gio::ApplicationHoldGuard,
    status: Status,
) -> Result<Lifetime, String> {
    let connection = gio::bus_get_future(gio::BusType::Session)
        .await
        .map_err(|e| format!("Session bus: {e}"))?;
    detect_on(&connection).await?;
    let mut lifetime = Lifetime {
        connection: connection.clone(),
        session: None,
        subscriptions: vec![],
        closed_handler: None,
        _hold: hold,
        _app: app.clone(),
    };
    let session_token = token();
    let mut options = Options::new();
    options.insert("session_handle_token".into(), session_token.to_variant());
    let results = request(&connection, "CreateSession", options, |options| {
        (options,).to_variant()
    })
    .await?;
    let session = results
        .get("session_handle")
        .and_then(|v| v.get::<String>())
        .ok_or("CreateSession did not return session_handle")?;
    let object = glib::variant::ObjectPath::try_from(session.as_str())
        .map_err(|_| "Invalid session_handle")?;
    lifetime.session = Some(session.clone());
    let app_weak = app.downgrade();
    let active_session = session.clone();
    lifetime.subscriptions.push(connection.subscribe_to_signal(
        Some(BUS),
        Some(IFACE),
        Some("Activated"),
        Some(ROOT),
        None,
        gio::DBusSignalFlags::NONE,
        move |signal| {
            let parameters = signal.parameters;
            if parameters.n_children() == 4
                && parameters.child_value(0).str() == Some(active_session.as_str())
                && parameters.child_value(1).str() == Some("launcher")
                && let Some(app) = app_weak.upgrade()
            {
                glib::idle_add_local_once(move || {
                    app.activate_action("launcher", None);
                });
            }
        },
    ));
    let closed_status = status.clone();
    lifetime.subscriptions.push(connection.subscribe_to_signal(
        Some(BUS),
        Some("org.freedesktop.portal.Session"),
        Some("Closed"),
        Some(&session),
        None,
        gio::DBusSignalFlags::NONE,
        move |_| {
            closed_status(Err(
                "Global shortcut session was closed; enable again to retry".into(),
            ))
        },
    ));
    let lost_status = status.clone();
    lifetime.subscriptions.push(connection.subscribe_to_signal(
        Some("org.freedesktop.DBus"),
        Some("org.freedesktop.DBus"),
        Some("NameOwnerChanged"),
        Some("/org/freedesktop/DBus"),
        Some(BUS),
        gio::DBusSignalFlags::NONE,
        move |signal| {
            let parameters = signal.parameters;
            if let Some((_, old, new)) = parameters.get::<(String, String, String)>()
                && !old.is_empty()
                && old != new
            {
                lost_status(Err("Desktop portal stopped or restarted".into()));
            }
        },
    ));
    lifetime.closed_handler = Some(connection.connect_local("closed", false, move |_| {
        status(Err("Global shortcuts session bus closed".into()));
        None
    }));
    let mut shortcut = Options::new();
    shortcut.insert("description".into(), "Open ZFG launcher".to_variant());
    shortcut.insert("preferred_trigger".into(), "CTRL+ALT+space".to_variant());
    let shortcuts = vec![("launcher", shortcut)];
    let results = request(&connection, "BindShortcuts", Options::new(), |options| {
        (object, shortcuts, "", options).to_variant()
    })
    .await?;
    let bound = results
        .get("shortcuts")
        .and_then(|v| v.get::<Vec<(String, Options)>>())
        .ok_or("BindShortcuts returned no shortcuts")?;
    if !bound.iter().any(|(id, _)| id == "launcher") {
        return Err("The portal did not bind the launcher shortcut".into());
    }
    Ok(lifetime)
}

fn token() -> String {
    // UUID characters are valid token components after replacing hyphens.
    format!("zfg_{}", glib::uuid_string_random().replace('-', "_"))
}

struct RequestGuard {
    connection: gio::DBusConnection,
    path: String,
    timeout: Rc<RefCell<Option<glib::SourceId>>>,
    completed: bool,
}

impl Drop for RequestGuard {
    fn drop(&mut self) {
        if let Some(timeout) = self.timeout.borrow_mut().take() {
            timeout.remove();
        }
        if !self.completed {
            self.connection.call(
                Some(BUS),
                &self.path,
                "org.freedesktop.portal.Request",
                "Close",
                None,
                None,
                gio::DBusCallFlags::NONE,
                5000,
                None::<&gio::Cancellable>,
                |_| {},
            );
        }
    }
}

async fn request(
    connection: &gio::DBusConnection,
    method: &str,
    mut options: Options,
    parameters: impl FnOnce(Options) -> glib::Variant,
) -> Result<Options, String> {
    let token = token();
    let sender = connection
        .unique_name()
        .ok_or("Bus connection has no unique name")?;
    // This is the portal's documented handle_token path derivation, not a guessed handle.
    let path = format!(
        "/org/freedesktop/portal/desktop/request/{}/{}",
        sender.trim_start_matches(':').replace('.', "_"),
        token
    );
    options.insert("handle_token".into(), token.to_variant());
    let response = Rc::new(RefCell::new((
        None::<Result<Options, String>>,
        None::<Waker>,
    )));
    let signal_response = response.clone();
    let subscription = connection.subscribe_to_signal(
        Some(BUS),
        Some("org.freedesktop.portal.Request"),
        Some("Response"),
        Some(&path),
        None,
        gio::DBusSignalFlags::NONE,
        move |signal| {
            let parameters = signal.parameters;
            let result = match parameters.get::<(u32, Options)>() {
                Some((0, results)) => Ok(results),
                Some((code, _)) => Err(format!("Portal request rejected (response {code})")),
                None => Err("Malformed portal Response".into()),
            };
            let mut state = signal_response.borrow_mut();
            state.0 = Some(result);
            if let Some(waker) = state.1.take() {
                waker.wake();
            }
        },
    );
    // Bound interactive requests too; do not block the main loop while the user decides.
    let timeout_response = response.clone();
    let timeout_slot = Rc::new(RefCell::new(None));
    let fired_slot = timeout_slot.clone();
    let timeout = glib::timeout_add_local_once(std::time::Duration::from_secs(300), move || {
        fired_slot.borrow_mut().take();
        let mut state = timeout_response.borrow_mut();
        if state.0.is_none() {
            state.0 = Some(Err("Portal request timed out".into()));
        }
        if let Some(waker) = state.1.take() {
            waker.wake();
        }
    });
    *timeout_slot.borrow_mut() = Some(timeout);
    let mut guard = RequestGuard {
        connection: connection.clone(),
        path: path.clone(),
        timeout: timeout_slot,
        completed: false,
    };
    let reply = call(connection, IFACE, method, parameters(options)).await;
    let result = match reply {
        Ok(reply)
            if reply.n_children() == 1 && reply.child_value(0).str() == Some(path.as_str()) =>
        {
            std::future::poll_fn(|cx| {
                let mut state = response.borrow_mut();
                if let Some(result) = state.0.take() {
                    Poll::Ready(result)
                } else {
                    state.1 = Some(cx.waker().clone());
                    Poll::Pending
                }
            })
            .await
        }
        Ok(_) => Err(format!("{method} returned an unexpected request handle")),
        Err(error) => Err(error),
    };
    guard.completed = result.is_ok();
    drop(subscription);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn stop_releases_state_before_reentrant_callback_and_is_idempotent() {
        let registration = Rc::new(RefCell::new(Registration {
            pending: true,
            invalidated: true,
            ..Registration::default()
        }));
        let calls = Rc::new(Cell::new(0));
        let weak = Rc::downgrade(&registration);
        let count = calls.clone();
        registration
            .borrow_mut()
            .callbacks
            .push(Rc::new(move |result| {
                assert!(result.is_err());
                count.set(count.get() + 1);
                let registration = weak.upgrade().unwrap();
                assert!(!registration.borrow().pending);
                assert!(!registration.borrow().invalidated);
                stop(&registration, "reentrant disable");
            }));
        stop(&registration, "disabled");
        stop(&registration, "disabled again");
        assert_eq!(calls.get(), 1);
        assert!(registration.borrow().callbacks.is_empty());
    }

    #[test]
    fn stop_notifies_all_pending_callers_and_invalidates_completion() {
        let registration = Rc::new(RefCell::new(Registration {
            pending: true,
            ..Registration::default()
        }));
        let generation = registration.borrow().generation;
        let calls = Rc::new(Cell::new(0));
        for _ in 0..2 {
            let count = calls.clone();
            registration
                .borrow_mut()
                .callbacks
                .push(Rc::new(move |result| {
                    assert_eq!(result, Err("portal lost".into()));
                    count.set(count.get() + 1);
                }));
        }
        stop(&registration, "portal lost");
        assert_eq!(calls.get(), 2);
        assert_ne!(registration.borrow().generation, generation);
    }
}
