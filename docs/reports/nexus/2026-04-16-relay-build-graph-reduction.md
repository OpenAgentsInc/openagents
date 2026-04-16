# Nexus Relay Build Graph Reduction

Date: 2026-04-16

Related decision:

- `docs/adr/ADR-0004-nexus-relay-core-vs-training-demo-control-boundary.md`

## Purpose

This note captures the first retained build-graph reduction after extracting
the Nexus-facing Psionic lane contract out of the full `psionic-train` crate.

## Change

The implemented first cut is:

- add a small local `crates/psionic-train-contract` crate
- move `nexus-control` from `psionic-train` to that local contract crate
- update the narrowed Nexus build context so the new crate is present in both
  the warm-builder and Cloud Build fallback lanes

This is intentionally narrower than a service split. The deployed Nexus runtime
still stays one `nexus-relay` binary with in-process authority routes.

## Evidence

Before the extraction, the repo-root command:

```bash
cargo tree -p nexus-relay --prefix none | rg '^psionic-' | sed 's/ v.*//' | sort -u
```

showed `25` unique `psionic-*` crates in the `nexus-relay` graph, including:

- `psionic-train`
- `psionic-cluster`
- `psionic-router`

After the extraction, the same command shows `13` unique `psionic-*` crates:

- `psionic-array`
- `psionic-backend-cuda`
- `psionic-backend-metal`
- `psionic-catalog`
- `psionic-compiler`
- `psionic-core`
- `psionic-ir`
- `psionic-models`
- `psionic-nn`
- `psionic-runtime`
- `psionic-sandbox`
- `psionic-train-contract`
- `psionic-transformer`

Specific removed crates from the relay path:

- `psionic-train`
- `psionic-cluster`
- `psionic-router`

The staged Nexus-only context also resolves with the reduced graph:

```bash
tmpdir="$(mktemp -d)"
scripts/deploy/nexus/stage-build-context.sh "$tmpdir" >/dev/null
cargo metadata --manifest-path "$tmpdir/Cargo.toml" --locked --format-version 1 --no-deps
cargo tree --manifest-path "$tmpdir/Cargo.toml" -p nexus-relay --prefix none
```

That staged path now shows the same `13` unique `psionic-*` crates and does
not reintroduce `psionic-train`.

## Conclusion

This first extraction does not finish the broader relay versus training/control
split, but it does remove the full `psionic-train` runtime from routine
`nexus-relay` hotfix builds. That is the first concrete reduction required by
the April 15 iteration-speed audit.
