# Publication checks

Share this repository as experimental aarch64 Termux compatibility tooling. The
source uses the standard `com.termux` runtime prefix, while user state comes from
HOME or an explicit root. Other app prefixes, architectures and phones require
separate validation. No compiled Claude executable is included in the repository.

## Check exactly what Git would include

Run from a Git checkout with Python 3.11+:

```sh
python -m unittest discover -s tests -p 'test_*.py'
python scripts/check-publish.py --report test-results/publication-audit.json
cargo fmt --all -- --check
OPENSSL_DIR="$PREFIX" cargo clippy --locked --all-targets -- -D warnings
OPENSSL_DIR="$PREFIX" cargo test --locked
python scripts/check-dependencies.py --output test-results/dependency-audit.json
```

The source check considers tracked files and untracked, unignored files. It checks
for known credential patterns, unexpected binaries, personal Termux home paths,
unsafe local-state filenames, missing fixture hashes and broken Markdown links.
Its report contains file hashes and rule names, never matching credential values.
Public keys and the attributed signed musl test fixture are intentional inputs.
The fixture contains Alpine/musl code, not the Claude executable; its upstream
copyright notice and exact source/hash accompany it.

The check cannot detect every possible secret. Review the actual file list with
`git status --short --untracked-files=all` and review any staged diff before a push.
Git history must be reviewed separately if commits are added later.

## Export and verify clean sources

Use a new directory under the ignored test-results directory:

```sh
python scripts/check-publish.py --export test-results/source-snapshot
cd test-results/source-snapshot
OPENSSL_DIR="$PREFIX" cargo build --release --locked
OPENSSL_DIR="$PREFIX" cargo test --locked
```

The exported directory contains only the checked source files. It excludes the
original .git directory, build output, developer tools, installed Claude releases,
configuration and local reports. Standard system packages and the locked Cargo
registry dependencies are still required. With dependencies already cached,
`--offline` can confirm there is no hidden network prerequisite for compilation.

Record results and compiler versions in validation.md. A successful local build
is not evidence that hosted GitHub CI or every phone has passed. The checked-in
workflow tests stable Rust and the declared minimum supported Rust on Linux when
GitHub runs it. Format checks use stable Rust, avoiding formatter drift between
compiler versions. Network tests remain explicit rather than running against
external services for every pull request.

## Repository status

This application sets `publish = false` for crates.io. That does not prevent
sharing its source on GitHub. There is no automatic push, release, account login,
or registry publication step in these checks. Configure the actual repository
remote only when its owner and destination have been chosen; never put a token
in a remote URL.

A source snapshot passes local publication checks when its inventory, build,
Clippy, offline tests and advisory lookup pass. Passing these checks is not a
security certification. Experimental feature limitations
remain visible in the README and validation record.
