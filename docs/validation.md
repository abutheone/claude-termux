# Validation

## Device tested

The project was exercised on one ARM64 Android phone running native Termux. The
user reported successful Claude login, chat, file editing, shell commands, search,
Ctrl+C, and a documentation connector. These are reports from that device, not
independent tests on other phones.

The launcher and setup path were also exercised with official Claude Code 2.1.302.
The Rust installer authenticated the signed npm metadata and Alpine loader before
launch. Local doctor/setup checks covered dry runs, failed downloads, re-use, and
refusal to repair damaged or foreign files. Optional network diagnostics completed
without account credentials or an inference request.

## Automated checks

The GitHub workflow runs Python checks, source publication scan, shell syntax and
ShellCheck, formatting, Clippy, Rust tests and dependency review on stable Rust
and Rust 1.85. See the [latest Actions runs](https://github.com/abutheone/claude-termux/actions)
for the current result. Hosted CI tests Linux builds; it does not reproduce an
Android phone or authenticated Claude session.

Manual zsh completion was exercised in an interactive zsh session. Its generated
command list included the current CLI subcommands. The completion comes from the
CLI parser and provides file path completion for path options.

## Still unverified

- Compatibility on other Termux installs, Android devices, or CPU architectures.
- Background agents, device sleep, process recovery, or Android kill behavior.
- Every MCP server, hook, plugin, script shebang, or network host.
- OAuth refresh after long use, and Claude's own TLS error handling.
- A working Linux filesystem sandbox. Claude runs with the permissions available
  to the Termux app; this launcher does not isolate it.
- Power-loss recovery during an update or every child-process cleanup path.

Use the [security guide](security.md) for limits that affect daily use and
[maintenance](maintenance.md) to add new device test results.
