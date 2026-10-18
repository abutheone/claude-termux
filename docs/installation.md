# Installation

## One-line installer

```sh
curl -fsSL https://raw.githubusercontent.com/abutheone/claude-termux/main/install.sh | sh
```

This executes the current installer from this GitHub repository. HTTPS protects
transport; it does not provide independent proof of the maintainer's identity or
make repository changes automatically trustworthy. To review it before execution:

```sh
curl -fsSL https://raw.githubusercontent.com/abutheone/claude-termux/main/install.sh -o install.sh
cat install.sh
sh install.sh
```

The bootstrap checks standard ARM64 Termux before changing files. It installs
Termux build dependencies with pkg and checks GitHub for the latest stable release.
It builds that release tag with Cargo.lock, then calls the Rust doctor and setup
commands. Until the first release exists, it uses a reviewed, fixed source commit.
Cargo downloads build dependencies; this is not an offline installation. The Rust
setup verifies Claude and the loader before activation.

The launcher is linked at `~/.local/bin/claude-termux`. Builds are kept under
`~/.local/share/claude-termux-launcher/releases`. Setup keeps Claude releases and
login settings in their usual separate locations. To enable zsh tab completion,
run the one-line setup in [command setup](command-setup.md).

The installer leaves shell startup files and any existing `claude` command alone.
If `~/.local/bin/claude-termux` is an executable from an older manual install, the
installer copies it to a dated folder under
`~/.local/share/claude-termux-launcher/backups/`, verifies the copy, then switches
to the new launcher. Other conflicting files and unrelated symlinks are left alone.

Run the same curl line again to check for a newer stable GitHub release. If the
installed launcher is already current, it skips rebuilding and checks doctor/setup.
When a newer release is found, the installer builds and checks it before switching.
Older managed builds stay under `releases/` for recovery. A failed build or setup
leaves the previous shortcut active. An interrupted operation may leave an unlinked
launcher build; older builds are not automatically removed. The lock is a directory:
after a forced kill, ensure no installer is running before removing
`~/.local/share/claude-termux-launcher/install.lock` and retrying. Never remove a live
lock. Removing the launcher does not remove build dependencies or login settings.

Allow about 2 GB of free space for the build and initial installation; actual use
varies with the Rust toolchain and artifacts. The Rust setup command separately
requires 768 MiB of headroom before installing Claude.

## Manual installation

Run each line in Termux:

```sh
pkg install git rust clang openssl ca-certificates patchelf
mkdir -p ~/projects
cd ~/projects
git clone https://github.com/abutheone/claude-termux.git
cd claude-termux
OPENSSL_DIR="$PREFIX" cargo build --release --locked
./target/release/claude-termux doctor
./target/release/claude-termux setup
./target/release/claude-termux run
```

If you already have a checkout, open that folder instead of cloning again. This
workflow runs the repository build directly; the one-line installer builds a pinned
revision separately and does not update your development checkout.

## Maintaining the bootstrap

Keep install.sh limited to prerequisites, source/build orchestration, and publishing
the launcher shortcut. Do not duplicate Claude downloads, signature checks, account
handling, or platform compatibility logic from Rust setup. Change its pinned source
revision only to a reviewed commit whose Rust CI passes on both supported toolchains.
The bootstrap script itself is fetched from main, so review and test its changes
before recommending them. GitHub Releases are created by the repository workflow
only after both Rust toolchains pass CI; tags must match the Cargo package version.
The installer discovers the latest stable release, but does not update Claude Code
or silently trust arbitrary commits from `main`.

Before publishing installer changes, use [the standard project checks](publication.md).
The installer tests use fake package, Git and Cargo commands in temporary homes.
They check reruns, paths with spaces, safe failure behavior, conflicts, and
unsupported platforms. They do not prove a fresh phone can build successfully.
