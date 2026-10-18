# claude-termux

Run Claude Code in Termux on your Android phone, without root or a separate Linux environment.

This community project helps Anthropic's official Claude Code run on Termux. It is not made by Anthropic.

**Experimental:** tested on one ARM64 Android device. Other phones may need fixes. Keep Termux open while using it; background use is not supported yet.

## Get started

You need Termux on an ARM64 Android phone, internet access, and at least 768 MiB of free space. Claude Code requires an Anthropic login; this project does not include a subscription or API credits.

Run each command below in Termux, one line at a time.

**1. Download the project and build the launcher.** This can take a few minutes.

```sh
pkg install git rust clang openssl ca-certificates patchelf
mkdir -p ~/projects
cd ~/projects
git clone https://github.com/abutheone/claude-termux.git
cd claude-termux
OPENSSL_DIR="$PREFIX" cargo build --release --locked
```

If you already downloaded this project, open its folder and continue below.

**2. Check your phone and install Claude Code.** If a check fails, follow the message before continuing.

```sh
./target/release/claude-termux doctor
./target/release/claude-termux setup
```

Setup downloads and checks the required files. You do not need to install Claude Code through npm first.

**3. Start Claude Code.** Follow the login instructions it shows. If the browser does not open, copy the displayed link into your browser.

```sh
./target/release/claude-termux run
```

## Use it again

Open Termux and run:

```sh
cd ~/projects/claude-termux
./target/release/claude-termux run
```

Want a shorter `claude` command? See [command setup](docs/command-setup.md).

## Good to know

- Claude can read and change files available to Termux. Review permission requests carefully.
- Setup keeps your existing npm installation and shell settings as they are.
- Claude updates are managed through this launcher. New versions may need compatibility fixes.
- Some websites and connected tools need extra network settings. See [network access](docs/security.md).

## Need help?

Run this to check your setup and connection:

```sh
./target/release/claude-termux doctor --network
```

Read the [setup guide](docs/onboarding.md), [test results](docs/validation.md), or [report a problem](https://github.com/abutheone/claude-termux/issues). Never share passwords, login codes, or API keys in an issue.

For contributors: [how it works](docs/architecture.md) · [security](docs/security.md) · [maintenance](docs/maintenance.md) · [publication checks](docs/publication.md).
