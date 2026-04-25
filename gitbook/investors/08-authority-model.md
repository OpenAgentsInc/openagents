[Home](../README.md) · [Investor Path](README.md) · **08. Authority & Ownership**

# 8. Authority & Ownership

> _"The app must never 'feel like it paid you' unless it actually did. The architecture exists to enforce that honesty."_
>
> — [`docs/MVP.md`, OpenAgentsInc/openagents](https://github.com/OpenAgentsInc/openagents/blob/main/docs/MVP.md)

**You will learn:**

- [ADR-0001](https://github.com/OpenAgentsInc/openagents/blob/main/docs/adr/0001-authority-boundaries.md)'s domain-scoped authority matrix
- Why Spacetime is **not** a money authority
- How TreasuryRouter and the Kernel Authority API stay separate from the desktop

## The non-negotiable invariant

Authority — the power to change economic truth — is kept out of the client. Every money-moving, settlement-finalizing, or verdict-closing action routes through **TreasuryRouter** and the **Kernel Authority API**, both backend services. That is the single non-negotiable invariant of the OpenAgents stack.

From [`docs/MVP.md`](https://github.com/OpenAgentsInc/openagents/blob/main/docs/MVP.md):

> _"This MVP is a money-moving, network-participating desktop app. That means the system's guarantees matter. There are a few invariants we will not violate because they protect determinism, safety, and our ability to evolve without breaking users."_

## The domain-scoped authority matrix

From [`docs/MVP.md`](https://github.com/OpenAgentsInc/openagents/blob/main/docs/MVP.md):

| Domain                                   | Authority Owner                    | Spacetime reducer authority |
| ---------------------------------------- | ---------------------------------- | --------------------------- |
| Money / settlement / wallet truth        | authenticated command lanes        | **no**                      |
| Trust / policy / security verdicts       | authenticated command lanes        | **no**                      |
| Provider / device online presence        | Spacetime presence reducers        | yes                         |
| Replay checkpoints / cursor continuity   | Spacetime checkpoint reducers      | yes                         |
| Non-monetary projections / counters      | Spacetime projection reducers/queries | yes                      |

Canonical ADR: [`docs/adr/ADR-0001-spacetime-domain-authority-matrix.md`](https://github.com/OpenAgentsInc/openagents/blob/main/docs/adr/ADR-0001-spacetime-domain-authority-matrix.md).

The matrix is the whole game. If money or policy verdicts are at stake, the write has to come through an authenticated command lane; Spacetime cannot mutate those domains no matter how convenient it would be. For presence, projections, and checkpoints — domains where consensus speeds up the UX but cannot cause monetary loss if they drift — Spacetime reducers are authoritative precisely because they _can't_ cost you money.

## Why this split exists

From [`README.md`](https://github.com/OpenAgentsInc/openagents/blob/main/README.md):

> _"This separation is intentional:_
>
> - _local runtime executes work_
> - _backend authority mutates economic truth_
> - _coordination channels project progress_
> - _receipts provide the canonical audit trail"_

Four surfaces, four responsibilities, one kernel behind them all. No one surface can corrupt another's truth.

## Desktop ownership boundaries

The full module-ownership map is [`docs/OWNERSHIP.md`](https://github.com/OpenAgentsInc/openagents/blob/main/docs/OWNERSHIP.md). A few highlights directly relevant to the investor read:

- **`apps/autopilot-deprecated`** — current WGPUI desktop surface + embedded runtime for MVP execution. This is where most of the current shipped product lives.
- **`apps/autopilot`** — new Tauri shell scaffold for the next product direction. _"Do not call that app `autopilot-tauri`. The product name is `Autopilot`; Tauri is the desktop shell implementation."_ ([`README.md`](https://github.com/OpenAgentsInc/openagents/blob/main/README.md))
- **`apps/nexus-control`** — the current in-repo backend authority slice; hosts retained mutation and projection entry points the desktop calls.
- **`openagents-kernel-core`** — the kernel authority crate (normative spec in [`docs/kernel/economy-kernel.md`](https://github.com/OpenAgentsInc/openagents/blob/main/docs/kernel/economy-kernel.md)).

From [`docs/MVP.md`](https://github.com/OpenAgentsInc/openagents/blob/main/docs/MVP.md):

> _"The retained implementation is Rust-only. We do not ship a split-brain authority system. Cross-boundary contracts are proto-first. The desktop app and services talk in typed, versioned contracts."_

Rust for authority-bearing code. Typed proto contracts across every boundary. No split-brain.

## The default backend stack (Nexus)

Nexus is the OpenAgents-hosted, open-source, self-hostable server-authority role. It is the desktop's default endpoint for:

- `POST /api/sync/token` token issuance
- auth / session flows
- public stats
- primary Nostr relay / index path
- starter-job dispatch for the CS336 A1 homework lane

Users and organizations can replace the default with their own Nexus deployment, including pointing Autopilot at their own Nexus with its own relay set. Starter-job guarantees are OpenAgents-hosted for now; self-hosted Nexus is free to run the same software but does not automatically inherit the subsidy-backed demand.

## Why the starter-job lane is gated

From [`docs/MVP.md`](https://github.com/OpenAgentsInc/openagents/blob/main/docs/MVP.md):

> _"Eligibility for those starter jobs should be enforced from Nexus-side proof, not from a user-supplied `client` tag alone. The practical near-term rule is: if the OpenAgents-hosted Nexus can prove the provider is connected through an authenticated Autopilot session with bound Nostr identity, that is sufficient for MVP."_

Self-reported `client` tags are a Nostr-level convention; they can be spoofed. The starter-job lane is gated on _authenticated_ Autopilot sessions instead, so the subsidy can't be siphoned by anonymous spoofers. Stronger anti-spoofing (device-bound proofs, richer attestation) is hardening work on the roadmap.

## Why Spacetime is not a money authority

From [`docs/MVP.md`](https://github.com/OpenAgentsInc/openagents/blob/main/docs/MVP.md):

> _"Sync/replay remains non-authoritative by default, with one explicit exception class: ADR-approved app-db domains (presence/checkpoints/projections) may be Spacetime-authoritative."_

Spacetime is a live sync substrate — very good at distributed presence, cursor continuity, and projection. It is _not_ where verdicts are finalized. The explicit exception class exists so we can use Spacetime authoritatively where being wrong is cheap (was the device online? is the cursor caught up?) and never where being wrong is expensive (did the wallet receive 25 sats?).

## What this means for investor diligence

Three things follow from the authority model:

1. **The audit trail lives in receipts**, not in UI state. Every settled payout in `docs/reports/nexus/*.json` is a kernel-signed receipt, not a UI screenshot.
2. **The client is untrusted by design**. A malicious or broken desktop cannot create monetary gain — only the kernel can, and the kernel demands authenticated command lanes.
3. **Self-hosting is possible today**. The desktop can be pointed at a user-owned Nexus + relay set. OpenAgents does not sit in the middle of authorized work; it is the buyer of first resort _on_ the open network.

---

**← Previous:** [07. Economy Kernel](07-economy-kernel.md) · **Next:** [09. Proof Receipts](09-proof-receipts.md) **→**
