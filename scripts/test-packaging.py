#!/usr/bin/env python3
"""Non-destructive CLI tests, plus optional verification of a prepared tree."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tomllib
import unittest

ROOT = Path(__file__).resolve().parents[1]


class PackagingTests(unittest.TestCase):
    def invoke(self, script, *args):
        return subprocess.run([sys.executable, "-B", str(ROOT / "scripts" / script), *args],
                              cwd=ROOT, text=True, capture_output=True,
                              env={**os.environ, "PYTHONDONTWRITEBYTECODE": "1"})

    def test_help(self):
        for script in ["install-native.py", "prepare-flatpak.py", "check-packaging.py"]:
            result = self.invoke(script, "--help")
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn("usage:", result.stdout)

    def test_native_dry_run(self):
        result = self.invoke("install-native.py", "--dry-run", "--offline")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("--locked", result.stdout)
        self.assertIn("--offline", result.stdout)
        self.assertIn("install -D", result.stdout)

    def test_prepare_dry_run(self):
        destination = ROOT / "packaging/flatpak/test-dry-run-output"
        self.assertFalse(destination.exists())
        result = self.invoke("prepare-flatpak.py", "--dry-run", "--output", str(destination))
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("cargo vendor --locked", result.stdout)
        self.assertFalse(destination.exists())

    def test_reject_unsafe_output(self):
        result = self.invoke("prepare-flatpak.py", "--dry-run", "--output", str(ROOT))
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("direct child", result.stderr)

    def test_prepared_tree_if_requested(self):
        path = os.environ.get("ZFG_PREPARED")
        if not path:
            self.skipTest("Set ZFG_PREPARED to validate an actual staged tree")
        output = Path(path).resolve()
        manifest = json.loads((output / "io.github.zfg.json").read_text())
        app = output / "app"
        config = tomllib.loads((app / ".cargo/config.toml").read_text())
        self.assertTrue(config["net"]["offline"])
        self.assertTrue((app / "vendor").is_dir())
        for relative in ["locales/en.json", "locales/ru.json", "assets/icons/app.svg", "src/main.rs"]:
            self.assertEqual((app / relative).read_bytes(), (ROOT / relative).read_bytes())
        self.assertEqual((app / "Cargo.lock").read_bytes(), (ROOT / "Cargo.lock").read_bytes())
        self.assertEqual(manifest["modules"][-1]["sources"], [{"type": "dir", "path": "app"}])
        self.assertIn("--offline", manifest["modules"][-1]["build-commands"][0])
        self.assertEqual(len(manifest["modules"][0]["sources"][0]["sha256"]), 64)
        subprocess.run(["cargo", "metadata", "--locked", "--offline", "--format-version", "1"],
                       cwd=app, stdout=subprocess.DEVNULL, check=True,
                       env={**os.environ, "CARGO_HOME": str(output / "test-empty-cargo-home")})


if __name__ == "__main__":
    unittest.main()
