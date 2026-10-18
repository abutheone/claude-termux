# claude-termux

A small Rust application for running Anthropic's official ARM64 musl build of
Claude Code directly in Termux. No proot, root access, community Claude installer,
or global shell/npm changes are needed.

Status: **experimental, foreground sessions only**. This is compatibility tooling,
not a sandbox. One-device testing and user reports are recorded in
[validation](docs/validation.md); other devices and background lifecycles remain unverified.

## Build

From a checkout of this repository on aarch64 Termux (standard `com.termux` prefix):

```sh
pkg install rust clang openssl ca-certificates patchelf
OPENSSL_DIR="$PREFIX" cargo build --release --locked
```

The application uses Termux's system OpenSSL for certificate and signature
verification. Rust dependencies are recorded in `Cargo.lock`; builds should always
use `--locked`. Rust's dependencies are third-party software too. We avoid community
Claude-specific wrapper code, not all external software.

## Beginner setup

After building, check the device and preview setup:

```sh
./target/release/claude-termux doctor
./target/release/claude-termux setup --dry-run
./target/release/claude-termux setup
./target/release/claude-termux run
```

`doctor` is read-only and offline by default. It checks ARM64 Android support,
Termux Bash, the CA bundle, the managed installation, trusted `patchelf` availability,
and storage. It does not create directories/lock files, run discovered dependencies,
read account credentials, install packages, or change shell/npm settings.

For explicit network diagnostics or machine-readable output:

```sh
./target/release/claude-termux doctor --network
./target/release/claude-termux doctor --json
```

`--network` checks signed npm metadata, the small signed Alpine loader package,
and verified HTTPS to Anthropic through our authenticated local proxy. It sends no
account token or inference request. Each network probe has a 15-second deadline.

`setup` shows its plan, then uses the verified installer. A healthy installed version
is retained by default; first-time setup uses the reviewed version **2.1.302**.
Repeated setup reuses the healthy installation without downloads or writes, even
when `patchelf` is no longer installed. A new installation needs `patchelf` and
768 MiB of free-space headroom. Failed checks stop setup without repairs or deletion.
`--dry-run` performs local checks and shows the plan without changes or downloads.

Read [doctor and setup](docs/onboarding.md) for recovery and custom-directory usage.

## Install a specific version

For an explicit version, use setup or the lower-level installer:

```sh
./target/release/claude-termux setup --version 2.1.302
./target/release/claude-termux install --version 2.1.302
./target/release/claude-termux inspect
```

Default managed directory: `~/.local/share/claude-termux`. For an isolated test,
specify `--root /absolute/path/to/test-install` on every command. An existing
nonempty unmanaged directory is never adopted. These commands do not replace
`claude` on PATH or alter an npm installation. See [command setup](docs/command-setup.md)
for an optional short command.

The installer authenticates npm metadata with pinned registry signing keys,
checks the archive's SHA-512, verifies the pinned Alpine package's signature and
payload hash, inspects ELF dependencies, patches a copy, and tests `--version`.
Activation happens only after these checks pass. Original binary bytes and an
integrity manifest remain in the release directory. Future dependency or signing
key changes fail closed until reviewed.

## Run

```sh
./target/release/claude-termux run
```

The first login uses Anthropic's browser flow. If the browser does not open, copy
the URL printed by Claude into your browser. Never send your login code to someone
else. The launcher does not handle your Anthropic password or decrypt HTTPS.

Claude's configuration is kept separately at `~/.config/claude-termux/claude`, or
at an absolute `CLAUDE_CONFIG_DIR` you supply. This directory is **not** a sandbox:
Claude still runs with your Termux filesystem and command permissions.

Only HTTPS CONNECT on port 443 is supported. The proxy admits these exact hosts:
`api.anthropic.com`, `claude.ai`, `claude.com`, `platform.claude.com`,
`mcp-proxy.anthropic.com`, and `code.claude.com`.

Admit another exact hostname explicitly when needed:

```sh
./target/release/claude-termux run --allow-host example.com
```

For npm registry downloads, use `run --allow-host registry.npmjs.org`. A 403 for
that host with the default policy is expected. npm packages may fetch additional
hosts or use install scripts; review those separately rather than opening every
proxy destination. Extra hosts can also be necessary for WebFetch or remote MCP. Private, loopback, special-use
and mixed public/private DNS answers are rejected by the proxy. CLI child processes
can make direct connections, so this allowlist is not an enforced network sandbox.

The launcher forwards the terminal, arguments, and exit status. The in-process
proxy uses a fresh random token per run; its URL is not stored or logged. Ctrl-C is
left for Claude to handle. SIGTERM requests a graceful exit, with a five-second
fallback to kill the main child. The proxy stops when the main child exits.

## Update, rollback and remove

Install another **explicit version** with the same `install` command. Existing
versions are immutable. A live foreground session blocks installation, rollback
and removal. There is no unattended update job.

```sh
./target/release/claude-termux install --version VERSION
./target/release/claude-termux rollback
./target/release/claude-termux uninstall
```

The previous active version is retained for rollback. Both native auto-updates and
manual self-update paths are disabled inside Claude. Use this tool's installer.
Older versions consume storage and are not automatically pruned in this release.
Uninstall refuses unexpected files and preserves Claude credentials/configuration,
the Rust toolchain, and your original npm installation.

## Development checks

```sh
cargo fmt --all -- --check
OPENSSL_DIR="$PREFIX" cargo clippy --locked --all-targets -- -D warnings
OPENSSL_DIR="$PREFIX" cargo test --locked
python -m unittest discover -s tests -p 'test_*.py'
python scripts/check-publish.py
python scripts/check-dependencies.py
```

The Python checks require Python 3.11 or later (`pkg install python git` on Termux).
The publication check scans Git candidate files, validates signed fixture hashes
and local documentation links, and rejects known credential patterns and build
artifacts. It is a heuristic scan, not a guarantee that every secret is detected.

The normal Rust tests are offline. Opt-in network tests use no Claude credentials and
make no inference requests:

```sh
OPENSSL_DIR="$PREFIX" cargo test --locked --test network -- --ignored --test-threads=1
```

Read [the architecture](docs/architecture.md), [security model](docs/security.md),
and [maintenance procedures](docs/maintenance.md) before changing trust or runtime
behavior. Device test results are recorded separately in [validation](docs/validation.md).

See [publication checks](docs/publication.md) for clean-source verification before
pushing a release.
