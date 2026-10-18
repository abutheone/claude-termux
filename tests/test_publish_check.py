"""Regression tests for the local publication guard; no actual credentials used."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

SCRIPT = Path(__file__).resolve().parent.parent / "scripts/check-publish.py"
SPEC = importlib.util.spec_from_file_location("publish_check", SCRIPT)
PUBLISH = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PUBLISH)


class PublicationCheckTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.root = Path(self.directory.name)
        fixtures = self.root / "tests/fixtures"
        fixtures.mkdir(parents=True)
        (fixtures / "sources.json").write_text("{}")
        self.root_patch = patch.object(PUBLISH, "ROOT", self.root)
        self.root_patch.start()
        self.addCleanup(self.root_patch.stop)
        self.addCleanup(self.directory.cleanup)

    def inspect_file(self, name, data):
        relative = Path(name)
        path = self.root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
        failures, _ = PUBLISH.inspect([relative])
        return failures

    def test_secret_findings_never_contain_the_matching_value(self):
        token = b"sk-" + b"ant-" + b"x" * 24
        failures = self.inspect_file("example.txt", token)
        self.assertIn("Anthropic credential", {item["rule"] for item in failures})
        self.assertNotIn(token.decode(), json.dumps(failures))

    def test_staged_local_files_remain_forbidden_even_if_gitignored(self):
        for name in [".env", "target/app", ".tools/app", ".test-install/active.json"]:
            with self.subTest(name=name):
                failures = self.inspect_file(name, b"local state")
                self.assertIn("local state or credential filename", {item["rule"] for item in failures})

    def test_public_key_is_allowed_but_private_key_is_rejected(self):
        self.assertFalse(self.inspect_file("trust/example.pub", b"-----BEGIN PUBLIC KEY-----"))
        private_header = b"-----BEGIN " + b"PRIVATE KEY-----"
        failures = self.inspect_file("example.txt", private_header)
        self.assertIn("private key", {item["rule"] for item in failures})

    def test_runtime_url_exception_does_not_allow_other_credentials(self):
        source = PUBLISH.RUNTIME_URL
        self.assertFalse(self.inspect_file("src/proxy/mod.rs", source))
        source += b"\nhttps://" + b"name:password@example.com"
        failures = self.inspect_file("src/proxy/mod.rs", source)
        self.assertIn("credential URL", {item["rule"] for item in failures})

    def test_documentation_link_must_exist_in_publishable_sources(self):
        failures = self.inspect_file("README.md", b"[missing](ignored.md)")
        self.assertIn("Markdown link missing from source snapshot", {item["rule"] for item in failures})

    def test_changed_fixture_fails_provenance_check(self):
        record = {"sample.txt": {"sha256": "0" * 64}}
        (self.root / "tests/fixtures/sources.json").write_text(json.dumps(record))
        failures = self.inspect_file("tests/fixtures/sample.txt", b"changed fixture")
        self.assertIn("fixture provenance mismatch or missing fixture", {item["rule"] for item in failures})


if __name__ == "__main__":
    unittest.main()
