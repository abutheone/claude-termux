#!/bin/sh
# Bootstrap only: Claude downloads and verification remain in the Rust setup command.
set -eu

main() (
    umask 077
    repository=https://github.com/abutheone/claude-termux.git
    # Used until the first GitHub Release is published. Change only after CI passes.
    fallback_revision=66a0f39af020a2a6010e6de38534b93e2db6fd9c

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
    command -v curl >/dev/null 2>&1 || fail 'Install curl with pkg install curl, then retry.'

    bin_directory=$HOME/.local/bin
    state=$HOME/.local/share/claude-termux-launcher
    destination=$bin_directory/claude-termux
    # Check before package installation or building. Never follow a directory symlink.
    for directory in "$HOME/.local" "$bin_directory" "$HOME/.local/share" "$state" "$state/releases" "$state/backups"; do
        [ ! -L "$directory" ] || fail "Directory is a symlink: $directory"
        [ ! -e "$directory" ] || [ -d "$directory" ] || fail "Not a directory: $directory"
    done
    legacy=0
    active_release=
    if [ -L "$destination" ]; then
        existing=$(readlink "$destination")
        case "$existing" in
            "$state/releases/"*/claude-termux) active_release=$existing ;;
            *) fail "An unrelated command already exists at $destination; keep it or move it yourself." ;;
        esac
    elif [ -e "$destination" ]; then
        if [ ! -f "$destination" ] || [ ! -x "$destination" ]; then
            fail "An existing non-executable file is at $destination; move it yourself."
        fi
        legacy=1
    else
        legacy=0
    fi

    mkdir -p "$bin_directory" "$state/releases" "$state/backups"
    lock=$state/install.lock
    mkdir "$lock" 2>/dev/null || fail "Another installer may be running. If it was interrupted, remove $lock and retry."
    work=
    link=
    backup_directory=
    published=0
    # The shell invokes this function indirectly through the EXIT trap.
    # shellcheck disable=SC2317
    cleanup() {
        if [ -n "$link" ]; then rm -f -- "$link"; fi
        if [ -n "$work" ]; then rm -rf -- "$work"; fi
        if [ "$published" = 0 ] && [ -n "$backup_directory" ]; then rm -rf -- "$backup_directory"; fi
        rmdir "$lock" 2>/dev/null || :
    }
    trap cleanup EXIT
    trap 'exit 130' INT
    trap 'exit 143' TERM HUP
    work=$(mktemp -d "$state/build.XXXXXXXX")

    printf '\nInstalling Termux build tools…\n'
    pkg install -y git rust clang openssl ca-certificates patchelf coreutils jq
    printf '\nChecking GitHub for the latest stable launcher release…\n'
    response_code=$(curl -sS -o "$work/release.json" -w '%{http_code}' \
        https://api.github.com/repos/abutheone/claude-termux/releases/latest) ||
        fail 'Could not check GitHub releases. Check your connection and retry.'
    case "$response_code" in
        200)
            tag=$(jq -er '.tag_name | select(test("^v[0-9]+\\.[0-9]+\\.[0-9]+$"))' "$work/release.json") ||
                fail 'The latest GitHub release has an invalid version tag.'
            source_ref="refs/tags/$tag"
            ;;
        404)
            printf 'No GitHub release exists yet; using the reviewed fallback revision.\n'
            source_ref=$fallback_revision
            ;;
        *) fail "GitHub release check returned HTTP $response_code." ;;
    esac
    git init -q "$work/source"
    git -C "$work/source" remote add origin "$repository"
    git -C "$work/source" fetch -q --depth 1 origin "$source_ref"
    actual=$(git -C "$work/source" rev-parse FETCH_HEAD)
    if [ "$response_code" = 404 ]; then
        [ "$actual" = "$fallback_revision" ] || fail 'The downloaded fallback revision does not match.'
    fi
    revision=$actual
    git -C "$work/source" checkout -q --detach FETCH_HEAD
    case "$active_release" in
        "$state/releases/$revision."*/claude-termux)
            printf '\nThe latest launcher release is already installed. Checking setup…\n'
            "$destination" doctor
            "$destination" setup
            printf 'No launcher update was needed.\n'
            exit 0
            ;;
    esac
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
    if [ "$legacy" = 1 ]; then
        backup_stamp=$(date +%Y%m%d-%H%M%S)
        backup_directory=$(mktemp -d "$state/backups/claude-termux.$backup_stamp.XXXXXXXX")
        cp -p "$destination" "$backup_directory/claude-termux"
        cmp -s "$destination" "$backup_directory/claude-termux" || fail 'The launcher backup did not verify.'
    fi
    link=$(mktemp "$bin_directory/.claude-termux.XXXXXXXX")
    rm -f -- "$link"
    ln -s "$release/claude-termux" "$link"
    mv -Tf "$link" "$destination"
    link=
    published=1
    printf '\nReady. Start Claude Code with:\n  "%s" run\n' "$destination"
    if [ "$legacy" = 1 ]; then
        printf 'Previous launcher backup: %s/claude-termux\n' "$backup_directory"
    else
        printf 'Previous managed launcher builds remain available under %s/releases.\n' "$state"
    fi
    printf 'No shell settings or existing claude command were changed.\n'
)

main "$@"
