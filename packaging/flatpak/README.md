# Local Flatpak preparation

See [the packaging guide](../../docs/flatpak.md) for native installation, offline Cargo vendoring, prerequisites, sandbox limitations, and local Flatpak build commands.

`python3 scripts/prepare-flatpak.py` generates `packaging/flatpak/prepared/io.github.zfg.json` and a staged app with `vendor/` and `.cargo/config.toml`. Generated directories here are ignored. The generated manifest uses a **local directory source**, not a publishable repository URL, and is **not Flathub-compatible**.

The provisional ID is `io.github.zfg`. No public owner or repository is assumed.
