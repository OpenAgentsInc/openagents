# Sarah Nostr journey proof (SARAH-NR-09)

- Date: 2026-07-24
- Class: journey proof checklist and receipt contract
- Packet: `SARAH-NR-09`
- OpenAgents issue: [OpenAgentsInc/openagents#9223](https://github.com/OpenAgentsInc/openagents/issues/9223)
- Master tracker: [OpenAgentsInc/omega#31](https://github.com/OpenAgentsInc/omega/issues/31)
- Absorbs: `OMEGA-SW-07`
- Spec home: [2026-07-24-sarah-workroom-mvp-spec.md](./2026-07-24-sarah-workroom-mvp-spec.md) §9.8 and §24.10
- Harness: `packages/sarah/src/nostr-journey/`
- Fixtures: `fixtures/sarah-nostr-journey/`
- Receipt schema: `openagents.sarah.nostr_journey_receipt.v1`
- STE issue: 9
- Glossary revision: `openagents-ste-glossary-v1`
- Status: automated harness admitted. Live install proof remains residual.

## 1. Purpose

This document is the journey proof for the Nostr-backed Sarah path.

It does three jobs:

1. Map each journey step to an existing surface (`SARAH-NR-04` through
   `SARAH-NR-08`, Omega workroom packets, and the record contract).
2. Name which steps a continuous-integration harness can simulate with mocks.
3. Name which steps need a signed Omega install and a human operator.

A continuous-integration run must not require a signed Omega install.
The automated harness proves the public-safe receipt schema, the redaction
rules, and the mock ladder for the Nostr-side steps.

A live owner proof still needs the human steps in §5.

Relay acceptance is never an OpenAgents admission.

## 2. Authority chain

| Role | Artifact |
| --- | --- |
| Product intent | `specs/openagents/sarah-owner-orchestrator.product-spec.md` |
| Workroom specification | `docs/omega/2026-07-24-sarah-workroom-mvp-spec.md` §9.8 and §24.10 |
| Record contract | `docs/omega/2026-07-24-sarah-nostr-record-contract.md` (`SARAH-NR-00`) |
| Identity contract | `docs/omega/2026-07-24-sarah-nostr-identity-contract.md` (`SARAH-NR-04`) |
| Turn service | `packages/sarah/src/nostr-turn/` (`SARAH-NR-05`) |
| Journey harness | `packages/sarah/src/nostr-journey/` (`SARAH-NR-09`) |

## 3. Surface map

| Surface | Path or home | Role in the journey |
| --- | --- | --- |
| `SARAH-NR-00` | `docs/omega/2026-07-24-sarah-nostr-record-contract.md`, `fixtures/sarah-nostr-record/` | Encrypted durable kinds, causal tags, operator-blind content |
| `SARAH-NR-04` | `packages/sarah/src/nostr-identity/` | Sealed signer, attested AUTH, public-safe redaction |
| `SARAH-NR-05` | `packages/sarah/src/nostr-turn/` | Claim, live ladder, durable ladder, interrupt, usage metric |
| `SARAH-NR-06` | Omega workroom Nostr client (planned in Omega) | Pane read/write, second relay, offline publish |
| `SARAH-NR-07` | memory / NIP-AE / NIP-RS / NIP-ER (planned) | Memory and read-state after cutover |
| `SARAH-NR-08` | migration shadow-cutover-retirement (planned) | Record cutover, not required for mock ladder |
| `OMEGA-SW-01` | Omega identity bind | Account bind from a clean profile |
| `OMEGA-SW-03` | Omega workroom pane | Open pane, release questions, network degrade UI |
| `OMEGA-SW-05` | receipt inspector | Full Auto pending state and authority receipts |
| `OMEGA-SW-07` | this journey (absorbed) | Install, kill `omega-effectd`, remove Omega |
| workroom | `docs/omega/2026-07-24-sarah-workroom-mvp-spec.md` | Principal, conversation, and authority revision |

## 4. Journey checklist

Status keys:

- `automated` — the harness simulates this step with mocks
- `human` — needs a signed Omega candidate or a live host action

| Id | Step | Class | Surface | Automated evidence |
| --- | --- | --- | --- | --- |
| J01 | Install the candidate from a clean profile | human | `OMEGA-SW-07` | residual |
| J02 | Bind the Omega identity to the OpenAgents account | human | `OMEGA-SW-01` | residual |
| J03 | Open the workroom pane | human | `OMEGA-SW-03` | residual |
| J04 | Confirm principal, conversation, and authority revision | automated | workroom | projection mock |
| J05 | Sarah authenticates to the owned relay with an attested key | automated | `SARAH-NR-04` | NIP-OA tag + NIP-42 AUTH |
| J06 | Owner sends a message only owner and Sarah can decrypt | automated | `SARAH-NR-00` | ciphertext on the wire |
| J07 | Relay operator cannot read conversation content | automated | `SARAH-NR-00` | no plaintext JSON |
| J08 | Ask Sarah for current release state | human | `OMEGA-SW-03` | residual live tools |
| J09 | Ask for coding capacity and a gap-free live tool ladder | automated | `SARAH-NR-05` | gap-free live and durable ladder |
| J10 | Control an existing Full Auto run and keep pending state | human | `OMEGA-SW-05` | residual host apply |
| J11 | Trigger one refusal with a reserved-category receipt | automated | `SARAH-NR-05` | kind `44301` refusal |
| J12 | Interrupt one turn and confirm the terminal event | automated | `SARAH-NR-05` | cancel + `turn.interrupted` |
| J13 | Restart mid-turn with one honest outcome | automated | `SARAH-NR-05` | unreclaimable terminal claim |
| J14 | Durable ladder replays from relay history alone | automated | `SARAH-NR-05` | memory-relay replay |
| J15 | Kill `omega-effectd` and recover without a duplicate answer | human | `OMEGA-SW-07` | residual process kill |
| J16 | Exact `token_usage_events` row and NIP-AM metric agree | automated | `SARAH-NR-05` | totals match metric body |
| J17 | Second admitted relay serves the same history | automated | `SARAH-NR-06` | mirrored event ids |
| J18 | Offline-signed event publishes after reconnection | automated | `SARAH-NR-06` | pre-sign then publish |
| J19 | Stale, duplicate, unsigned, revoked, unauthorized rejected | automated | `SARAH-NR-05` | five reject classes |
| J20 | Export verifies causal chain without Cloud SQL | automated | `SARAH-NR-00` | parent `e` tags only |
| J21 | Disconnect network and show a visible degraded state | human | `OMEGA-SW-03` | residual UI |
| J22 | No token, credential, or private path in any log | automated | `SARAH-NR-04` | public-safe projection |
| J23 | Remove Omega without change to Zed or Electron data | human | `OMEGA-SW-07` | residual uninstall |

## 5. Residual human proof steps

Run these only on an installed and signed Omega candidate. Do not run them in
continuous integration.

1. Install the candidate from a clean profile (J01).
2. Bind the Omega identity through the loopback OpenAgents flow (J02).
3. Open the workroom pane (J03).
4. Ask Sarah for current release state and read the cited answer (J08).
5. Ask Sarah to control an existing Full Auto run and keep pending until the
   host applies it (J10).
6. Kill `omega-effectd` during a turn and confirm recovery without a second
   answer (J15).
7. Disconnect the network and confirm a visible degraded state, not a hang
   (J21).
8. Remove Omega and confirm Zed and Electron data did not change (J23).

Record the live receipt with `mode: "live"` and
`candidate.kind: "signed_omega"` after those steps pass.
The automated harness refuses `mode: "live"` so a mock run cannot claim a live
install proof.

## 6. Automated harness

Package path: `packages/sarah/src/nostr-journey/`.

Commands:

```sh
pnpm --dir packages/sarah test
pnpm --dir packages/sarah run generate:journey-receipt -- --out fixtures/sarah-nostr-journey/receipt.simulated.json
node fixtures/sarah-nostr-journey/validate.mjs
```

The harness:

1. Builds a sealed Sarah signer and an owner attestation.
2. Publishes attested AUTH, encrypted owner traffic, and a capacity turn.
3. Exercises refusal, interrupt, claim terminal, replay, metric agreement,
   second-relay mirror, offline publish, and bad-input rejects.
4. Scans the receipt with `assertSarahNostrPublicSafe`.
5. Emits `openagents.sarah.nostr_journey_receipt.v1`.

The harness does not mint a production Secret Manager key.
The harness does not open a live relay socket.
The harness does not install Omega.

## 7. Receipt schema

Schema id: `openagents.sarah.nostr_journey_receipt.v1`.

Required top-level fields:

- `schema`, `packet` (`SARAH-NR-09`), `issue` (`OpenAgentsInc/openagents#9223`)
- `mode` (`simulated` or `live`)
- `generatedAt`
- `candidate` (`kind` is `mock` or `signed_omega`)
- `surfaces` (map in §3)
- `steps` (one result per J01–J23)
- `redaction` (`ok: true`, `forbiddenFieldsScanned: true`, rule name)
- `independentReviewer` (status and checklist)
- `summary` (automated counts, human residual, overall)

Forbidden in the receipt:

- raw private keys, `nsec`, mnemonics, seed material
- Secret Manager secret values
- private filesystem paths that hold credentials
- raw owner conversation plaintext

## 8. Independent reviewer checklist

An independent reviewer must use a distinct execution identity.
The producer agent that generated the receipt must not also accept it.

| Id | Check | Pass rule |
| --- | --- | --- |
| IR01 | Receipt schema | Value is `openagents.sarah.nostr_journey_receipt.v1` |
| IR02 | Automated steps | Each automated step is `passed` or an honest `failed` with detail |
| IR03 | Redaction | No secret field, `nsec`, or private credential path appears |
| IR04 | Human residual | Human steps are `skipped_human` in simulated mode, never `passed` without live evidence |
| IR05 | Independence | Reviewer execution identity is distinct from the producer |
| IR06 | Live gate | `mode: "live"` only with a signed Omega candidate ref and completed human steps |

Reviewer outcome fields on the receipt:

- `independentReviewer.status`: `pending`, `accepted`, or `rejected`
- one note per checklist item when the reviewer records a decision

A simulated-green receipt can close the automated half of `SARAH-NR-09`.
It does not admit a daily-use public claim for Nostr-backed Sarah.
That claim needs the residual human steps and an accepted live review.

## 9. Exit criteria

Automated exit (this packet):

1. `pnpm --dir packages/sarah test` is green for the journey tests.
2. `node fixtures/sarah-nostr-journey/validate.mjs` accepts the simulated
   receipt.
3. The receipt schema rejects secret-shaped fields.
4. This checklist document maps every step to a surface.

Live residual exit (later, not required to land the harness):

1. A signed Omega candidate completes §5.
2. An independent reviewer accepts the live receipt.
3. No release-blocking defect remains open.

## 10. Falsifiers

The journey proof is wrong if any of these becomes true.

1. A simulated receipt claims `mode: "live"`.
2. Evidence marks a human install step as `passed` without a signed candidate.
3. A secret field or `nsec` appears in a receipt or fixture.
4. The producer agent accepts its own independent-reviewer checklist.
5. Evidence treats relay acceptance as an OpenAgents admission.
