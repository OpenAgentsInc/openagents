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
