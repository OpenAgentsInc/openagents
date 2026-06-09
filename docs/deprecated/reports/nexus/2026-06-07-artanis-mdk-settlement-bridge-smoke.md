# Artanis To MDK Settlement Bridge Smoke

Date: 2026-06-07
Issue: `OpenAgentsInc/openagents#4553`

## Result

A direct Artanis/Pylon/MDK settlement bridge smoke completed with real bitcoin
movement.

Public-safe receipt:

```text
docs/reports/nexus/artanis-mdk-settlement-bridge-smoke-20260607195330.json
```

Status:

```text
completed
```

This smoke is stronger than the earlier v0.2.2 release-gate bundle because the
same generated assignment id is present in the settlement plan, public receipt,
Artanis run reference, accepted-work reference, and MDK payment description
digest.

## Evidence Summary

| Field | Value |
| --- | --- |
| Assignment id | `artanis-mdk-bridge-3dc3347ae51ee6856652a567` |
| Settlement intent id | `settlement-3dc3347ae51ee6856652a56746034206` |
| Receipt id | `receipt-3dc3347ae51ee6856652a56746034206` |
| Artanis run | `artanis.bootstrap.pylon-launch.20260607141825` |
| Training run | `run.cs336.a1.starter.20260607183442.404ae7ea` |
| Window | `window.cs336.a1.starter.20260607183442.404ae7ea.0001` |
| Closeout | `rewarded` |
| Accepted contributions | `1` |
| Pylon package | `@openagentsinc/pylon@0.2.2` |
| Release tag | `pylon-v0.2.2` |
| Install method | `release_asset` |
| Install target | `darwin/arm64` |
| Receiver wallet runtime | `moneydevkit` |
| Payment amount | `21` bitcoin sats |
| Payer balance movement | `6658 -> 6637` bitcoin sats |
| Receiver payment count after | `1` |

Public-safe payment digests:

```text
description_digest: sha256:914736a755058f77b6b543ac033958fd6399d3525be6999c4a309b740b3c50ef
invoice_digest: sha256:4edc67de41709bc1d05db17141e1298b82e9d397c12bda247f30782fa2bee2b7
payment_id_digest: sha256:2efdc0af1db6c685a37206c6bf4da66cb08a444475d4d70808cbb9dabdb24417
payment_hash_digest: sha256:2efdc0af1db6c685a37206c6bf4da66cb08a444475d4d70808cbb9dabdb24417
```

Raw invoices, payment ids, payment hashes, preimages, wallet config, and wallet
state remain in ignored private artifacts and are not committed.

## Command

The smoke command is:

```bash
TIMESTAMP=20260607195330 \
MDK_PAYER_HOME=<funded-mdk-agent-wallet-home> \
MDK_PAYER_PORT=3462 \
RAW_ARTIFACT_DIR=<ignored-private-artifact-dir> \
OUTPUT_PATH=docs/reports/nexus/artanis-mdk-settlement-bridge-smoke-20260607195330.json \
scripts/nexus/artanis-mdk-settlement-bridge-smoke.sh
```

The script:

1. reads the current Artanis bootstrap proof and SHC accepted-work proof;
2. creates a settlement plan containing Artanis run id, training run id,
   window id, generated assignment id, settlement intent id, and receipt id;
3. installs `@openagentsinc/pylon@0.2.2` through npm and resolves the public
   `pylon-v0.2.2` release asset;
4. uses that public Pylon binary to create a Pylon-scoped MDK receive invoice;
5. pays the invoice from the funded local MDK payer wallet;
6. polls the Pylon receiver wallet until it observes the incoming payment;
7. writes a public-safe receipt JSON with ids and digests only.

## Remaining Production Work

This smoke proves the bridge shape and a live payment, but it is still an
operator smoke. The remaining production work for #4553 is to move this
authority/idempotency bridge into the deployed production path so a real
Artanis assignment creates the settlement intent, Pylon work acceptance updates
it, and the MDK payment/settlement record emits the public receipt without an
operator manually assembling the proof.

Do not claim fully automated production Artanis paid-work settlement until that
server-side path is deployed and a fresh live proof records the same id chain
without manual bridge assembly.

## 2026-06-07 Follow-Up

Pylon now carries optional public-safe Artanis settlement authority ids through
the retained provider ledger and public report projections:

- `artanis_run_id`
- `artanis_assignment_id`
- `settlement_intent_id`

Those fields are available on retained provider jobs, provider settlements,
provider receipt summaries, and Pylon core receipt projections when present.
They are omitted when absent so older ledger files and existing NIP-90 jobs
remain compatible.

This is not the full production bridge. It is the first data-model step needed
for the deployed path to prove that one Artanis assignment id is the source of
truth for a Pylon accepted-work result, MDK settlement, and public receipt.

## 2026-06-07 Follow-Up 2

Pylon now also carries those authority ids through the live NIP-90 request path.

Structured buyer/Artanis submissions can include:

```json
{
  "artanis_run_id": "artanis.bootstrap.pylon-launch.test",
  "artanis_assignment_id": "assignment-json-001",
  "settlement_intent_id": "settlement-intent-json-001"
}
```

Pylon validates those values as short public-safe ids, appends them to the
published NIP-90 job request as explicit OpenAgents tags, and ignores invalid
or unknown metadata. Provider intake accepts the following exact tag names:

- `oa:artanis_run_id`
- `oa:artanis_assignment_id`
- `oa:settlement_intent_id`

The same values now flow through provider scan output, provider run output,
retained provider jobs, provider settlements, retained job reports, and receipt
reports. The paid-provider regression exercises the real local run loop:
payment-required first, wallet payment observed second, then settled provider
job and receipt projection with the same authority ids.

Verification:

```bash
cargo check -p pylon -p pylon-core -p openagents-provider-substrate
cargo test -p pylon submit_buyer_job_accepts_structured_payload_json -- --nocapture
cargo test -p pylon provider_run_settles_paid_request_and_projects_retained_views -- --nocapture
```

Remaining gap for #4553: this is still a Pylon-side runtime bridge. The final
production closure needs Artanis itself to create/authorize assignments with
these ids in deployed operation and a fresh live proof showing the same id
chain across Artanis dispatch, Pylon accepted work, MDK settlement, and the
public receipt without operator assembly.

## 2026-06-07 Follow-Up 3

The private Cloud Artanis bootstrap contract/control path now has the matching
source-of-truth metadata hook.

Cloud commit:

```text
OpenAgentsInc/cloud d0f84e4 Attach Artanis settlement intent metadata
```

That change adds optional no-wallet `settlement_intent` metadata to
`openagents.artanis_bootstrap_assignment.v1`:

- `artanis_run_id`
- `artanis_assignment_id`
- `settlement_intent_id`
- optional `public_receipt_id`

The Cloud contract validates those values as public-safe contract refs and
requires `settlement_intent.artanis_run_id` to equal `bootstrap_run_id`.
`oa-codex-control` now emits `artanis.settlement_intent.attached` when the
intent is present and includes the exact Pylon structured-request fields and
`oa:*` tag mapping in the generated Artanis prompt. The workroom still has
`wallet_authority=false`; the settlement intent is traceability metadata only.

Cloud verification:

```bash
cargo test -p openagents-cloud-contract -- --nocapture
cargo test -p oa-codex-control artanis_bootstrap -- --nocapture
```

Remaining gap for #4553: deploy/run a fresh Artanis bootstrap assignment using
the new Cloud contract, then prove the same id chain appears in the Artanis
dispatch event, Pylon NIP-90 accepted-work/settled run, MDK settlement, and
public receipt. Until that proof exists, the production claim remains
intentionally blocked.
