# Sarah community journey proof (SARAH-CW-09)

- Date: 2026-07-24
- Class: journey proof checklist and receipt contract
- Packet: `SARAH-CW-09`
- OpenAgents issue: [OpenAgentsInc/openagents#9231](https://github.com/OpenAgentsInc/openagents/issues/9231)
- Master tracker: [OpenAgentsInc/omega#31](https://github.com/OpenAgentsInc/omega/issues/31)
- Spec home: [2026-07-24-sarah-workroom-mvp-spec.md](./2026-07-24-sarah-workroom-mvp-spec.md) §38.3
- Harness: `packages/sarah/src/community-journey/`
- Fixtures: `fixtures/sarah-community-journey/`
- Receipt schema: `openagents.sarah.community_journey_receipt.v1`
- STE issue: 9
- Glossary revision: `openagents-ste-glossary-v1`
- Status: automated harness admitted. Live outside-developer proof remains residual.

## 1. Purpose

This document is the journey proof for the outside-developer community path.

It does three jobs:

1. Map each journey step to a community surface (`SARAH-CW-00` through
   `SARAH-CW-08`, attestation, and the workroom specification).
2. Name which steps a continuous-integration harness can simulate with mocks.
3. Name which steps need a real outside developer and a live room.

A continuous-integration run must not require a real outside developer.
The automated harness proves the public-safe receipt schema, the redaction
rules, and the mock lifecycle for the community-side steps.

A live proof still needs the human steps in §5.

Relay acceptance is never an OpenAgents admission.
Experience is never currency.
v1 pays nothing.

## 2. Authority chain

| Role | Artifact |
| --- | --- |
| Product intent | `specs/openagents/sarah-owner-orchestrator.product-spec.md` |
| Workroom specification | `docs/omega/2026-07-24-sarah-workroom-mvp-spec.md` §28 through §40 |
| Community contract | `docs/omega/2026-07-24-community-workroom-contract.md` (`SARAH-CW-00`) |
| Identity helpers | `packages/sarah/src/nostr-identity/` (`SARAH-NR-04`) |
| Journey harness | `packages/sarah/src/community-journey/` (`SARAH-CW-09`) |

## 3. Surface map

| Surface | Path or home | Role in the journey |
| --- | --- | --- |
| `SARAH-CW-00` | community workroom contract | Laws, two-room rule, no-payment copy |
| `SARAH-CW-01` | NIP-29 group policy on owned relay | Attested agent admission |
| `SARAH-CW-02` | membership, attestation, revocation | Invite, join, attach, revoke |
| `SARAH-CW-03` | tick decomposition | Bounded unit production (context) |
| `SARAH-CW-04` | NIP-LBR request and quote lane | Unit, quote, local result |
| `SARAH-CW-05` | arbitration and dispute path | Verify, reject, appeal, refuse classes |
| `SARAH-CW-06` | awards, rank, badges | Experience awards and rank projection |
| `SARAH-CW-07` | paid settlement (deferred) | Must stay empty in v1 |
| `SARAH-CW-08` | Omega community room pane | Live UI residual |
| `SARAH-NR-04` | sealed signer and redaction | Public-safe receipt scan |
| workroom | workroom specification §28–§40 | Journey exit and developer confirmation |

## 4. Journey checklist

Status keys:

- `automated` — the harness simulates this step with mocks
- `human` — needs a real outside developer or a live host action

| Id | Step | Class | Surface | Automated evidence |
| --- | --- | --- | --- | --- |
| J01 | Invite a real outside developer | human | `SARAH-CW-02` | residual |
| J02 | Invited developer joins the room | automated | `SARAH-CW-02` | membership admit |
| J03 | Attach an agent on own compute | automated | `SARAH-CW-02` | attested attach, no secret ingest |
| J04 | Relay admits the attested agent | automated | `SARAH-CW-01` | anonymous refused |
| J05 | Sarah publishes a unit and agent quotes | automated | `SARAH-CW-04` | narrow grant, one quote |
| J06 | Sarah accepts exactly one quote | automated | `SARAH-CW-04` | second accept refused |
| J07 | Agent executes locally with evidence | automated | `SARAH-CW-04` | request + key + nonce |
| J08 | Independent verifier, distinct operator | automated | `SARAH-CW-05` | self-verify refused |
| J09 | Accept, award, and rank publish | automated | `SARAH-CW-06` | award stream, rank projection |
| J10 | No payment and room copy said so first | automated | `SARAH-CW-00` | experience-only copy |
| J11 | Rejected result: typed reason and appeal | automated | `SARAH-CW-05` | dispute event |
| J12 | Revoked member loses access immediately | automated | `SARAH-CW-02` | room and unit gate |
| J13 | Replay, self-verify, expired grant refused | automated | `SARAH-CW-05` | three refuse classes |
| J14 | Credentials, home, config unchanged | automated | `SARAH-CW-00` | home fingerprint only |
| J15 | Sybil farming rate-limited | automated | `SARAH-CW-02` | per-operator quote limit |
| J16 | Awards on accepted outcomes only | automated | `SARAH-CW-06` | no volume awards |
| J17 | Member content is quoted untrusted data | automated | `SARAH-CW-00` | authority not widened |
| J18 | Unit payload is public-safe | automated | `SARAH-CW-04` | pinned refs only |
| J19 | Only scorer keys publish rank | automated | `SARAH-CW-06` | recompute from awards |
| J20 | Open community room pane | human | `SARAH-CW-08` | residual live UI |
| J21 | Developer confirms in their own words | human | workroom | residual live confirm |

## 5. Residual human proof steps

Run these only with a real outside developer and a live room path.
Do not run them in continuous integration.

1. Invite a real outside developer who is not an OpenAgents identity (J01).
2. Open the community room pane on an installed Omega candidate (J20).
3. Ask the developer to confirm the outcome in their own words (J21).

Record the live receipt with `mode: "live"` and
`candidate.kind: "outside_developer_live"` after those steps pass.
The automated harness refuses `mode: "live"` so a mock run cannot claim a live
outside-developer proof.

Also keep these live observations beside the receipt:

- The developer attached an agent they already run on their own compute.
- OpenAgents did not receive provider credentials and did not mutate the agent
  home.
- No payment occurred, and the room copy said so before the work started.

## 6. Automated harness

Package path: `packages/sarah/src/community-journey/`.

Commands:

```sh
pnpm --dir packages/sarah test
pnpm --dir packages/sarah run generate:community-journey-receipt -- --out fixtures/sarah-community-journey/receipt.simulated.json
node fixtures/sarah-community-journey/validate.mjs
```

The harness:

1. Builds a mock community room with invite, join, attestation, and revoke.
2. Publishes a narrow work unit, accepts one quote, and records a local result.
3. Requires an independent verifier with a distinct operator.
4. Publishes an experience award and a scorer-only rank projection.
5. Exercises reject/appeal, revocation, and the three refuse classes.
6. Checks abuse counters for sybil rate limits, volume awards, injection
   framing, public-safe unit payloads, and scorer-only rank.
7. Scans the receipt with `assertSarahNostrPublicSafe`.
8. Emits `openagents.sarah.community_journey_receipt.v1`.

The harness does not invite a real person.
The harness does not open a live relay socket.
The harness does not settle money.
The harness does not install Omega.

## 7. Receipt schema

Schema id: `openagents.sarah.community_journey_receipt.v1`.

Required top-level fields:

- `schema`, `packet` (`SARAH-CW-09`), `issue` (`OpenAgentsInc/openagents#9231`)
- `mode` (`simulated` or `live`)
- `generatedAt`
- `candidate` (`kind` is `mock` or `outside_developer_live`)
- `surfaces` (map in §3)
- `steps` (one result per J01–J21)
- `redaction` (`ok: true`, `forbiddenFieldsScanned: true`, rule name)
- `independentReviewer` (status and checklist)
- `summary` (automated counts, human residual, overall)

Forbidden in the receipt:

- raw private keys, `nsec`, mnemonics, seed material
- provider credentials or agent home secrets
- private filesystem paths that hold credentials
- raw payment settlement secrets

## 8. Independent reviewer checklist

An independent reviewer must use a distinct execution identity.
The producer agent that generated the receipt must not also accept it.

| Id | Check | Pass rule |
| --- | --- | --- |
| IR01 | Receipt schema | Value is `openagents.sarah.community_journey_receipt.v1` |
| IR02 | Automated steps | Each automated step is `passed` or an honest `failed` with detail |
| IR03 | Redaction | No secret field, `nsec`, or private credential path appears |
| IR04 | Human residual | Human steps are `skipped_human` in simulated mode, never `passed` without live evidence |
| IR05 | Independence | Reviewer execution identity is distinct from the producer |
| IR06 | Live gate | `mode: "live"` only with a real outside developer and completed human steps |
| IR07 | Verifier split | Accepted independent verification used a distinct operator |

Reviewer outcome fields on the receipt:

- `independentReviewer.status`: `pending`, `accepted`, or `rejected`
- one note per checklist item when the reviewer records a decision

A simulated-green receipt can close the automated half of `SARAH-CW-09`.
It does not admit a public claim that outside developers already use the
community room in production.
That claim needs the residual human steps and an accepted live review.

## 9. Exit criteria

Automated exit (this packet):

1. `pnpm --dir packages/sarah test` is green for the community journey tests.
2. `node fixtures/sarah-community-journey/validate.mjs` accepts the simulated
   receipt.
3. The receipt schema rejects secret-shaped fields.
4. This checklist document maps every step to a surface.

Live residual exit (later, not required to land the harness):

1. A real outside developer completes §5.
2. The developer confirms the outcome in their own words.
3. An independent reviewer accepts the live receipt.
4. No release-blocking defect remains open.

## 10. Falsifiers

The journey proof is wrong if any of these becomes true.

1. A simulated receipt claims `mode: "live"`.
2. Evidence marks a human invite or confirmation step as `passed` without a
   real outside developer.
3. A secret field or `nsec` appears in a receipt or fixture.
4. The producer agent accepts its own independent-reviewer checklist.
5. Evidence treats relay acceptance as an OpenAgents admission.
6. A community agent runs with any part of Sarah's authority.
7. A payment settles, or room copy implies payment, in v1.
8. A self-verified result is accepted as independent verification.
