#!/usr/bin/env python3
"""Inspect the exact Git candidate files; optionally export a checked source snapshot.

This is a local heuristic check, not a complete credential or security audit.
Reports name matching rules and file paths, never the suspected secret values.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parent.parent
FORBIDDEN_ROOTS = {".git", ".tools", "target", "test-results", ".claude"}
FORBIDDEN_NAMES = {".npmrc", ".credentials.json", ".claude.json", "id_rsa", "id_ed25519"}
PATTERNS = {
    "private key": rb"-----BEGIN (?:RSA |EC |OPENSSH |DSA |ENCRYPTED )?PRIVATE KEY-----",
    "Anthropic credential": rb"sk-ant-[A-Za-z0-9_-]{20,}",
    "GitHub credential": rb"(?:gh[pousr]_[A-Za-z0-9]{30,}|github_pat_[A-Za-z0-9_]{30,})",
    "AWS access key": rb"(?:AKIA|ASIA)[A-Z0-9]{16}",
    "JWT credential": rb"eyJ[A-Za-z0-9_-]{15,}\.[A-Za-z0-9_-]{15,}\.[A-Za-z0-9_-]{15,}",
    "credential URL": rb"https?://[^\s/<>\"]+:[^\s/<>\"]+@",
    "device home path": rb"/data/(?:data|user/0)/com\.termux/files/home/",
}
# This literal formats an ephemeral, random proxy credential at runtime. It is
# source code rather than a stored credential; all other credential URLs fail.
RUNTIME_URL = b'format!("http' + b'://claude:{}@{}", self.token, self.address)'


def candidates():
    result = subprocess.run(
        ["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"],
        cwd=ROOT, check=True, capture_output=True,
    )
    return sorted({Path(name.decode()) for name in result.stdout.split(b"\0") if name})


def inspect(files):
    failures, contents = [], {}
    fixture_sources = json.loads((ROOT / "tests/fixtures/sources.json").read_text())
    for relative in files:
        name = relative.as_posix()
        path = ROOT / relative
        if (relative.parts[0] in FORBIDDEN_ROOTS
                or relative.parts[0].startswith(".test-")
                or any(part in FORBIDDEN_NAMES or (part.startswith(".env") and part != ".env.example") for part in relative.parts)):
            failures.append({"file": name, "rule": "local state or credential filename"})
        if path.is_symlink() or not path.is_file():
            failures.append({"file": name, "rule": "source must be a regular file"})
            continue
        data = path.read_bytes()
        contents[relative] = data
        if len(data) > 1024 * 1024:
            failures.append({"file": name, "rule": "source file exceeds 1 MiB"})
        if data.startswith((b"\x7fELF", b"MZ")):
            failures.append({"file": name, "rule": "compiled executable"})
        scanned = data
        if name == "src/proxy/mod.rs":
            scanned = scanned.replace(RUNTIME_URL, b"RUNTIME_PROXY_URL")
        for label, pattern in PATTERNS.items():
            if re.search(pattern, scanned):
                failures.append({"file": name, "rule": label})
        if path.suffix == ".apk" and relative.parent != Path("tests/fixtures"):
            failures.append({"file": name, "rule": "unexpected binary package"})
        if b"\0" in data and name != "tests/fixtures/musl-1.2.6-r2.apk":
            failures.append({"file": name, "rule": "unexpected binary file"})
    for name, source in fixture_sources.items():
        relative = Path("tests/fixtures") / name
        data = contents.get(relative)
        if data is None or hashlib.sha256(data).hexdigest() != source["sha256"]:
            failures.append({"file": str(relative), "rule": "fixture provenance mismatch or missing fixture"})
    # Check local Markdown references against the publishable file set, not the
    # developer's directory, where an ignored file can conceal a broken link.
    for relative, data in contents.items():
        if relative.suffix != ".md":
            continue
        for target in re.findall(r"\]\(([^)]+)\)", data.decode()):
            target = target.strip("<>").split("#", 1)[0]
            if not target or re.match(r"[a-zA-Z][a-zA-Z0-9+.-]*:", target):
                continue
            resolved = (ROOT / relative.parent / target).resolve()
            if not resolved.is_relative_to(ROOT) or resolved.relative_to(ROOT) not in contents:
                failures.append({"file": str(relative), "rule": "Markdown link missing from source snapshot"})
    return failures, contents


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--export", type=Path, help="export checked files to a new, empty directory")
    parser.add_argument("--report", type=Path)
    args = parser.parse_args()
    failures, contents = inspect(candidates())
    report = {
        "files_checked": len(contents),
        "source_bytes": sum(len(data) for data in contents.values()),
        "failures": failures,
        "files": {str(path): hashlib.sha256(data).hexdigest() for path, data in contents.items()},
    }
    if args.report:
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(json.dumps(report, indent=2) + "\n")
    if failures:
        print(json.dumps({"failures": failures}, indent=2))
        raise SystemExit(1)
    if args.export:
        destination = args.export.resolve()
        if destination.exists() and any(destination.iterdir()):
            raise SystemExit("Export destination must be empty.")
        destination.mkdir(parents=True, exist_ok=True)
        for relative, data in contents.items():
            path = destination / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
            path.chmod(0o755 if (ROOT / relative).stat().st_mode & 0o111 else 0o644)
    print(f"PASS: {len(contents)} Git candidate files, {report['source_bytes']} bytes; no configured secret/path patterns, excluded artifacts, broken local Markdown links, or fixture hash mismatches.")


if __name__ == "__main__":
    main()
