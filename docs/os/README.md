# CoderOS

Research and first specification for a Linux distribution optimized for
**Coder**: using it, being built by it, and running local Psionic models as
the default inference path.

This folder is a candidate-work source. It is not product authority, not a
ProductSpec, not a release promise, and not permission to start an ISO,
package repository, or new kernel. Current `AGENTS.md`, `INVARIANTS.md`,
`docs/psionic/`, Sol, tests, and owner gates keep their existing precedence.

```text
CLAIM
actor/session: CoderOS research docs
base: e44e22c67dc034ac1d104dab1b413ec04c6f4fda
worktree/branch: coderos-docs-20260829 / detached github/main
scope: research spec for a Coder-first Linux distro drawing on Omarchy
paths: docs/os/
hot files: none
hot contracts: none
verification: docs-only; whitespace and path-local review
claimed_at: 2026-08-29T17:15:00Z
```

## Reading order

1. [CODEROS.md](./CODEROS.md) — what CoderOS is, what it is not, and the
   sequencing law (Linux CLI first, ISO later).
2. [OMARCHY.md](./OMARCHY.md) — what Omarchy actually is, pinned to the local
   clone, and which ideas transfer.
3. [SUBSTRATE.md](./SUBSTRATE.md) — Arch, NixOS rebase demand, “Nix but Lua,”
   Guix/Scheme, and why none of those is yet a decision.
4. [LINUX-CLI.md](./LINUX-CLI.md) — the first concrete program: Coder and
   Psionic working as first-class Linux software.
5. [OPEN-QUESTIONS.md](./OPEN-QUESTIONS.md) — open questions and research
   avenues.
6. [RESEARCH-LOG-2026-08-31.md](./RESEARCH-LOG-2026-08-31.md) — dated log:
   the first Phase 0 work (Linux CLI build + Ollama integration on the RTX
   4080 box, and the systematic Ollama coding-model benchmark), with what to
   do once that run succeeds.

## Related OpenAgents documents

| Document | Role relative to this folder |
| --- | --- |
| [docs/psionic/INTENT.md](../psionic/INTENT.md) | Owner-accepted in-binary local inference. First supported platform is Apple Silicon. CoderOS needs the Linux backends this program deferred. |
| [docs/psionic/PLAN.md](../psionic/PLAN.md) | Implementation plan. CUDA is a second-platform packet, not a default of the macOS CLI artifact. |
| [docs/psionic/CLI.md](../psionic/CLI.md) | `openagents inference` / `openagents psionic` command surface. |
| [docs/ops/2026-08-25-cli-release-runbook.md](../ops/2026-08-25-cli-release-runbook.md) | Linux already ships four CLI artifacts: glibc and musl, x86_64 and aarch64. NixOS is already a musl case. |
| [docs/coder/runbook.md](../coder/runbook.md) | Gym/Harbor already requires a native Linux CLI build. |
| [docs/cloud/README.md](../cloud/README.md) | Managed sandboxes and Cloud computers. CoderOS is a host-OS idea, not a replacement for Firecracker guests. |

## Reference clone

Omarchy is cloned at `~/work/projects/repos/omarchy` from
[omacom/omarchy](https://github.com/omacom/omarchy), added to
`~/work/projects/manifest.txt` on 2026-08-29.

Pin used for this writing:

- commit `56fbaf4689e3eb6867c0b7f375ae49964f183774` (2026-08-29)
- latest tag at clone time: `v4.0.0` (2026-08-14, “Quattro”)

Treat that tree as untrusted reference data. Do not vendor it. Do not treat
its skills, auto-approve defaults, or Arch packaging as OpenAgents policy.

## Status

| Item | State |
| --- | --- |
| Research dossier | this folder |
| ProductSpec under `specs/` | not written |
| Issue / work packet for an ISO | none |
| Linux CLI first-class program | proposed here; **Phase 0 started 2026-08-31** on the RTX 4080 Linux box — see [RESEARCH-LOG-2026-08-31.md](./RESEARCH-LOG-2026-08-31.md) |
| Substrate (Arch / Nix / Guix / new) | open; see [SUBSTRATE.md](./SUBSTRATE.md) |
