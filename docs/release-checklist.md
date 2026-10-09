# Release checklist

## Implemented and checked locally
- [x] Plain-text storage, Focus cap/replacement, duplicate/snapshot/concurrent writer semantics
- [x] Semicircular parent/child pie, geometric hit targets, typing and keyboard/back navigation
- [x] Opening/closing fade, child crossfade and 100 ms hover interpolation gated by animation/reduced-motion settings
- [x] Transparent Wayland launcher and outside-click dismissal (dismissal click is consumed)
- [x] EN/RU manager, launcher, chooser, settings and dialog labels; matching catalog tests
- [x] URI allow-list/template encoding, native/Flatpak policy and literal argv tests
- [x] Manager directory monitoring, snapshot-safe mutations, Focus reorder and keyboard task actions
- [x] Isolated RU Focus Full send-to-Later and numbered-key replacement smoke tests preserve every task
- [x] Portal capability probe, explicit enable/disable and session lifecycle implementation/tests
- [x] Ryoku widget installation after clock, counter/width preferences and localized status contract
- [x] Native binary, desktop entry and SVG icon installation; desktop-file validation
- [x] Actual Cargo vendoring, all packaging tests and empty-Cargo-home offline staged-source check
- [x] Rust formatting, strict clippy and automated tests

## Required before public stable release
- [ ] Resolve product name, app ID, owner/homepage, copyright and methodology attribution
- [ ] Approve a portal shortcut and verify delivery, disable, restart/cancellation on the actual backend
- [ ] Check native app/URI actions on a clean install and sandbox URI routing
- [ ] Manually verify live language/theme changes, high DPI, multi-monitor placement and compositor fallback
- [ ] Collect daily-use visual feedback and optional scale-animation refinement
- [ ] Capture privacy-safe real screenshots/demo; concept art remains explicitly labelled
- [ ] Finish SDK/Builder installation (last attempt timed out), then build/install/run the sandboxed Flatpak
- [ ] Replace local Flatpak sources with immutable public release sources and validate sandbox permissions
- [ ] AppStream passes without missing homepage warning
- [ ] Create/verify real GitHub remote, CI run, release artifacts and tag after identity is confirmed
- [ ] Review privacy/security/attribution and submit actual Flathub package
