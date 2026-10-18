# Optional claude command

The default workflow runs `./target/release/claude-termux run`. If you want a short
`claude` command, install a copy of the reviewed Rust executable in your home first:

```sh
mkdir -p "$HOME/.local/bin"
install -m 700 target/release/claude-termux "$HOME/.local/bin/claude-termux"
```

Verify the managed release exists before configuring your shell:

```sh
"$HOME/.local/bin/claude-termux" inspect
"$HOME/.local/bin/claude-termux" run -- --version
```

For an interactive Bash or Zsh shell, define this function:

```sh
claude() {
    "$HOME/.local/bin/claude-termux" run -- "$@"
}
```

Add that function to your own shell startup file if you want it in future terminals.
It takes precedence over an existing executable for that shell and preserves the
old installation on disk. To undo it in the current shell, run `unset -f claude`;
remove the function from your startup file as well if you saved it there. Other
programs and child shells do not automatically inherit this function.

The default release directory is `$HOME/.local/share/claude-termux`; configuration
is `$HOME/.config/claude-termux/claude`. A custom `--root` must be added to the
function if you installed to a different directory.

Use the Rust command directly when configuring extra proxy destinations:

```sh
"$HOME/.local/bin/claude-termux" run --allow-host registry.npmjs.org
```

Use `claude-termux install --version VERSION` to update the managed Claude binary.
After reviewing and rebuilding this Rust application, deliberately replace its
installed copy while your sessions are closed. Rebuilding the repository does not
update an earlier installed copy automatically.

This guide does not overwrite npm-managed command files. If an existing npm install
is broken, remove it separately only after verifying the new setup works. Removing
a shell function does not uninstall this project's managed releases or credentials;
see the README for the separate uninstall command.
