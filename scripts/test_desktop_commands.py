"""Exercise real gsettings and desktop-entry parsing with isolated config."""

import ast
import os
from pathlib import Path
import shlex
import subprocess
import tempfile
import unittest


class DesktopCommandTests(unittest.TestCase):
    def test_missing_dconf_bus_does_not_report_success(self):
        for flag in ("--install-shortcuts", "--uninstall-shortcuts"):
            with self.subTest(flag=flag), tempfile.TemporaryDirectory() as temp:
                env = {**os.environ, "GSETTINGS_BACKEND": "dconf",
                       "DBUS_SESSION_BUS_ADDRESS": f"unix:path={temp}/missing-bus",
                       "XDG_CONFIG_HOME": f"{temp}/config", "CLIPFINCH_DATA_DIR": f"{temp}/data"}
                if flag == "--uninstall-shortcuts":
                    # Removal must change an actual binding, not an already-empty list.
                    owned = "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/clipfinch-popup/"
                    subprocess.run(
                        ["dbus-run-session", "--", "gsettings", "set",
                         "org.gnome.settings-daemon.plugins.media-keys", "custom-keybindings", repr([owned])],
                        env=env, check=True, capture_output=True, text=True)
                    installed = subprocess.check_output(
                        ["gsettings", "get", "org.gnome.settings-daemon.plugins.media-keys", "custom-keybindings"],
                        env=env, text=True)
                    self.assertIn(owned, installed)
                result = subprocess.run([str(Path("target/debug/clipfinch").resolve()), flag],
                                        env=env, capture_output=True, text=True)
                self.assertNotEqual(result.returncode, 0, result.stdout)

    def test_launcher_path_roundtrips_through_gnome_and_autostart(self):
        binary = Path("target/debug/clipfinch").resolve()
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            launcher = root / 'folder with spaces' / 'clip\'s $test % "quoted" \\launcher'
            launcher.parent.mkdir()
            marker = root / "invoked"
            launcher.write_text(f'#!/bin/sh\nprintf "%s\\n" "$@" > {shlex.quote(str(marker))}\n')
            launcher.chmod(0o755)
            env = {**os.environ, "CLIPFINCH_LAUNCHER": str(launcher),
                   "XDG_CONFIG_HOME": str(root / "config"),
                   "CLIPFINCH_DATA_DIR": str(root / "data"), "GSETTINGS_BACKEND": "keyfile"}

            def run(*args):
                result = subprocess.run(args, env=env, capture_output=True, text=True)
                details = result.stderr
                if args[0] == "gio":
                    details += Path(args[-1]).read_text()
                self.assertEqual(result.returncode, 0, details)
                return result.stdout.strip()

            media = "org.gnome.settings-daemon.plugins.media-keys"
            unrelated = "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/custom0/"
            run("gsettings", "set", media, "custom-keybindings", f"['{unrelated}']")
            for _ in range(2):
                run(str(binary), "--install-shortcuts")
            paths = ast.literal_eval(run("gsettings", "get", media, "custom-keybindings"))
            self.assertEqual(len(paths), 4)
            self.assertIn(unrelated, paths)
            schema = media + ".custom-keybinding:" + paths[1]
            command = ast.literal_eval(run("gsettings", "get", schema, "command"))
            run(*shlex.split(command))
            self.assertEqual(marker.read_text(), "--toggle\n")

            run(str(binary), "--autostart", "on")
            entry = root / "config/autostart/clipfinch.desktop"
            marker.unlink()
            run("gio", "launch", str(entry))
            # gio may return before its child exits.
            import time
            deadline = time.monotonic() + 3
            while not marker.exists() and time.monotonic() < deadline:
                time.sleep(0.01)
            self.assertEqual(marker.read_text(), "--background\n")
            run(str(binary), "--uninstall-shortcuts")
            self.assertEqual(ast.literal_eval(run("gsettings", "get", media, "custom-keybindings")), [unrelated])


if __name__ == "__main__":
    unittest.main()
