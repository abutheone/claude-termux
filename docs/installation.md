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
Termux build dependencies with pkg, fetches an exact reviewed Git commit, verifies
the fetched commit ID, builds with Cargo.lock, and calls the Rust doctor and setup
commands. Cargo downloads build dependencies; this is not an offline installation.
The existing Rust installer verifies Claude and the loader before activation.

The launcher is linked at ~/.local/bin/claude-termux. Builds are kept under
~/.local/share/claude-termux-launcher/releases. Setup keeps Claude releases and
login settings in their usual separate locations. No shell startup file or existing
claude command is changed. An unrelated existing claude-termux command is refused,
including copies installed manually; move it yourself only after reviewing it.

Rerunning rebuilds the pinned launcher and reuses a healthy Claude installation.
The shortcut changes only after doctor and setup succeed. A failed launcher build
or setup leaves the previous shortcut intact. An interrupted operation may leave
an unlinked launcher build; older builds are not automatically removed. The lock
is a directory: after a forced kill, ensure no installer is running before removing
~/.local/share/claude-termux-launcher/install.lock and retrying. Never remove a live
lock. Removing the launcher does not remove the build dependencies or login settings.

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
before recommending them. It does not silently discover newer Claude versions.

Run sh -n install.sh, ShellCheck, and the Python installer tests before publishing.
The Python tests use fake package, Git and Cargo commands in isolated temporary
homes. They cover reruns, paths containing spaces, failures preserving the previous
launcher, conflicting commands, unsupported platforms, and source revision mismatch.
They do not establish real package installation or fresh-phone build behavior.
