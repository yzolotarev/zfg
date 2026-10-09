"""Local packaging helpers; no privileged installation or remote publishing."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shlex
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
APP_ID = "io.github.zfg"
SDK_VERSION = "50"
RUST_EXTENSION_VERSION = "25.08"
LAYER_URL = "https://codeload.github.com/wmww/gtk4-layer-shell/tar.gz/refs/tags/v1.3.0"
LAYER_SHA256 = "1ebb01ab14e98afd1727f68f64981c37bd23305b1f131f5667c02b94cf593192"


def run(args, dry=False, cwd=ROOT, **kwargs):
    print("+ " + shlex.join(map(str, args)), flush=True)
    if dry:
        return None
    return subprocess.run(args, cwd=cwd, check=True, **kwargs)


def check(flatpak=False):
    missing = []
    tools = ["cargo", "rustc"]
    tools += ["flatpak", "flatpak-builder"] if flatpak else ["cc", "pkg-config", "install"]
    for tool in tools:
        path = shutil.which(tool)
        print(f"{tool}: {path or 'MISSING'}")
        if not path:
            missing.append(tool)
    if shutil.which("pkg-config"):
        for dependency in ["gtk4 >= 4.0", "gtk4-layer-shell-0 >= 1.0"]:
            ok = subprocess.run(["pkg-config", "--exists", dependency]).returncode == 0
            print(f"native {dependency}: {'OK' if ok else 'MISSING'}")
            if not ok and not flatpak:
                missing.append(dependency)
    if flatpak and shutil.which("flatpak"):
        for ref in [f"org.gnome.Sdk//{SDK_VERSION}", f"org.gnome.Platform//{SDK_VERSION}",
                    f"org.freedesktop.Sdk.Extension.rust-stable//{RUST_EXTENSION_VERSION}"]:
            ok = subprocess.run(["flatpak", "info", ref], stdout=subprocess.DEVNULL,
                                stderr=subprocess.DEVNULL).returncode == 0
            print(f"{ref}: {'OK' if ok else 'MISSING'}")
            if not ok:
                missing.append(ref)
    print("Optional validators: " + ", ".join(
        f"{t}={'available' if shutil.which(t) else 'missing'}"
        for t in ["shellcheck", "desktop-file-validate", "appstreamcli"]))
    if missing:
        print("Missing requirements: " + ", ".join(missing), file=sys.stderr)
    return 1 if missing else 0


def native(args):
    prefix = Path(args.prefix).expanduser().resolve()
    if prefix == ROOT or ROOT in prefix.parents:
        raise ValueError("Install prefix must be outside the checkout")
    if not args.dry_run and hasattr(os, "geteuid") and os.geteuid() == 0:
        raise ValueError("Run as an ordinary user; this script does not support privileged installs")
    if not args.dry_run and check():
        return 1
    command = ["cargo", "build", "--release", "--locked", "--target-dir", str(ROOT / "target")]
    if args.offline:
        command.append("--offline")
    run(command, args.dry_run)
    for source, destination, mode in [
        (ROOT / "target/release/zfg", prefix / "bin/zfg", "755"),
        (ROOT / "data/io.github.zfg.desktop", prefix / "share/applications/io.github.zfg.desktop", "644"),
        (ROOT / "data/io.github.zfg.metainfo.xml", prefix / "share/metainfo/io.github.zfg.metainfo.xml", "644"),
        (ROOT / "assets/icons/app.svg", prefix / "share/icons/hicolor/scalable/apps/io.github.zfg.svg", "644"),
    ]:
        run(["install", "-D", "-m", mode, str(source), str(destination)], args.dry_run)
    print(f"Add {prefix / 'bin'} to PATH. Desktop discovery requires the prefix's share directory in XDG_DATA_DIRS.")
    print("Installed desktop metadata and the application icon. Public identity is still provisional.")
    return 0


def prepare(args):
    output = Path(args.output).expanduser().resolve()
    # Do not recursively copy an output located inside the input tree.
    if output == ROOT or ROOT not in output.parents or output.parent != ROOT / "packaging/flatpak":
        raise ValueError("Output must be a direct child of packaging/flatpak in this checkout")
    if output.exists():
        raise ValueError(f"Refusing to overwrite existing output: {output}")
    if args.dry_run:
        print(f"Would stage Cargo.toml, Cargo.lock, src/, data/, locales/, assets/icons/, LICENSE into {output / 'app'}")
        run(["cargo", "vendor", "--locked", "--versioned-dirs", "vendor"], True, output / "app")
        print("Would save Cargo's source replacement config plus [net] offline = true")
        print(f"Would generate {output / (APP_ID + '.json')} with pinned gtk4-layer-shell 1.3.0")
        print("Preparation can access Cargo registries; flatpak-builder must download the pinned layer-shell archive before its offline build.")
        return 0
    app = output / "app"
    app.mkdir(parents=True)
    for name in ["Cargo.toml", "Cargo.lock", "LICENSE"]:
        shutil.copy2(ROOT / name, app / name)
    for name in ["src", "data", "locales"]:
        shutil.copytree(ROOT / name, app / name)
    (app / "assets").mkdir()
    shutil.copytree(ROOT / "assets/icons", app / "assets/icons")
    original_lock = (app / "Cargo.lock").read_bytes()
    result = run(["cargo", "vendor", "--locked", "--versioned-dirs", "vendor"],
                 cwd=app, stdout=subprocess.PIPE, text=True)
    (app / ".cargo").mkdir()
    (app / ".cargo/config.toml").write_text(result.stdout + '\n[net]\noffline = true\n')
    run(["cargo", "metadata", "--locked", "--offline", "--format-version", "1"],
        cwd=app, stdout=subprocess.DEVNULL)
    if (app / "Cargo.lock").read_bytes() != original_lock:
        raise ValueError("Cargo.lock changed during preparation")
    manifest = {
        "app-id": APP_ID, "runtime": "org.gnome.Platform", "runtime-version": SDK_VERSION,
        "sdk": "org.gnome.Sdk", "sdk-extensions": ["org.freedesktop.Sdk.Extension.rust-stable"],
        "command": "zfg", "build-options": {"append-path": "/usr/lib/sdk/rust-stable/bin",
            "env": {"CARGO_HOME": "/run/build/zfg/cargo-home", "CARGO_NET_OFFLINE": "true"}},
        "finish-args": ["--socket=wayland", "--device=dri"],
        "modules": [
            {"name": "gtk4-layer-shell", "buildsystem": "meson",
             "config-opts": ["-Dexamples=false", "-Ddocs=false", "-Dtests=false",
                             "-Dsmoke-tests=false", "-Dintrospection=false", "-Dvapi=false"],
             "sources": [{"type": "archive", "url": LAYER_URL, "sha256": LAYER_SHA256}]},
            {"name": "zfg", "buildsystem": "simple", "build-commands": [
                "cargo build --release --frozen --offline",
                "install -Dm755 target/release/zfg /app/bin/zfg",
                "install -Dm644 data/io.github.zfg.desktop /app/share/applications/io.github.zfg.desktop",
                "install -Dm644 data/io.github.zfg.metainfo.xml /app/share/metainfo/io.github.zfg.metainfo.xml",
                "install -Dm644 assets/icons/app.svg /app/share/icons/hicolor/scalable/apps/io.github.zfg.svg"],
             "sources": [{"type": "dir", "path": "app"}]}
        ]}
    path = output / f"{APP_ID}.json"
    path.write_text(json.dumps(manifest, indent=2) + "\n")
    (output / "preparation.json").write_text(json.dumps({
        "cargo_lock_sha256": hashlib.sha256(original_lock).hexdigest(),
        "layer_shell_sha256": LAYER_SHA256,
        "local_only": True,
    }, indent=2) + "\n")
    print(f"Prepared {path}. Local development manifest only; not a Flathub submission.")
    return 0


def main(mode):
    parser = argparse.ArgumentParser(description={
        "check": "Report missing native or Flatpak packaging prerequisites (no installation).",
        "native": "Build from Cargo.lock and install to a user-owned prefix without sudo.",
        "prepare": "Stage local application sources, vendor locked Cargo dependencies, and generate an offline-build Flatpak manifest.",
    }[mode])
    if mode == "check":
        parser.add_argument("--flatpak", action="store_true", help="also check SDK, runtime, Rust extension and builder")
    else:
        parser.add_argument("--dry-run", action="store_true", help="print plan without writes, builds or network access")
        if mode == "native":
            parser.add_argument("--prefix", default="~/.local", help="user-owned install prefix (default: ~/.local)")
            parser.add_argument("--offline", action="store_true", help="require cached or vendored dependencies")
        else:
            parser.add_argument("--output", default=str(ROOT / "packaging/flatpak/prepared"),
                                help="new direct child of packaging/flatpak; existing paths are never overwritten")
    args = parser.parse_args()
    try:
        return check(args.flatpak) if mode == "check" else native(args) if mode == "native" else prepare(args)
    except (ValueError, OSError, subprocess.CalledProcessError) as exc:
        print(f"Packaging failed: {exc}", file=sys.stderr)
        if mode == "prepare":
            print("A partial output may remain; inspect it and remove it explicitly before retrying.", file=sys.stderr)
        return 1
