# Security model

## Trusted components

This project trusts Anthropic's official executable and the npm registry, Alpine's
musl build/signing key, Termux's system packages, the Rust compiler, and locked
Rust dependencies. It is not independently audited. Owning the wrapper source
makes the compatibility code inspectable, not automatically safe.

Alpine's pinned public key was obtained from its HTTPS key service and compared
against its official aports source mirror. Both matched. This is a TLS-bootstrapped
trust decision, not an offline independent identity check. npm public keys are also
pinned from its HTTPS key service. Keys are not refreshed automatically.

The supported APK uses legacy RSA/SHA-1 for its control signature and SHA-256 for
the compressed payload hash. The narrow verifier is intentionally limited to the
pinned APK v2 musl package; it is not a replacement for apk-tools. Crypto operations
use system OpenSSL for signature verification and the RustCrypto SHA-2 crate for
payload/file hashes rather than custom cryptographic implementations. Tests include
an official signed package and altered metadata/payloads.

## Protections

* HTTPS certificate validation for downloads; no redirect following or insecure flags.
* Exact artifact identities and origins; bounded metadata, archives and decompression.
* Registry signature plus archive SHA-512; Alpine signature plus payload SHA-256.
* No npm lifecycle scripts or arbitrary extraction paths.
* Private managed directories, no-follow checks on managed files, immutable releases.
* Installed hashes checked before execution; preservation of original binary bytes.
* Random per-run proxy credentials, constant-time comparison, loopback-only binding.
* Destination allowlist and rejection of private/special addresses before connection.
* No TLS interception, payload logging, or credential URL logging.
* No changes to shell startup, global npm configuration or the original claude executable.

## Explicit limits

* Working Claude Linux sandbox isolation has not been demonstrated. Run with
  ordinary permission prompts; never assume filesystem isolation.
* Other processes with the Termux UID can read or modify this tool and its files.
  Hash manifests detect accidental change; a same-UID attacker can change both
  binary and manifest. File modes do not isolate processes within Termux.
* Proxy credentials are in Claude's environment and inherited by its children.
  The token is not intended to defend against those children.
* The proxy allowlist is not a firewall. Claude and tools can connect directly.
* The proxy can observe destination hosts and traffic timing, but TLS payloads
  remain encrypted. Never turn off TLS verification to repair connectivity.
* Termux's bionic preload cannot be loaded into musl. Removing it can break scripts
  with foreign shebangs such as /usr/bin/env. Full subprocess compatibility is
  not established by a version check.
* Login refresh, streaming, MCP and background work require separate device tests.
  The validation record separates local checks from user reports. A successful
  Rust HTTP request does not prove Claude uses the same path for every feature.
* Android can kill the launcher or descendants unexpectedly. SIGKILL cannot run
  cleanup handlers; orphaned child processes and unfinished work remain possible.
* The foreground wrapper blocks obvious background CLI entry points but cannot
  prevent every internal or shell-triggered self-spawn. Background use is unsupported.
* OSV results identify reported advisories, not undiscovered flaws. OSV checks
  locked crates; Termux/OpenSSL and downloaded executable updates are separate.

## Sources

* https://docs.npmjs.com/about-registry-signatures/
* https://wiki.alpinelinux.org/wiki/Apk_spec
* https://alpinelinux.org/keys/alpine-devel@lists.alpinelinux.org-616ae350.rsa.pub
* https://github.com/alpinelinux/aports/tree/master/main/alpine-keys
* https://code.claude.com/docs/en/network-config
* https://code.claude.com/docs/en/sandboxing
* https://code.claude.com/docs/en/corporate-launcher
* https://github.com/termux/termux-exec-package
* https://source.android.com/docs/security/app-sandbox
