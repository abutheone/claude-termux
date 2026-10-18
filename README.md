# claude-termux

Run Claude Code in Termux on your Android phone, without root or a separate Linux environment.

This community project helps Anthropic's official Claude Code run on Termux. It is not made by Anthropic.

**Experimental:** tested on one ARM64 Android device. Other phones may need fixes. Keep Termux open while using it; background use is not supported yet.

## Install

You need Termux on an ARM64 Android phone, internet access, and about 2 GB of free space for building and setup. Claude Code requires an Anthropic login; this project does not include a subscription or API credits.

Paste this single line into Termux:

```sh
curl -fsSL https://raw.githubusercontent.com/abutheone/claude-termux/main/install.sh | sh
```

The installer adds the build tools, builds the launcher, and sets up Claude Code. This can take several minutes. It runs code from this repository; you can [read the installer](install.sh) first. If Termux says `curl` is missing, run `pkg install curl` and retry.

## Start Claude Code

```sh
~/.local/bin/claude-termux run
```

Follow the login instructions. If the browser does not open, copy the displayed link into your browser. Keep Termux open while using Claude.

Want to build it yourself? See [manual installation](docs/installation.md). Want a shorter `claude` command? See [command setup](docs/command-setup.md).

## Good to know

- Claude can read and change files available to Termux. Review permission requests carefully.
- Setup keeps your existing npm installation and shell settings as they are.
- Claude updates are managed through this launcher. New versions may need compatibility fixes.
- Some websites and connected tools need extra network settings. See [network access](docs/security.md).

## Need help?

Run this to check your setup and connection:

```sh
~/.local/bin/claude-termux doctor --network
```

Read the [setup guide](docs/onboarding.md), [test results](docs/validation.md), or [report a problem](https://github.com/abutheone/claude-termux/issues). Never share passwords, login codes, or API keys in an issue.

For contributors: [how it works](docs/architecture.md) · [security](docs/security.md) · [maintenance](docs/maintenance.md) · [publication checks](docs/publication.md).
