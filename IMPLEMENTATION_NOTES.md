# Implementation notes (2026-10-01)

## Implemented
- Canonical UTF-8 `focus.txt` / `later.txt`, ordered duplicate-friendly tasks, default Focus cap 5.
- Cross-process advisory lock, reread before mutation, fsync/atomic file replacement, recovery journal for transfers, full-snapshot validation for destructive UI actions.
- CLI task/status operations and single-instance GTK application. CLI, Ryoku and portal activation share the `launcher` GApplication action.
- Transparent semicircular launcher, nested Task branch, symbolic icons, geometric wedge activation, keyboard focus and selection outlines.
- Input appears only after Focus/Later selection; internal GtkText typing is not intercepted. Escape walks input → child → parent → close.
- Short opening/closing fades and child crossfade honor application animation preference and GTK's reduced-motion setting. Hover uses 100 ms cubic-out brightness/outline interpolation and at most 3 px radial expansion; rapid reversals cancel the previous tick and there is no idle tick. Opening scale animation remains optional visual polish.
- Outside-click dismissal on supported Wayland layer-shell uses a transparent modal full-monitor surface. The dismissal click is consumed, not forwarded to the underlying app; no global hook exists.
- Focus Full chooser preserves displaced tasks; dialogs use their own focusable overlay surfaces to avoid being blocked by the launcher's keyboard grab.
- Focus/Later managers watch task-directory changes and support add/edit/move/complete/confirmed delete, row navigation, and Ctrl+Up/Down Focus reorder.
- Settings include EN/RU, capacity, theme/animations/close behavior, panel preferences, desktop-ID/URI/custom-argv actions. Saving a language change rebuilds Settings and affects newly opened views.
- UI locale catalogs cover manager, launcher, chooser, settings and error-dialog labels. Raw technical error details and third-party application names may remain English.
- ExternalLauncher native/sandbox policy, safe URI-template allow-list and encoding, desktop-ID launch through GIO, literal argv execution with no inserted shell. Flatpak disables unsupported host actions.
- Explicit portal GlobalShortcuts enable/disable; asynchronous capability detection, one registration per application, request cancellation and session/hold cleanup. Activation invokes the same launcher action.
- Ryoku widget after clock: focus-ring icon, first task with width-based ellipsis, optional N/limit, subdued optional Later count, and +. Preferences/localized labels come from status JSON; polling interval is two seconds. Existing todo-file plugin preserved.
- Native user-local installation script installs binary, desktop metadata and SVG icon. Offline-source Flatpak preparation vendors locked dependencies and generates a local development manifest pinned to gtk4-layer-shell 1.3.0.

## Validation actually performed
- Strict fmt/clippy and Rust tests passed after the latest feature integration; rerun after any subsequent source changes. Added template/sandbox-policy, wedge-hit-test, shortcut-lifecycle, and localized panel-status tests.
- Wayland screenshots verified parent/child menu, plain input containing f/l, and no rectangular launcher background. Screenshots containing unrelated desktop content are temporary and are not release artwork.
- Outside click via a test pointer closed the launcher and its process.
- Isolated temporary XDG directories: RU Focus Full chooser opened; sending incoming Six to Later preserved the five original Focus tasks. A separate numbered-key replacement smoke test put Six in slot 1 and moved One to Later. Real personal task files were not used for these tests.
- Portal backend reports GlobalShortcuts version 1. Permission approval and shortcut delivery have not been manually tested.
- All five packaging tests passed with an actual staged tree; staged code passed frozen/offline Cargo check with an empty Cargo home and host GTK libraries.
- Native install script succeeded. Ryoku static plugin validation previously returned 0 blocking findings/0 warnings; run again after plugin edits.
- Desktop entry validation passed. AppStream reports missing homepage; no public URL was fabricated.

## Remaining release gates / limits
- Public name/owner, app ID (currently provisional io.github.zfg), homepage, methodology attribution and copyright review. No Git remote, commits, tag or release has been created.
- Native pointer-relative placement is not available through the portable GTK Wayland API used here. Placement uses a previous app window's monitor or first GDK monitor and a top-center anchor. Multi-monitor/high-DPI and non-Hyprland fallback need manual verification.
- Remaining visual refinement needs daily-use feedback; GTK4 is used directly, not a libadwaita Rust wrapper.
- Sandbox build/run, portal shortcut approval/delivery, sandbox URI routing and live theme/language switching need remaining release smoke tests.
- GNOME SDK 50 uses Freedesktop SDK extension branch 25.08 (verified upstream metadata). SDK, Rust extension and official Flatpak Builder installation timed out after 200 seconds during related runtime setup; requested tools remained uninstalled. Some related user-local runtime components were installed/updated; they were not rolled back.
- Local Flatpak manifest uses directory sources, not immutable release sources, and is not a Flathub submission. A native offline Cargo check is not a sandbox build.
- No GitHub or Flathub publication is claimed. See docs/release-checklist.md and docs/flatpak.md.
