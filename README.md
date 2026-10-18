# claude-termux

Run Claude Code on ARM64 Android in Termux. This community project helps the
official Claude Code program run there; Anthropic does not make this project.

**Experimental:** tested on one phone. Background use is not supported. Claude
can access files and commands available to Termux, so review its permission prompts.

## Install

In Termux, paste this line. It builds the launcher and sets up Claude Code; allow
about 2 GB of free space and several minutes.

```sh
curl -fsSL https://raw.githubusercontent.com/abutheone/claude-termux/main/install.sh | sh
```

Read [what the installer does](docs/installation.md) before running it if you prefer.
Run the same line again later to check for and install the latest stable launcher release.

## Start Claude

```sh
~/.local/bin/claude-termux run
```

Follow its browser sign-in instructions. You need your own Anthropic account.

Run it again from any Termux folder with the same command. The installer leaves
an existing npm `claude` command alone. For zsh tab completion, manual setup, or help,
use the [project guide](docs/README.md).

## Project files

```text
install.sh   One-line installer
src/         Rust application
trust/       Public signing keys
tests/       Automated checks and fixtures
docs/        User guides, security notes, and maintenance
```

Report issues at [GitHub](https://github.com/abutheone/claude-termux/issues). Never
include passwords, login codes, or API keys.
