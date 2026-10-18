#!/bin/sh
# Bootstrap only: Claude downloads and verification remain in the Rust setup command.
set -eu

main() (
    umask 077
    repository=https://github.com/abutheone/claude-termux.git
    # Change only after this revision's Rust checks pass on both CI toolchains.
    revision=4e63549a82255f3a094ce050d692e9204fdf8526

    fail() { printf 'Installation stopped: %s\n' "$*" >&2; exit 1; }
    if [ "$(uname -o)" != Android ] || [ "$(uname -m)" != aarch64 ]; then
        fail 'This installer needs ARM64 Android with Termux.'
    fi
    [ "${PREFIX:-}" = /data/data/com.termux/files/usr ] ||
        fail 'Use the standard com.termux installation.'
    if [ -z "${HOME:-}" ] || [ ! -d "$HOME" ]; then
        fail 'HOME must be an existing directory.'
    fi
    case "$HOME" in /*) ;; *) fail 'HOME must be an absolute path.' ;; esac
    command -v pkg >/dev/null 2>&1 || fail 'The Termux pkg command is missing.'

    bin_directory=$HOME/.local/bin
    state=$HOME/.local/share/claude-termux-launcher
    destination=$bin_directory/claude-termux
    # Check before package installation or building. Never follow a directory symlink.
    for directory in "$HOME/.local" "$bin_directory" "$HOME/.local/share" "$state" "$state/releases"; do
        [ ! -L "$directory" ] || fail "Directory is a symlink: $directory"
        [ ! -e "$directory" ] || [ -d "$directory" ] || fail "Not a directory: $directory"
    done
    if [ -L "$destination" ]; then
        existing=$(readlink "$destination")
        case "$existing" in
            "$state/releases/"*/claude-termux) ;;
            *) fail "An unrelated command already exists at $destination; keep it or move it yourself." ;;
        esac
    elif [ -e "$destination" ]; then
        fail "An existing command is at $destination; keep it or move it yourself."
    fi

    mkdir -p "$bin_directory" "$state/releases"
    lock=$state/install.lock
    mkdir "$lock" 2>/dev/null || fail "Another installer may be running. If it was interrupted, remove $lock and retry."
    work=
    trap 'if [ -n "$work" ]; then rm -rf -- "$work"; fi; rmdir "$lock"' EXIT
    trap 'exit 130' INT
    trap 'exit 143' TERM HUP
    work=$(mktemp -d "$state/build.XXXXXXXX")

    printf '\nInstalling Termux build tools…\n'
    pkg install -y git rust clang openssl ca-certificates patchelf
    printf '\nDownloading reviewed launcher source…\n'
    git init -q "$work/source"
    git -C "$work/source" remote add origin "$repository"
    git -C "$work/source" fetch -q --depth 1 origin "$revision"
    actual=$(git -C "$work/source" rev-parse FETCH_HEAD)
    [ "$actual" = "$revision" ] || fail 'The downloaded source revision does not match.'
    git -C "$work/source" checkout -q --detach FETCH_HEAD
    printf '\nBuilding the launcher. This can take several minutes…\n'
    (
        cd "$work/source"
        CARGO_TARGET_DIR="$work/source/target" OPENSSL_DIR="$PREFIX" cargo build --release --locked
    )
    candidate=$work/source/target/release/claude-termux
    if [ ! -f "$candidate" ] || [ -L "$candidate" ] || [ ! -x "$candidate" ]; then
        fail 'The build did not produce a launcher.'
    fi
    printf '\nChecking your phone and setting up Claude Code…\n'
    "$candidate" doctor
    "$candidate" setup

    # Publish only after setup succeeds. Keep previous launcher builds for recovery.
    release=$(mktemp -d "$state/releases/$revision.XXXXXXXX")
    install -m 700 "$candidate" "$release/claude-termux"
    link=$work/claude-termux
    ln -s "$release/claude-termux" "$link"
    mv -Tf "$link" "$destination"
    printf '\nReady. Start Claude Code with:\n  "%s" run\n' "$destination"
    printf 'No shell settings or existing claude command were changed.\n'
)

main "$@"
