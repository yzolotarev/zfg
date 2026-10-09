# Contributing

1. Open an issue for major behavior changes.
2. Keep changes focused and preserve plain-text task files as canonical storage.
3. Never test against a real personal data directory: use temporary XDG directories.
4. Run `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all`.
5. Do not modify live Ryoku or Hyprland config in automated tests.

Conventional commit prefixes: `feat:`, `fix:`, `test:`, `docs:`, `build:`, `chore:`.
