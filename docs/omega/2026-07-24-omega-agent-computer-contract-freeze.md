# Omega Agent Computer contract freeze

- Date: 2026-07-24
- Class: contract freeze
- Packet: `OMEGA-AC-00`
- Omega issue: [OpenAgentsInc/omega#27](https://github.com/OpenAgentsInc/omega/issues/27)
- OpenAgents pin: `483fc30305f1dcc3debb611241005015c6ede752`
- Plan: [2026-07-24-agent-computer-omega-completion-plan.md](./2026-07-24-agent-computer-omega-completion-plan.md)
- Roadmap home: `docs/omega/ROADMAP.md` §7.8 `OMEGA-OA-08`
- STE issue: 9
- Glossary revision: `openagents-ste-glossary-v1`
- Status: admitted for Omega Agent Computer implementation packets

## 1. Purpose

This freeze locks how Omega consumes OpenAgents Agent Computer capacity.
Later packets must not invent a second cloud control plane.
Later packets must not put Agent Computer receipt authority in GPUI or Rust
placement code.

This freeze does not start an `omega-effectd` cloud runner.
This freeze does not start a GPUI launch surface.
This freeze does not admit a release.

## 2. Authority chain

| Role | Artifact | Pin / note |
| --- | --- | --- |
| Completion plan | `docs/omega/2026-07-24-agent-computer-omega-completion-plan.md` | OpenAgents pin below |
| Roadmap packet | `docs/omega/ROADMAP.md` §7.8 `OMEGA-OA-08` | portable execution |
| Harness environment | `@openagentsinc/agent-harness-environment` | `0.1.0-rc.1` (HE-02 npm) |
| Full Auto freeze | `docs/omega/2026-07-24-full-auto-contract-freeze.md` | separate product |
| Shared runtime prerequisite | roadmap `OMEGA-OA-01` | required before AC-01 |

## 3. Digests Omega must consume

All digests are SHA-256 of the exact bytes at OpenAgents pin
`483fc30305f1dcc3debb611241005015c6ede752`.

| Path | SHA-256 |
| --- | --- |
| `docs/omega/2026-07-24-agent-computer-omega-completion-plan.md` | `c477263a3cac1ece4617f152c31d5fb2e0f2b53225208793c1304e35212f8275` |
| `docs/omega/ROADMAP.md` | `4629c409e1a2a210caac762eb58bce93325a8b8b78db1f7d1c8780f7b186e74b` |
| `packages/harness-environment/package.json` | `b00ae668c2649b4010ee80620c81156e99827c51d2d1686348cb661a5cb003a3` |
| `packages/harness-environment/src/contract.ts` | `6de72fe5b4bb3039330be03f1ff115e91000581900d5c5f381ed05cbc6f99fa2` |
| `packages/harness-environment/src/openagents-cloud-runner.ts` | `a085fdfe73b23ec423ae10ac5a9450839b35c2647fca6fb9c25034a6dc3db28e` |
| `docs/omega/2026-07-24-full-auto-contract-freeze.md` | `f2fa10b79392cde2d20a47bba1fab1edea894e26962a1b666138c9c05ac05156` |

A later OpenAgents commit may replace these digests only with a new freeze
revision and an explicit Omega issue note.

## 4. Product laws

### 4.1 Environment identity

- Agent Computer for Omega is exactly `HarnessEnvironment.openagents_cloud`.
- Omega must not invent a parallel environment tag for the same capacity.
- Managed sandbox and owner Pylon lanes remain distinct typed environments.

### 4.2 Control-plane ownership

- OpenAgents owns Firecracker, GCE, placement, and guest image truth.
- Omega never calls `oa-codex-control`, GCE, or Firecracker APIs for this path.
- Omega never owns a second placement or guest-image authority.

### 4.3 Mutation path

- `omega-effectd` is the only Omega mutation path to cloud coding sessions.
- Rust supervises process life, health, restart, and generation fencing.
- Rust does not become Agent Computer receipt authority.
- GPUI is projection and command entry only.
- GPUI must not store a second durable cloud-session or receipt ledger.

### 4.4 Capacity and credentials

- Live-capacity probes are mandatory before dispatch.
- Stale slot advertisements are not capacity proof.
- Provider and harness credentials stay runtime-only.
- No baked provider keys enter the guest image through an Omega packet.

### 4.5 Relation to Full Auto

- Full Auto may later add a cloud lane only after `OMEGA-AC-03` proof and a
  Full Auto freeze revision that admits it.
- This freeze does not make Full Auto depend on Agent Computer.
- This freeze does not expand Full Auto lifecycle or active-run limits.

### 4.6 Shared runtime prerequisite

- Implementation packets `OMEGA-AC-01` through `OMEGA-AC-03` require the shared
  runtime seam (`OMEGA-OA-01`) or an explicit owner staging note that names the
  temporary substitute.
- Prefer the released harness-environment artifact (HE-02) over monorepo
  relative imports. HE-02 published
  `@openagentsinc/agent-harness-environment@0.1.0-rc.1`
  (tarball SHA-256
  `9ed2d1c2439dfd33f736b2d3f63795144f7ffb9ad0ce8965f49cc78cd44334fd`).
  See [2026-07-24-he02-harness-environment-release.md](./2026-07-24-he02-harness-environment-release.md).

## 5. Non-goals

This freeze does not:

- qualify the seventh Agent Computer harness
- close openagents `#9190` or `#9193`
- implement `omega-effectd` cloud methods
- add a GPUI Agent Computer panel
- admit public claims or Desktop primary cutover

## 6. Later packet rules

| Packet | Allowed | Forbidden |
| --- | --- | --- |
| `OMEGA-AC-01` | `openagents_cloud` runner inside `omega-effectd` | Direct Rust placement or GCE calls |
| `OMEGA-AC-02` | One bounded projection/command surface | Second durable cloud thread store |
| `OMEGA-AC-03` | Live Firecracker turn proof from Omega | Mock-only closeout |

## 7. Falsifier

A GPUI view or Rust crate becomes Agent Computer receipt authority, or Omega
owns Firecracker, GCE, or placement for this product path.

## 8. Admission

This freeze is admitted for Omega Agent Computer implementation packets when it
is on `main` and linked from Omega issue `#27`.
Owner acceptance of the plan ledger and this freeze text is the authority for
later OMEGA-AC packets.
Independent assurance for live capacity remains a later proof gate
(`OMEGA-AC-03`), not this freeze.
