# Pinned public keys

These files are public verification material, not credentials. Unknown signing
keys fail verification. They are never fetched as part of a normal install.

Alpine key:

* Name: alpine-devel@lists.alpinelinux.org-616ae350.rsa.pub
* Official HTTPS source: https://alpinelinux.org/keys/alpine-devel@lists.alpinelinux.org-616ae350.rsa.pub
* Cross-check: https://raw.githubusercontent.com/alpinelinux/aports/master/main/alpine-keys/alpine-devel@lists.alpinelinux.org-616ae350.rsa.pub
* SHA-256 of DER SubjectPublicKeyInfo: 479f16f4170a9ea0c65e1de854860949b1a549cfa1f43552f7a2e821c9e2f409
* Scope: musl 1.2.6-r2, Alpine v3.24, aarch64 APK v2.

npm keyring source: https://registry.npmjs.org/-/npm/v1/keys

Only pinned keys whose expiration is null are accepted. The expired historical key
is preserved in the source snapshot for provenance, but not trusted for installation.
The accepted key ID is SHA256:DhQ8wR5APBvFHLF/+Tc+AYvPOdTpcIDqOhxsBHRwC7U.
The signature authenticates registry metadata, not a separate Anthropic attestation.

Retrieved and cross-checked on 2026-10-18. See docs/maintenance.md for rotation.
