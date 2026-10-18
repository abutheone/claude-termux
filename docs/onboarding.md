# Doctor and setup

These commands prepare the official Claude executable for supported ARM64 Termux.
They do not install a separate AI model or implement Claude's tools.

## Start with local checks

```sh
./target/release/claude-termux doctor
```

Statuses are OK, WARN, FAIL and SKIP. Exit status is 0 when no required check failed,
or 1 when there is a failure. A missing installation is a warning if the device is
otherwise ready to install. Missing `patchelf` blocks a new install but only warns
when an already verified active release can be reused. Low storage follows the
same distinction. Unsupported platforms, broken installation metadata, missing
required system files and non-executable filesystems block setup.

Doctor reads installed binary hashes without executing Claude. It never creates
an installation root or lock file, executes a discovered dependency, reads login
credentials or performs automatic repairs. It may take a few seconds to hash the
large installed executables. Use a release build rather than a debug build.

A custom trusted patcher can be checked explicitly:

```sh
./target/release/claude-termux doctor --patchelf /path/to/trusted/patchelf
```

A path found by doctor has not been executed or certified safe. The standard
recommended dependency is the Termux package `pkg install patchelf`.

## Preview, then install

```sh
./target/release/claude-termux setup --dry-run
./target/release/claude-termux setup
```

Setup prints the selected version, destination and intended action. Invoking setup
runs that plan without an extra confirmation prompt; use --dry-run to preview only.
The plan uses the healthy installed version by default, or the reviewed first-install
version when nothing is active. Specify `--version X.Y.Z` to choose another exact
release. No latest-version discovery or silent downgrade occurs.

Setup does not install system dependencies automatically. Follow doctor's package
instructions and retry if a dependency is missing. A new installation needs 768 MiB
of headroom for the currently supported artifact sizes. Verification or dependency
changes fail closed rather than accepting an unknown key or runtime library.

An unchanged healthy installation is reused without invoking the installer. A new
release uses the existing signature, checksum, ELF and launch checks before atomic
activation. Shell commands, npm packages, login credentials and existing sessions
are not rewritten by setup.

After success, setup prints an executable command with correctly quoted paths.
Run it to start Claude and follow the official browser login. Setup does not sign
you in, request passwords or store a login token of its own.

## Custom directories

Use the same --root on doctor, setup, run and uninstall:

```sh
./target/release/claude-termux --root "$HOME/claude-test" doctor
./target/release/claude-termux --root "$HOME/claude-test" setup --dry-run
./target/release/claude-termux --root "$HOME/claude-test" setup
./target/release/claude-termux --root "$HOME/claude-test" run
```

Choose a new private directory under Termux home. Do not use a directory containing
personal files or Android shared storage. Setup refuses unmanaged or damaged
installation state and preserves its files rather than erasing them. Existing
installation directories must be private (mode 0700).

An interrupted first attempt may leave a managed root without an active release.
Doctor reports this as incomplete. Retry setup after resolving the download or
connectivity problem. Normal download failures clean their staging directory and
do not replace an active release. Power-loss/forced-kill recovery has the limitations
recorded in the maintenance guide; automatic repair of damaged state is not offered.

## Explicit network diagnostics

```sh
./target/release/claude-termux doctor --network
./target/release/claude-termux doctor --network --json
```

The probes run concurrently with individual 15-second deadlines. They verify npm
metadata signatures, download/verify the small pinned Alpine loader in memory, and
make an unauthenticated Anthropic API request through a temporary authenticated
local proxy. No account credentials or inference request is sent, and the proxy
is stopped afterward. The Anthropic probe expects HTTP 401 because it has no API
key; this is a successful connectivity result. TLS verification remains enabled.
These checks exercise the Rust client/proxy, not Claude's closed-source networking.

JSON output has schema_version 1, root, selected_version, installed_version,
needs_install, patchelf_path and a checks array. Each check has id, status, detail,
and an optional remedy. Unknown/partial state is reported, never repaired silently.
