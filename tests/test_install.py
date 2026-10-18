"""Exercise the real bootstrap script without network, packages, or Claude login."""
import os
from pathlib import Path
import re
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parent.parent
INSTALLER = ROOT / "install.sh"
REVISION = re.search(r"fallback_revision=([0-9a-f]{40})", INSTALLER.read_text())[1]


class InstallerTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.home = self.root / "home with spaces"
        self.home.mkdir()
        self.bin = self.root / "tools"
        self.bin.mkdir()
        self.log = self.root / "calls"
        self.env = dict(os.environ, HOME=str(self.home),
                        PREFIX="/data/data/com.termux/files/usr",
                        PATH=f"{self.bin}:{os.environ['PATH']}",
                        TEST_LOG=str(self.log), TEST_REVISION=REVISION,
                        TEST_ARCH="aarch64", TEST_BUILD_FAIL="0", TEST_SETUP_FAIL="0",
                        TEST_HTTP_CODE="404", TEST_TAG="v0.1.0",
                        TEST_RELEASE_REVISION="1" * 40)
        self.tool("uname", 'case "$1" in -o) echo Android ;; -m) echo "$TEST_ARCH" ;; esac')
        self.tool("pkg", 'echo pkg >> "$TEST_LOG"')
        self.tool("curl", '''
            echo curl >> "$TEST_LOG"
            printf '{\"tag_name\":\"%s\"}\n' "$TEST_TAG" > "$3"
            printf '%s' "$TEST_HTTP_CODE"
        ''')
        self.tool("jq", '''
            echo jq >> "$TEST_LOG"
            printf '%s\n' "$TEST_TAG"
        ''')
        self.tool("git", '''
            echo "git $*" >> "$TEST_LOG"
            if [ "$1" = init ]; then mkdir -p "$3"; fi
            if [ "${3:-}" = rev-parse ]; then
                if [ "$TEST_HTTP_CODE" = 200 ]; then echo "$TEST_RELEASE_REVISION"; else echo "$TEST_REVISION"; fi
            fi
        ''')
        self.tool("cargo", '''
            echo cargo >> "$TEST_LOG"
            [ "$TEST_BUILD_FAIL" = 0 ] || exit 1
            mkdir -p target/release
            cat > target/release/claude-termux <<'LAUNCHER'
#!/bin/sh
printf '%s\\n' "$1" >> "$TEST_LOG"
if [ "$1" = setup ] && [ "$TEST_SETUP_FAIL" != 0 ]; then exit 1; fi
LAUNCHER
            chmod 700 target/release/claude-termux
        ''')

    def tool(self, name, body):
        path = self.bin / name
        path.write_text("#!/bin/sh\nset -eu\n" + body + "\n")
        path.chmod(0o700)

    @property
    def destination(self):
        return self.home / ".local/bin/claude-termux"

    def run_installer(self):
        return subprocess.run(["sh", str(INSTALLER)], env=self.env,
                              text=True, capture_output=True, timeout=20)

    def assert_clean(self):
        state = self.home / ".local/share/claude-termux-launcher"
        self.assertFalse((state / "install.lock").exists())
        self.assertEqual(list(state.glob("build.*")), [])

    def test_success_and_rerun_preserve_user_files(self):
        credentials = self.home / ".config/claude-termux/claude/credentials"
        credentials.parent.mkdir(parents=True)
        credentials.write_text("fixture, not a token")
        shell = self.home / ".bashrc"
        shell.write_text("# personal settings\n")
        for _ in range(2):
            result = self.run_installer()
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertTrue(self.destination.is_symlink())
            self.assertTrue(os.access(self.destination, os.X_OK))
            self.assertTrue("already installed" in result.stdout or str(self.destination) in result.stdout)
            self.assert_clean()
        self.assertEqual(credentials.read_text(), "fixture, not a token")
        self.assertEqual(shell.read_text(), "# personal settings\n")
        self.assertEqual(self.log.read_text().splitlines().count("setup"), 2)

    def test_failed_build_or_setup_preserves_installed_launcher(self):
        self.assertEqual(self.run_installer().returncode, 0)
        previous = self.destination.readlink()
        self.env["TEST_HTTP_CODE"] = "200"
        for failure in ("TEST_BUILD_FAIL", "TEST_SETUP_FAIL"):
            self.env[failure] = "1"
            self.assertNotEqual(self.run_installer().returncode, 0)
            self.assertEqual(self.destination.readlink(), previous)
            self.assert_clean()
            self.env[failure] = "0"

    def test_existing_non_executable_file_is_not_overwritten(self):
        self.destination.parent.mkdir(parents=True)
        self.destination.write_text("personal file")
        self.assertNotEqual(self.run_installer().returncode, 0)
        self.assertEqual(self.destination.read_text(), "personal file")
        self.assertFalse(self.log.exists())

    def test_legacy_launcher_is_backed_up_before_replacement(self):
        self.destination.parent.mkdir(parents=True)
        self.destination.write_text("#!/bin/sh\necho previous launcher\n")
        self.destination.chmod(0o700)
        self.env["TEST_HTTP_CODE"] = "200"
        result = self.run_installer()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(self.destination.is_symlink())
        backups = list((self.home / ".local/share/claude-termux-launcher/backups").glob("claude-termux.*/claude-termux"))
        self.assertEqual(len(backups), 1)
        self.assertEqual(backups[0].read_text(), "#!/bin/sh\necho previous launcher\n")
        self.assertIn("Previous launcher backup:", result.stdout)

    def test_failed_setup_keeps_legacy_launcher_and_removes_temporary_backup(self):
        self.destination.parent.mkdir(parents=True)
        original = "#!/bin/sh\necho old\n"
        self.destination.write_text(original)
        self.destination.chmod(0o700)
        self.env["TEST_HTTP_CODE"] = "200"
        self.env["TEST_SETUP_FAIL"] = "1"
        result = self.run_installer()
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(self.destination.read_text(), original)
        state = self.home / ".local/share/claude-termux-launcher"
        self.assertEqual(list((state / "backups").iterdir()), [])
        self.assert_clean()

    def test_release_discovery_uses_latest_release_tag(self):
        self.env["TEST_HTTP_CODE"] = "200"
        result = self.run_installer()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("refs/tags/v0.1.0", self.log.read_text())

    def test_unrelated_symlink_is_not_overwritten(self):
        self.destination.parent.mkdir(parents=True)
        self.destination.symlink_to("/unrelated/command")
        self.assertNotEqual(self.run_installer().returncode, 0)
        self.assertEqual(str(self.destination.readlink()), "/unrelated/command")
        self.assertFalse(self.log.exists())

    def test_unsupported_phone_has_no_side_effects(self):
        self.env["TEST_ARCH"] = "x86_64"
        self.assertNotEqual(self.run_installer().returncode, 0)
        self.assertFalse((self.home / ".local").exists())
        self.assertFalse(self.log.exists())

    def test_source_revision_mismatch_stops_before_build(self):
        self.env["TEST_REVISION"] = "0" * 40
        self.assertNotEqual(self.run_installer().returncode, 0)
        self.assertNotIn("cargo", self.log.read_text().splitlines())
        self.assertFalse(self.destination.exists())
        self.assert_clean()

    def test_package_failure_stops_before_source_download(self):
        self.tool("pkg", 'echo pkg >> "$TEST_LOG"; exit 1')
        self.assertNotEqual(self.run_installer().returncode, 0)
        self.assertEqual(self.log.read_text().splitlines(), ["pkg"])
        self.assertFalse(self.destination.exists())
        self.assert_clean()

    def test_existing_lock_is_preserved(self):
        lock = self.home / ".local/share/claude-termux-launcher/install.lock"
        lock.mkdir(parents=True)
        self.assertNotEqual(self.run_installer().returncode, 0)
        self.assertTrue(lock.is_dir())
        self.assertFalse(self.log.exists())

    def test_symlinked_directory_is_rejected(self):
        other = self.root / "other"
        other.mkdir()
        (self.home / ".local").symlink_to(other)
        self.assertNotEqual(self.run_installer().returncode, 0)
        self.assertEqual(list(other.iterdir()), [])
        self.assertFalse(self.log.exists())


if __name__ == "__main__":
    unittest.main()
