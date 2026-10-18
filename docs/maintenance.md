# Maintenance

## Routine changes

Keep changes focused on one module boundary. Check failures as well as successes
when changing signatures, paths, parsing, activation or cleanup. Run the checks at
the end of this guide. Review Cargo.lock changes instead of accepting dependencies
as incidental formatting.

Dependencies use compatible Cargo version ranges and a committed lockfile.
Application builds use --locked. Update dependencies deliberately with cargo update,
review changed direct/transitive dependencies, re-query OSV and run the same checks.
The crate is not automatically published. CI has read-only repository permissions.

## New Claude version

1. Fetch its exact official npm metadata and confirm a pinned registry signature.
2. Stage it with install --version. The installer refuses new ELF requirements.
3. Check inspect, --version, --help, valid HTTPS and expired-certificate rejection.
4. Before recommending normal use, test browser login/refresh, one streamed reply,
   file read/edit/search, command cancellation, Termux scripts and selected MCP.
5. Verify signal cleanup and recovery. Preserve the previous active release.
6. Record results, device/tool versions, and untested features in validation.md.

Version checks do not count as authenticated integration tests. Tests that consume
paid inference or need credentials should be run explicitly with the user's account.

## Alpine or npm key rotation

Do not add a generic accept-any-key fallback. Obtain a new key from the official
HTTPS source and cross-check the official source repository or published key
fingerprint. Record the source and fingerprint in trust/README.md. Review expiration,
algorithm, package identity and payload hash behavior. Update the pinned artifact
URL/version and signed fixtures together. Run tampering/truncation tests. A signer
rotation is a release change, not an automatic retry.

## Storage and recovery

Each release retains a patched binary, original binary, loader and manifest. Expect
at least 768 MiB free for setup's conservative installation headroom; future
binary sizes can change. Retained releases cost additional space. Downloads have
hard limits and failed downloads remain inside disposable staging directories.

If interrupted, inspect the managed directory. The active.json record is replaced
atomically. An unrecorded release has not been activated; verify it before reusing it.
Do not manually point active.json at unchecked files. A stale .stage-* or .check-*
directory makes uninstall refuse deletion; inspect and remove only that known stale
directory, then retry. Lock files are advisory file locks, not PID files: an exited
process releases its lock automatically.

A failed candidate does not change active.json. Rollback verifies the previous
release's hashes before switching. Full interrupted-update and power-loss testing
is not yet complete. Do not promise crash consistency beyond tested behavior.

## Adding background support

Create a separate design change and device test plan. Cover a stable private proxy
service, credential rotation, readiness/authentication checks, shared ownership,
Claude processWrapper handling, supervisor environment, shell exit, device sleep,
Android process kills, and uninstall with live clients. Do not merely remove the
foreground restriction.

## Doctor and setup changes

Keep local diagnostics read-only. Network diagnostics remain explicit,
credential-free and bounded. Setup must reuse the existing verified installer;
do not add a second download or patch path. Retain healthy versions by default.
Keep shell completion generated from the CLI so it follows command changes.

## Development checks

Use the standard [publication checks](publication.md) before sharing changes.
After changing CLI arguments, load completion in an interactive zsh and check the
suggestions. Keep project trees and command examples in sync with the actual files.
See [installation](installation.md) for the bootstrap and manual build.
