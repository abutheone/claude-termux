# Project guide

Start with the README for installation and everyday use.

## Project map

```text
claude-termux/
├── install.sh               One-line Termux installer
├── src/                     Rust command, installer, checks, and network proxy
├── tests/                   Rust and Python checks with public fixtures
├── scripts/                 Source and dependency review tools
├── trust/                   Pinned public signing keys
├── .github/workflows/       Hosted checks for each change
├── docs/
│   ├── README.md            This guide and reading order
│   ├── installation.md      Installer and manual build
│   ├── command-setup.md     zsh completion and optional shortcut
│   ├── onboarding.md        Device checks and recovery
│   ├── security.md          Trust and safety limits
│   ├── architecture.md      How the code is organized
│   ├── validation.md        Device reports and remaining gaps
│   ├── maintenance.md       Updates and contributor checks
│   └── publication.md       Source review before sharing
├── Cargo.toml               Rust package and dependencies
└── Cargo.lock               Exact Rust dependency versions
```

`target/`, `.tools/` and `test-results/` are local build or development files;
they are excluded from the public source.

## Choose a guide

1. [Install or build it](installation.md)
2. [Start Claude and diagnose issues](onboarding.md)
3. [Enable zsh completion](command-setup.md)
4. [Understand its security limits](security.md)
5. [Read how it works](architecture.md)
6. [Check device results](validation.md)
7. [Maintain or publish changes](maintenance.md) · [publication checks](publication.md)
