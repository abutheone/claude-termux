# Check, set up, and recover

Start with the read-only device check:

```sh
~/.local/bin/claude-termux doctor
```

`OK` means the check passed. `WARN` means setup can continue. `FAIL` means fix the
shown issue first. A missing Claude installation, patcher, or free space warns when
reusing a healthy installation; it blocks a new installation when needed.

Doctor does not download files, install packages, change settings, or read login
data. `doctor --network` is optional. It checks the artifact servers and HTTPS
connection without your Anthropic login or a Claude request.

## Set up Claude

```sh
~/.local/bin/claude-termux setup --dry-run
~/.local/bin/claude-termux setup
```

The preview does not change files or download anything. Setup keeps a healthy
installed version. First setup uses the reviewed version built into the launcher.
The installer checks downloaded files before activating them and preserves your
login and existing `claude` command. It does not install Termux packages; follow
the message shown by doctor if a package is missing.

Start Claude with:

```sh
~/.local/bin/claude-termux run
```

Follow the browser sign-in instructions. The launcher does not save a separate
login or ask you to share your password or code.

## Common problems

- **Not enough space:** free space in Termux and run `doctor` again. A new Claude
  setup needs 768 MiB free; the one-line build installer recommends about 2 GB.
- **`patchelf` missing:** run `pkg install patchelf`, then retry `setup`.
- **No connection:** check Termux internet access. `doctor --network` tests the
  supported downloads and Anthropic connection.
- **Damaged or unknown installation:** doctor and setup explain the problem. They
  do not erase or automatically repair files they cannot verify.
- **Custom install folder:** pass the same `--root /absolute/path` to doctor,
  setup, run and uninstall.

More detail: [installer behavior](installation.md), [network and security](security.md),
[known test limits](validation.md).
