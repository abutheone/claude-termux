# Architecture

One Cargo package produces one executable. A workspace or plugin framework would
add coordination cost without a second independently deployable component.
Modules separate trust, storage, networking and process ownership so those pieces
can evolve without rewriting the CLI.

```text
src/
  cli.rs                 Commands, flags, and completion source
  setup.rs               Read-only preflight plus the existing installer
  doctor/
    mod.rs               Shared diagnostic report and opt-in orchestration
    local.rs             Read-only device, storage and integrity probes
    network.rs           Bounded credential-free network diagnostics
  installation.rs        Immutable releases, activation, locks, removal
  launcher.rs            Foreground child and proxy supervision
  artifacts/
    download.rs          Bounded HTTPS downloads, no redirects
    npm.rs               Package identity, registry signature, SHA-512
    alpine.rs            Pinned APK v2 signature and payload verification
    elf.rs               Architecture, interpreter, dependency inspection
  proxy/
    protocol.rs          Bounded CONNECT parsing and authentication
    policy.rs            Exact hosts and checked destination addresses
    mod.rs               Async sockets, connection limits, shutdown
trust/                   Reviewed public signing keys, never private keys
tests/fixtures/          Official signed test inputs and provenance
```

## Artifact flow

1. Require an exact numeric Claude version; tags such as `latest` are not accepted.
2. Fetch metadata directly from npm over verified HTTPS without redirects.
3. Require a pinned, non-expiring registry key to authenticate package identity,
   version and SHA-512. Require the expected exact tarball URL.
4. Authenticate the pinned Alpine control segment with its RSA public key, then
   check the payload SHA-256 named in signed metadata. Check package identity.
5. Verify the Claude archive SHA-512 before reading its entries. Extract only the
   expected regular binary; no general archive unpacking or scripts are executed.
6. Require the expected ELF architecture/interpreter and the exact known library
   dependency. Keep the original and patch a copy with trusted Termux patchelf.
7. Write hashes and ELF reports into a versioned manifest, move the release into
   its final inactive location, and prove `--version` works there.
8. Atomically replace the activation record. A rejected candidate is removed and
   the active record remains untouched.

The final inactive location is needed because the patched interpreter contains an
absolute path to that release's loader. A shared lock protects running sessions;
an exclusive lock protects installation, activation and uninstall. Candidate and
activation writes are staged on the same filesystem. A power loss may leave an
inactive release or staging directory; it must never promote unverified bytes.

## Proxy flow

A single Tokio task accepts at most 64 concurrent connections on an ephemeral IPv4
loopback port. A 256-bit random per-session token is sent using HTTP proxy Basic
authentication. Parsing uses `httparse`, and credential comparison uses `subtle`.
Authentication happens before DNS resolution or destination connection.

Only exact configured DNS hosts on HTTPS port 443 are accepted. All resolved
addresses must be public; connection attempts use those already checked addresses,
avoiding a second lookup. Header reads, connection attempts, writes and idle
connections have separate bounds. The tunnel preserves bytes and half-closes;
it never terminates TLS. Task ownership cancels all connections on proxy shutdown.

## Boundaries for future work

Background agents need their own lifecycle design: a stable authenticated proxy
service, Claude's process-wrapper contract, health checks and recovery after
Android kills a process. A foreground proxy must not silently become a daemon.
MCP programs and subprocess shebang handling need independent device tests.
Add another module or crate only when a real deployment or ownership boundary
requires it. A new architecture requires artifact keys, ELF policies, and device
validation; it is not enabled by changing one architecture string.
