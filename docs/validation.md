# Validation record

Date: 2026-10-18. Platform: native aarch64 Termux on Android.
Toolchain: rustc 1.99.1, cargo 1.99.1. Tool version: claude-termux 0.1.0.
Claude artifact: official @anthropic-ai/claude-code-linux-arm64-musl 2.1.302.
Loader: signed Alpine v3.24 musl 1.2.6-r2 package.

## Passed on this device

* cargo fmt --all -- --check.
* cargo clippy --locked --all-targets -- -D warnings.
* cargo test --locked: 39 offline Rust tests (36 library and 3 CLI integration tests).
* Six Python publication-guard regression tests, including redacted secret findings,
  local-state rejection, public/private key distinction, fixture tampering and links.
* The Git candidate publication scan passed. It checks source files only, excluding
  installed binaries, local credentials, build output and developer tools.
* A prompt-forwarding regression was fixed: prompt values such as `-p agents`
  are no longer mistaken for an unsupported background command.
* Both explicitly enabled network tests: authenticated HTTPS to Anthropic returned
  the expected unauthenticated API status; expired.badssl.com failed specifically
  with a certificate-expiry error. No account token or inference request was used.
* A release build completed. The resulting launcher executable is approximately 5 MiB.
* npm registry signature and tarball SHA-512 verification passed during installation.
* Alpine control signature and signed payload SHA-256 verification passed.
* The official executable is AArch64 little-endian ELF ET_EXEC (type 2), with the
  interpreter /lib/ld-musl-aarch64.so.1 and one required library,
  libc.musl-aarch64.so.1. The loader itself has no interpreter or shared dependencies.
* The staged native executable reported 2.1.302 (Claude Code) before activation.
* Release launcher --version and --help both succeeded with the in-process proxy.
* Deliberately modifying the installed test loader prevented launch. Original bytes
  and permissions were restored, and final inspection passed.
* Installing a nonexistent version failed without changing the active record and
  left no staging directory behind.
* Reinstalling the current version was idempotent.
* Rollback without a previous version failed without changing the active record.
* Uninstall refused a user-added file; after that test file was removed, it removed
  an otherwise clean managed directory successfully.
* OSV query of all 171 registry crate versions in Cargo.lock reported no advisories
  at check time. This is an advisory lookup, not a security audit or a check of
  Termux's system libraries.

Machine-readable local evidence lives in gitignored test-results/smoke.json and
test-results/dependency-audit.json. These contain no login credentials. Ordinary
launcher startup including file verification took approximately 2.5 seconds in one
run; this is not a general performance guarantee. Unoptimized debug hashing is much
slower, so use the release build for real artifact operations.

## Clean-source publication check

A checked source snapshot was exported with no developer tools, installed releases,
credentials, build output or original Git metadata. Its release build, Clippy,
39 offline Rust tests, six Python tests and formatter check passed using the system
Termux toolchain and cached Cargo dependencies with network access disabled for
Cargo. This checks source completeness, not installation on a fresh operating
system. The compiled snapshot executable also reported claude-termux 0.1.0.

All locked dependencies declare Rust minimum versions at or below 1.85 in their
metadata. Actual Rust 1.85 compilation and hosted GitHub CI remain pending; the
workflow is prepared to run them after publication. Ignore rules were checked
against build output, developer tools, test installations, logs and credential
paths. The repository currently has no commits or configured remote, so there is
no existing Git history to review. No commit or push was performed by these checks.

## Doctor and setup validation

Both commands were implemented and tested in the repository without replacing the
operator's installed Rust helper or Claude activation. Doctor uses a read-only
lock/inspection path; tests cover missing roots/locks, corrupt state, busy installers
and special-file rejection. Local checks never execute a discovered patcher.
Setup shares those checks and delegates installation to the existing verified
installer. New tests cover missing dependencies, the 768 MiB storage threshold,
read-only dry runs, failed preflight, safe reuse, failed-download retry and quoting
of the printed start command. Black-box CLI tests also preserve foreign files and
verify that JSON diagnostics create no home/configuration directory.

On native Termux, isolated real-command tests passed for:

* Missing-patcher diagnostics without root creation.
* A valid dry run with no files/downloads.
* An initial HTTP 404 download failure leaving no active version or staging files.
* Read-only incomplete-install diagnosis and successful real setup retry.
* Repeated setup without a patcher, with all installed file contents/inodes/mtime unchanged.
* Offline doctor with the same no-change verification.
* All optional network probes (signed npm metadata, signed loader and proxy HTTPS).
* Modified-loader detection without repair and setup refusal of damaged state.
* Successful diagnosis after restoring the original test loader.

The live helper and activation hashes were checked before/after and matched. No
account credentials or model inference were used. Local machine-readable evidence
is in the gitignored test-results/onboarding/results.json. A successful subagent
report still does not establish detached/background-session lifecycle support.

## User-reported integration results

The user subsequently reported successful browser login, streamed chat, file
create/read/edit/delete, shell commands, search, Ctrl-C cancellation and login
persistence after restart. A session summary supplied by the user also reported a
working Claude Docs MCP connector and one Haiku subagent. These are useful
one-device integration reports, not independently replayed automated tests or
proof that every MCP server and background lifecycle works.

The same session reported npm registry access failing with proxy HTTP 403. That
matches the default exact-host allowlist. `run --allow-host registry.npmjs.org`
explicitly admits that host; packages can contact additional domains and execute
install scripts, so it is not an unrestricted npm compatibility guarantee.

## Not established yet

* OAuth token refresh after expiration or compatibility across other phones.
* Claude's own TLS rejection path; the expired-certificate test exercised the Rust
  HTTP client through our proxy, not the closed-source Claude executable.
* Automated replay of the reported authenticated session; broad foreign-shebang,
  local/remote MCP, hooks and plugin compatibility.
* Background agents, supervisors, remote control, device sleep and Android kill recovery.
* Working Linux bubblewrap sandbox isolation on this phone.
* Successful rollback between two different real releases, or power-loss recovery.
* Automated signal/descendant cleanup tests for an authenticated Claude session.
* CI execution on a separate Linux host or the declared Rust 1.85 minimum toolchain.
* Independent audit of the verifier, proxy, launcher, or their dependencies.

Development/test installations, logs, installed command backups and credentials
are local state excluded from the source snapshot. The public setup guide uses
HOME-relative paths and describes optional command integration without requiring
another user's files. Publishing this source does not publish the Claude binary
or any authentication state.
