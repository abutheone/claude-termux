# Review before sharing changes

This repository contains compatibility code, trust files, tests, and documentation.
It does not contain Anthropic's Claude executable or login data.

## Check the changes

Run from the repository on Termux:

```sh
python3 -m unittest discover -s tests -p 'test_*.py'
python3 scripts/check-publish.py
cargo fmt --all -- --check
OPENSSL_DIR="$PREFIX" cargo clippy --locked --all-targets -- -D warnings
OPENSSL_DIR="$PREFIX" cargo test --locked
python3 scripts/check-dependencies.py
sh -n install.sh
shellcheck install.sh
cargo run --locked -- completions zsh > /dev/null
```

The source scan checks Git's publishable files for common credential patterns,
unexpected binaries, local paths, and broken Markdown links. It also checks the
public test-fixture hashes. It is a useful screen, not proof that every secret is
found. Review `git status --short --untracked-files=all` and the staged changes too.

To check that the repository can build without local helper files, export only the
reviewed source into a new ignored folder and build there:

```sh
python3 scripts/check-publish.py --export test-results/source-snapshot
cd test-results/source-snapshot
OPENSSL_DIR="$PREFIX" cargo build --release --locked
```

## GitHub checks

The GitHub workflow runs shell and Python checks, Rust formatting, Clippy, Rust
tests and the dependency advisory scan on stable Rust and the declared minimum
Rust version. Check the [Actions results](https://github.com/abutheone/claude-termux/actions)
after pushing. A local pass does not mean hosted checks passed.

The package is source code shared on GitHub. `publish = false` only prevents
publishing this crate to crates.io. Releases, Claude updates, and GitHub pushes are
separate actions. Do not place credentials in source files, commit messages, or Git
remote URLs. Python checks require Python 3.11 or later; install ShellCheck if it is
not available on your system.
