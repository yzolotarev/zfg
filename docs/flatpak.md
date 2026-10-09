# Native installation and local Flatpak builds

The app ID `io.github.zfg` is provisional. No public repository/owner is confirmed. These scripts do not create Git repositories, remotes, releases, or privileged installs.

## Native install

Requires Python 3, Rust supporting edition 2024 and the locked dependencies, Cargo, a C toolchain, `pkg-config`, `install`, GTK4 and gtk4-layer-shell development packages. Library discovery is through `pkg-config` (`gtk4`, `gtk4-layer-shell-0`). A Wayland compositor with the layer-shell protocol is needed for the launcher; not every compositor supports it.

```sh
python3 scripts/check-packaging.py
python3 scripts/install-native.py --help
python3 scripts/install-native.py --dry-run
python3 scripts/install-native.py --prefix ~/.local
```

The script builds `cargo build --release --locked` into the checkout's `target/` and installs the executable, desktop entry and AppStream metadata. Existing files at those installation paths are replaced. Add `~/.local/bin` to `PATH`; ensure `~/.local/share` is included in `XDG_DATA_DIRS` for desktop discovery. The included SVG application icon is installed in the hicolor scalable apps directory. `--offline` requires dependencies already cached or vendored. There is no automatic package-manager invocation, uninstall, or desktop database refresh.

`Cargo.lock` fixes Rust dependency resolution, not the Rust compiler, system libraries, or build environment. This is a repeatable locked build procedure, **not a claim of bit-for-bit reproducibility**. Record toolchain and library versions when distributing native builds. The release workflow's tarball is dynamically linked, not a portable static Linux executable.

## Check prerequisites

```sh
python3 scripts/check-packaging.py --flatpak
```

Missing mandatory tools/refs produce exit status 1. The Flatpak check looks for `flatpak-builder`, GNOME Platform and SDK 50, and `org.freedesktop.Sdk.Extension.rust-stable//25.08` on the current architecture. It reports native library availability separately: host GTK libraries cannot satisfy dependencies inside the SDK. Optional validators are reported, not installed. Host Meson is not needed by flatpak-builder's SDK build; it is needed if building layer-shell directly on the host.

At implementation time this host had Cargo 1.98.1, rustc 1.98.1, GTK 4.22.5 and gtk4-layer-shell 1.3.0. `flatpak-builder`, GNOME SDK 50 and the Rust extension were missing; GNOME Platform 50 was installed. Meson and shellcheck were absent. An explicit user-local Flatpak installation of the SDK, Rust extension and official Builder was attempted but timed out after 200 seconds during related runtime setup. The requested SDK/extension/Builder were still absent afterward. Retry requires allowing a longer installation; no privileged install was attempted. GNOME SDK 50's extension metadata was checked against Flathub and specifies branch 25.08.

## Prepare an offline Cargo tree

From the checkout root:

```sh
python3 scripts/prepare-flatpak.py --help
python3 scripts/prepare-flatpak.py --dry-run
python3 scripts/prepare-flatpak.py
```

Preparation copies `Cargo.toml`, `Cargo.lock`, `LICENSE`, `src/`, `data/`, `locales/` and `assets/icons/` into `packaging/flatpak/prepared/app/`. It runs `cargo vendor --locked --versioned-dirs vendor`, saves Cargo's emitted source-replacement TOML to `.cargo/config.toml`, and appends `[net] offline = true`. It validates dependency resolution with `cargo metadata --locked --offline` and checks that the lockfile has not changed. `preparation.json` records the lockfile hash. Vendoring may access Cargo registries; it downloads dependencies, not application data. No build is performed during preparation.

The output must be a **new direct child** of `packaging/flatpak/`; existing output is never overwritten. On failure a partial output can remain. Inspect and remove it manually before retrying. Do not use a staged tree after preparation failed. Regenerate after source or lockfile changes; generated directories are ignored by Git.

## GTK layer-shell source

SDK layer-shell support could not be tested because the SDK was absent. The generated manifest therefore builds the library explicitly before ZFG, rather than assuming that the SDK includes it.

Verified on 2026-10-01 using upstream GitHub release metadata and downloaded archive contents:

- Upstream: `wmww/gtk4-layer-shell`, stable release `v1.3.0`, published 2025-10-29.
- Archive: `https://codeload.github.com/wmww/gtk4-layer-shell/tar.gz/refs/tags/v1.3.0`
- SHA-256: `1ebb01ab14e98afd1727f68f64981c37bd23305b1f131f5667c02b94cf593192`
- Verified Meson project version 1.3.0 and option names. Examples, documentation, tests, smoke tests, introspection and VAPI are disabled. Rust links the C library directly.

The content hash pins the archive even if a tag moves (a changed archive will fail verification). The library requires GTK4, Wayland client, Wayland scanner and wayland-protocols in the SDK. Their actual SDK build has **not** been validated here. No fictitious release checksum or application source URL is used.

## Build locally, after prerequisites are supplied

```sh
flatpak-builder --user --download-only packaging/flatpak/build packaging/flatpak/prepared/io.github.zfg.json
flatpak-builder --user --disable-download --force-clean packaging/flatpak/build packaging/flatpak/prepared/io.github.zfg.json
flatpak-builder --user --install --disable-download --force-clean packaging/flatpak/build packaging/flatpak/prepared/io.github.zfg.json
flatpak run io.github.zfg launcher
```

The download step fetches the pinned layer-shell archive. Supply the SDK/runtime/Rust extension separately before building; these commands do not enable `--install-deps-from`. Cargo builds use `--frozen --offline`, with an isolated `CARGO_HOME`, the vendored config, and no network permission during module builds. Native host libraries and Cargo caches are not Flatpak inputs. `--force-clean` replaces only the specified build directory: do not point it at user data.

The manifest grants Wayland socket and GPU access only: no X11 fallback, home filesystem or network permission. Flatpak task/config files live under the app's sandbox directories, not the native application's paths. No migration is performed. Arbitrary configured host commands are unavailable inside the sandbox. Portal-aware URI launching and explicit GlobalShortcuts support are implemented but not tested inside the package. A compositor without layer-shell uses a normal undecorated GTK fallback, whose positioning differs. These are development constraints, not a claim of fully tested Flatpak integration.

## Validation performed

The actual prepared tree was generated successfully, and all five tests in `scripts/test-packaging.py` passed, including offline Cargo metadata resolution with an empty Cargo home. Run them with Python 3.11 or newer (the tests use `tomllib`):

```sh
ZFG_PREPARED=packaging/flatpak/prepared python3 -B scripts/test-packaging.py
```

`desktop-file-validate` passed on the staged desktop file. `appstreamcli validate --no-net` returned status 3 with `url-homepage-missing`. This is an explicit release blocker until a genuine homepage exists; no URL was fabricated and the original `data/` metadata was not modified. The native install script was subsequently exercised successfully, including desktop metadata and icon installation. The staged source also passed `cargo check --frozen --offline` with an empty Cargo home, from its own vendored working directory, using host GTK libraries. This is not a sandboxed SDK/Flatpak compile or GUI test.

## Release blockers

This is a local development manifest with `type: dir`, **not a Flathub submission**. Confirm ownership/ID, replace local app sources with verifiable immutable release sources, review the included app icon and provide privacy-safe real UI screenshots, validate AppStream/desktop metadata, and test sandbox behavior before submission. The Rust SDK extension is a moving stable toolchain; record its installed commit for strict environment reproduction. A successful vendoring/metadata check does not prove a successful compile, Flatpak build, or GUI run. Packaging CI checks staging and offline dependency resolution only, not these release claims.
