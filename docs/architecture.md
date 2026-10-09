# Architecture

```text
GTK UI / CLI / Ryoku QS plugin
            │ zfg commands + status --json
            ▼
       Rust application
            │ advisory lock + recovery journal
            ▼
  XDG data: focus.txt / later.txt
```

The task text files remain canonical. Core mutations reread files under an advisory process lock and replace each list atomically. A small journal supports recovery of operations that update both files. UI mutations use a complete ordered snapshot to reject stale list views, including duplicate task names.

GTK windows use a single-instance `GtkApplication`. CLI, panel, and portal-shortcut activation share its `launcher` action. Settings are reread on activation. Focus/Later managers monitor directory changes and verify snapshots before destructive mutations.

On supported Wayland compositors, gtk4-layer-shell provides a transparent modal launcher surface. Both semicircular pies use geometric wedge hit testing, accessible buttons, hover/focus/selection states, and short reduced-motion-aware fades. A transparent full-monitor catcher consumes an outside dismissal click, as a modal popup does; it does not install a global input hook. Escape walks input → child → parent → close. Dialogs are separate focusable overlay surfaces so a chooser cannot be trapped under the launcher's keyboard grab.

Placement uses an available application surface's monitor or the first GDK monitor, at a top-center anchor. A portable global pointer position is not available through this Wayland API; pointer anchoring and broad multi-monitor/compositor verification remain follow-up requirements.

The Ryoku widget is a separate user plugin. It consumes the status CLI and launches application actions through argv arrays; it never parses or writes the task files itself.
