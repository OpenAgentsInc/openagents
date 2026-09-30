# Dependency review

Run the dependency gate before a release or a dependency update:

```sh
./scripts/check-dependencies.sh
```

The command uses Rust 1.97.1 and `cargo-deny` (verified with 0.20.2).
Install the reviewed checker with `cargo +1.97.1 install cargo-deny
--version 0.20.2 --locked` if it is absent. Required checks run locally or
on non-GitHub infrastructure; this policy adds no GitHub automation.

## What the gate checks

[`deny.toml`](../deny.toml) checks the resolved workspace graph with all
features and development dependencies. It does not restrict the graph to the
host target. This is dependency inspection, not evidence that every target
or feature compiles. `--locked` prevents dependency resolution from changing
the reviewed lockfile.

- Vulnerability, unsoundness, and maintenance advisories fail unless their
  exact advisory ID has an explicit exception. Yanked versions fail.
- Dependency licenses must satisfy the enumerated SPDX allowlist. A dual
  license can satisfy the policy through an allowed alternative; an `AND`
  expression must satisfy both requirements. Unknown licenses fail.
- Registry packages must come from crates.io. Unapproved Git sources and
  other registries fail. Local workspace code requires the provenance review
  below; a path dependency is not proof of trustworthy origin.
- A wildcard version requirement on a registry dependency fails, because it
  leaves the reviewed version to whatever the resolver picks next. Workspace
  path dependencies are exempt. Duplicate versions of one crate are reported
  as warnings, not failures; the numeric stack and the two tokenizer releases
  pin several, and collapsing them is a dependency change to review on its
  own, not a policy setting.

The checker fetches RustSec data during normal online runs. Offline data may
be at most seven days old, but release review should run online. A clean
result means the inspected graph satisfies this policy and current advisory
records. It does not establish the absence of undiscovered defects.

License compatibility checks do not assemble a distribution's notices. Before
shipping, retain required license texts, copyright notices, and attribution
for the dependencies actually distributed. Review bundled native code and
external assets separately from Cargo's declared license expressions.

## Unicode editing dependency

Rust Native's local editor adds a direct dependency on `unicode-segmentation`
1.13.3, the version already in both the root and phone lockfiles. It supplies
Unicode grapheme and word boundaries so movement and deletion preserve emoji
and combining sequences. The package comes from crates.io and declares
`MIT OR Apache-2.0`; its license texts remain in the registry package. This
adds an existing dependency edge without changing the resolved package version.

The editing design is reimplemented from Zeron's public `ComposerInput` at
`ed3b1aae4a5189eef67143db7b8c5c3ee7a933c5`. No GPUI, Zeron source, assets, or
new platform runtime are vendored by this foundation.

## The paste exception

[RUSTSEC-2024-0436](https://rustsec.org/advisories/RUSTSEC-2024-0436) reports
that `paste` is unmaintained and lists no patched version. It is not an
established exploitable vulnerability. The exception applies only to that
advisory and the inspected `paste 1.0.15`; it does not waive future advisories.

Owner: **OpenAgents Kev maintainers**. Review before **2026-10-20 UTC**.
The script refuses on that date or later and refuses a different resolved
`paste` version. `cargo-deny` also fails if an ignored advisory is no longer
encountered, so obsolete exceptions must be removed.

At `94944556de`, and unchanged at `5f0e67b9f2`, the dependency paths are:

- `kev -> candle-core 0.11.0 -> gemm 0.19.0` and its `gemm-c32`,
  `gemm-c64`, `gemm-common`, `gemm-f16`, `gemm-f32`, and `gemm-f64` crates.
- `gemm-common 0.19.0 -> pulp 0.22.3 -> paste`.
- `kev -> candle-core 0.11.0 -> tokenizers 0.22.2 -> paste`.
- `kev -> tokenizers 0.23.2 -> paste`.

`candle-nn 0.11.0` also reaches Candle's graph. Reproduce the paths with:

```sh
cargo +1.97.1 tree --locked --workspace --all-features -i paste
```

The current decision is to retain the reviewed dependency temporarily,
rather than introduce an unverified numeric-stack replacement. At review,
check upstream releases and replacement provenance, then remove the exception
or record a new reason and date in both the policy and the script. A proposed
replacement must pass Kev's API fixtures and affected numerical/conformance
fixtures, including relevant feature-specific paths. Do not treat an API-only
pass as numerical equivalence or suppress the whole maintenance category.

## Source provenance and repository licensing

All registry packages in the inspected graph originate from crates.io; no
Git dependency is admitted. Cargo.lock pins their versions and registry
checksums. This does not review a newly introduced local or vendored source
by itself. For each such addition, identify its public source, license, and
revision, preserve required attribution, and review the actual copied files.
A design reimplementation must be identified in its commit message. Private
sibling repositories are reference material, not sources to copy into this
public repository.

The relay documents its extraction from the public CC0 `immortal-relay`;
ATIF documents a reimplementation of the Harbor trajectory design. Preserve
those provenance statements and inspect new carried-over code independently.
Models, weight files, Apple framework terms, and non-Cargo assets require their
own distribution review; the dependency check does not license them.

The September 20 audit found conflicting repository declarations. As reviewed
on September 26, the root [LICENSE](../LICENSE) contains Apache-2.0 and the root
README no longer labels the repository CC0-1.0. The workspace package metadata
still omits a `license` field; the earlier count of 13 packages is obsolete.
Preserve component-specific provenance, including the relay's public CC0 source.

`licenses.private.ignore = true` excludes unpublished workspace packages from
license classification while keeping their dependencies under review. A passing
dependency check does not validate the repository's own distribution metadata
or assemble the required notices.

## Terminal PTY dependency

`crates/coder-pty` (NIP-TERM terminal sessions) needs a pseudo-terminal. It
uses `libc` 0.2 directly — `openpty`, `setsid`, `TIOCSCTTY`, `TIOCSWINSZ`,
`tcgetpgrp`, `poll`, and `killpg` — and adds no PTY crate. Reviewed on
2026-09-26:

- `libc` is already in the graph through `crates/supervise` and
  `crates/coder-boundary`, so the resolved graph gains no new registry
  package. The crate's other dependencies (`base64` 0.22, `serde`,
  `serde_json`, and `tempfile` for tests) were already resolved as well.
- A PTY crate such as `portable-pty` would add a second process-spawning path
  beside the supervisor's process-group conventions, plus Windows console
  support this crate refuses to use. The system calls above are the whole
  requirement.
- The host half is Unix and Windows. `libc` and `supervise` are target-gated
  dependencies of the optional `host` feature; on Windows it uses ConPTY
  through `windows-sys` 0.61 instead
  ([#9980](https://github.com/OpenAgentsInc/openagents/issues/9980)), and
  on another platform the host compiles and refuses every open as
  `unavailable`. The client half builds without them.

On Windows ([#9980](https://github.com/OpenAgentsInc/openagents/issues/9980)),
`supervise` (job objects), `coder-pty` (ConPTY), `acp-client`, the new
`private-fs` (owner-only DACLs), and `coder-host` (the control pipe and
Credential Manager) call Win32 through `windows-sys` 0.61, which the graph
already resolved for `tokio` and `iroh`; `knowledge` and `ext-eval` take
their random bytes from `getrandom` 0.3, also already resolved. The
resolved graph gains no registry package. `coder-boundary`'s Windows
backend ([#9983](https://github.com/OpenAgentsInc/openagents/issues/9983)):
the AppContainer profile, its DACL entries, the launcher's
`CreateProcessW`, and the snapshot's `NtCreateFile` opens, also calls
`windows-sys` 0.61 directly and adds no package.
- On glibc the crate links `libutil`, which holds `openpty` before glibc 2.34
  and remains as an empty compatibility library afterwards.
- macOS implements `openpty` with the non-reentrant `ptsname`, and concurrent
  calls failed during testing. The crate serializes PTY allocation and
  spawning within the process.

## Host TLS dependencies

`crates/coder-host` terminates TLS on its WebSocket listener so a `wss` hint
needs no forwarder. It depends directly on `rustls` 0.23 (with only the
`ring`, `std`, and `tls12` features) and `tokio-rustls` 0.26 (without
default features, and only in the optional `host` feature). Reviewed on
2026-09-27:

- Both crates were already in the resolved graph through
  `tokio-tungstenite`'s `rustls-tls-webpki-roots` feature, at the locked
  `rustls` 0.23.45 and `tokio-rustls` 0.26.5, so the graph gains no new
  registry package. The only `Cargo.lock` change is the two new edges from
  `coder-host`.
- The features match `nostr-transport` and `verse`: the `ring` provider,
  never `aws-lc-rs`, so no C toolchain or prebuilt native library is added.
- PEM parsing uses `rustls-pki-types`, which `rustls` re-exports; no
  `rustls-pemfile` crate is added. No ACME client or certificate generator
  is added: the operator supplies the files, and the tests use checked-in,
  test-only PEM fixtures instead of `rcgen`.

## iroh transport

`crates/openagents-connect` carries QR pairing and the NIP-REACH direct
channel over [iroh](https://docs.rs/iroh/1.3.0/iroh/) QUIC connections
([design](coder/design/2026-09-29-auto-pairing.md), issue
[#9966](https://github.com/OpenAgentsInc/openagents/issues/9966)). The root
`Cargo.toml` pins `iroh` and `iroh-relay` exactly, at `=1.3.0`, as workspace
dependencies. Reviewed on 2026-09-29:

- Default features are off. The crate enables only `tls-ring` and
  `fast-apple-datapath`, so the graph has no `portmapper` (UPnP and
  NAT-PMP), no metrics service, and no `aws-lc-rs`: TLS is `rustls` with
  `ring`, as elsewhere in the workspace. Hole punching and the relay replace
  port mapping. iroh's default features would also bring `attohttpc`
  (MPL-2.0).
- The endpoint uses `presets::Minimal`, only our relay or none, and an
  in-memory address lookup. No n0 relay, n0 DNS, PKARR, or DHT is
  configured, so none of their code paths runs.
- The lockfile gains 120 registry packages, among them `noq`,
  `noq-proto`, and `noq-udp` (iroh's QUIC), `netwatch` and `netlink-*`
  (interface monitoring), `ed25519-dalek` 3 and `curve25519-dalek` 5,
  `reqwest` 0.13 (relay HTTPS), `rustls-platform-verifier`, and
  platform crates for Windows, macOS, and Android. No existing locked
  version changed.
- 1.3.0 was published on 2026-09-28. The design asks for a release at least
  seven days old; the owner directed this pin on 2026-09-29, one day after
  release, so that rule is waived for it. The next dependency update checks
  for a 1.3.x fix release.

Four license findings remain in iroh's graph. Each has a per-crate,
per-version exception in `deny.toml`; no identifier is allowed graph-wide:

- `spez` 0.1.2, `BSD-2-Clause`: a proc macro used by `n0-error`, iroh's
  error crate. It runs at compile time and contributes no code to a binary.
  BSD-2-Clause is permissive; retain its notice with the others.
- `ws_stream_wasm` 0.7.5, `async_io_stream` 0.3.3, and `pharos` 0.5.3,
  `Unlicense`: `iroh-relay`'s browser WebSocket client, a dependency only on
  `wasm32`. They appear because the gate inspects every target; they never
  build for macOS, Linux, iOS, or Android. A wasm build of this crate would
  need its own review.

With these exceptions, iroh adds no finding to the gate. At `e3bc2d4064`,
before iroh, the gate already failed on findings outside this graph: Git
sources under `openagents-wallet`'s `ldk-node`, the `bip21`, `hex_lit`,
`webpki-roots` 0.25, and `musig2` licenses, advisories for `cgmath` and
`wasmtime`, and a wildcard in `verse-ruins`. The run with iroh reports
exactly those and nothing else.

Nearby approval (issue
[#9975](https://github.com/OpenAgentsInc/openagents/issues/9975)) adds
`iroh-mdns-address-lookup`, pinned at `=0.5.0` as a workspace dependency
with default features off: 0.5.0 (2026-08-18) is the newest release at least
seven days old, and it takes `iroh` `^1`. It brings four registry packages:
`swarm-discovery` 0.6.3 (Apache-2.0; its own UDP multicast socket on
224.0.0.251 and ff02::fb, port 5353), `acto` 0.8.2, `smol_str` 0.1.24, and a
second `hickory-proto` (0.26.3), all MIT or Apache-2.0. `cargo deny check`
reports no new finding for them. The advertisement carries only the
`EndpointId`, IP addresses, and a display label
(`openagents_connect::nearby::advertised`); relay URLs are dropped.

`crates/openagents-mobile` is its own workspace and lockfile; adding
`openagents-connect` there (issue #9971) needs the same review of that
graph.

The crate checks for `aarch64-apple-darwin`, `x86_64-apple-darwin`,
`aarch64-apple-ios`, `aarch64-apple-ios-sim`, `aarch64-linux-android` (with
`cargo ndk` and NDK 27.1), and `x86_64-unknown-linux-gnu`. On Android the
app must call `iroh::dns::install_android_jni_context` before binding.

## Verification record

On 2026-09-20, `./scripts/check-dependencies.sh` passed advisory, license, and
source checks with `cargo-deny 0.20.2`. The sole advisory exception is the
maintenance finding above. No dependency version, source, or numeric
implementation changed. Numeric regression tests were therefore not rerun
for this policy-only change. The license findings in that run are historical; the source-provenance section
above records the later repository-text review.

Later on 2026-09-20 at `5f0e67b9f2`, the gate added the bans check and
passed all four: `advisories ok, bans ok, licenses ok, sources ok`. The bans
check reported 21 duplicate-version warnings (for example `thiserror` 1 and
2, `tokenizers` 0.22.2 and 0.23.2, `getrandom` three times) and no wildcard
requirement. The `paste` paths above were reproduced unchanged. The eight
allowed license identifiers are the ones the resolved graph declares; no
identifier is allowed that no dependency uses, and no clarification or
exception entry exists.

On 2026-09-22 the allowlist gained a ninth identifier:
`Apache-2.0 WITH LLVM-exception`. The `wasmtime` dependency (through
`plugin`) resolves to the Cranelift compiler crates, which declare that
expression. The LLVM exception keeps the Apache-2.0 terms while waiving
GPL-2.0 incompatibility from the patent clause and relaxing attribution for
compiler output; it is a permissive superset for downstream use. The
dependency is reachable and intentional, so the identifier is allowed
graph-wide rather than as a per-crate exception.

To inspect license assignments without changing policy:

```sh
cargo +1.97.1 deny --locked list
```

Cargo metadata and the lockfile can include packages that are not reachable
in the inspected graph. Do not add license exceptions solely because an
unused package appears there. The gate's resolved graph and diagnostics are
the evidence for which dependency licenses need review.

## Terminal emulator parser

`crates/coder-vt`, the terminal emulator behind the Coder mobile terminal
screen, parses output with `vte` 0.15 (Alacritty's escape-sequence state
machine, `Apache-2.0 OR MIT`, from crates.io) and measures character widths
with `unicode-width` 0.2. Reviewed on 2026-09-27:

- `vte` builds with default features off and only `std`: no `ansi` feature,
  so no `log`, `bitflags`, `cursor-icon`, or `serde`. Its two dependencies,
  `arrayvec` and `memchr`, were already in the resolved graph; `vte` is the
  one new registry package. `unicode-width` 0.2.0 was already resolved.
- `vte` is a pure parser. It decodes UTF-8 across calls and dispatches
  printable characters and control, escape, CSI, OSC, and DCS sequences; it
  keeps no grid, runs no command, and reads no clipboard. The grid, modes,
  and replies are `coder-vt`'s own, with every string and count bounded.
- Writing a parser here would duplicate a small, well-tested state machine
  used by a widely deployed terminal. A full emulator crate such as
  `alacritty_terminal` would bring its own event loop, PTY, and configuration.
