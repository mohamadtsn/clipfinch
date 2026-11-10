"""Run in the development image: python3 scripts/test_container.py."""

import os
from pathlib import Path
import shutil
import socket
import subprocess
import tempfile
import unittest


class ContainerLauncherTests(unittest.TestCase):
    def test_forwards_arguments_from_another_directory_and_preserves_failure(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "project with spaces"
            (root / "scripts").mkdir(parents=True)
            shutil.copy(Path(__file__).with_name("container.sh"), root / "scripts/container.sh")
            (root / "compose.yaml").write_text("services: {}\n")
            docker = Path(temp) / "docker"
            capture = Path(temp) / "args"
            docker.write_text('#!/bin/bash\nif [[ "$1" == image ]]; then exit 0; fi\n'
                              'printf "%s\\n" "$@" > "$CAPTURE"\nexit "${FAKE_EXIT:-0}"\n')
            docker.chmod(0o755)
            env = {**os.environ, "PATH": f"{temp}:{os.environ['PATH']}",
                   "CAPTURE": str(capture), "FAKE_EXIT": "37"}
            runtime = Path(temp) / "runtime"
            runtime.mkdir()
            bus = socket.socket(socket.AF_UNIX)
            self.addCleanup(bus.close)
            bus.bind(str(runtime / "bus"))
            env["XDG_RUNTIME_DIR"] = str(runtime)
            for flag in ("--version", "--queue-paste"):
                with self.subTest(flag=flag):
                    result = subprocess.run(
                        ["bash", str(root / "scripts/container.sh"), flag], cwd=temp,
                        env={**env, "DISPLAY": ":999"}, capture_output=True, text=True)
                    self.assertEqual(result.returncode, 37, result.stderr)
                    args = capture.read_text().splitlines()
                    self.assertIn(str(root / "compose.yaml"), args)
                    self.assertEqual(args[-2:], ["desktop", flag])
                    self.assertIn(f"CLIPFINCH_LAUNCHER={root}/scripts/container.sh", args)

            host_config = Path(temp) / "host-config"
            (host_config / "dconf").mkdir(parents=True)
            (runtime / "dconf").mkdir()
            authority = Path(temp) / ".Xauthority"
            authority.write_text("synthetic authentication fixture")
            env.pop("XAUTHORITY", None)
            result = subprocess.run(
                ["bash", str(root / "scripts/container.sh"), "--install-shortcuts"],
                env={**env, "XDG_CONFIG_HOME": str(host_config), "HOME": temp},
                capture_output=True, text=True)
            self.assertEqual(result.returncode, 37, result.stderr)
            args = capture.read_text().splitlines()
            self.assertIn(f"{host_config}/dconf:/state/config/dconf:ro", args)
            self.assertIn(f"{runtime}/dconf:{runtime}/dconf", args)
            self.assertIn(f"{authority}:{authority}:ro", args)
            self.assertIn(f"XAUTHORITY={authority}", args)

            # A GNOME invocation must retain explicit session access without inherited environment.
            result = subprocess.run(
                ["bash", str(root / "scripts/container.sh"), "--version"],
                env={**env, "CLIPFINCH_SESSION_ACCESS": "1"}, capture_output=True, text=True)
            self.assertEqual(result.returncode, 37, result.stderr)
            self.assertIn(f"CLIPFINCH_LAUNCHER={root}/scripts/container-session.sh",
                          capture.read_text().splitlines())
            shutil.copy(Path(__file__).with_name("container-session.sh"), root / "scripts/container-session.sh")
            env.pop("CLIPFINCH_SESSION_ACCESS", None)
            result = subprocess.run(["bash", str(root / "scripts/container-session.sh"), "--version"],
                                    env=env, capture_output=True, text=True)
            self.assertEqual(result.returncode, 37, result.stderr)
            self.assertIn(str(root / "compose.session.yaml"), capture.read_text().splitlines())

    def test_refuses_gui_without_a_display(self):
        env = {k: v for k, v in os.environ.items() if k not in ("DISPLAY", "WAYLAND_DISPLAY")}
        result = subprocess.run(["bash", str(Path(__file__).with_name("container.sh")), "--toggle"],
                                env=env, capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("desktop session", result.stderr)

    def test_gui_refuses_missing_host_bus(self):
        with tempfile.TemporaryDirectory() as temp:
            docker = Path(temp) / "docker"
            docker.write_text("#!/bin/sh\nexit 0\n")
            docker.chmod(0o755)
            result = subprocess.run(
                ["bash", str(Path(__file__).with_name("container.sh")), "--toggle"],
                env={**os.environ, "PATH": f"{temp}:{os.environ['PATH']}", "DISPLAY": ":999",
                     "XDG_RUNTIME_DIR": temp, "CLIPFINCH_STATE_DIR": f"{temp}/state",
                     "CLIPFINCH_AUTOSTART_DIR": f"{temp}/autostart"},
                capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("session bus", result.stderr)

    def test_entrypoint_refuses_unreachable_bus_before_starting_gtk(self):
        result = subprocess.run(
            ["bash", str(Path(__file__).with_name("container-entrypoint.sh")), "--toggle"],
            env={**os.environ, "DBUS_SESSION_BUS_ADDRESS": "unix:path=/missing-clipfinch-test-bus"},
            capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("session bus", result.stderr)


if __name__ == "__main__":
    unittest.main()
