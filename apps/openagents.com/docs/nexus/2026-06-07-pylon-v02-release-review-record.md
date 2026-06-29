# Pylon v0.2 Release Review Record And Rollback Plan

Date: 2026-06-07
Repository: `OpenAgentsInc/openagents`
Related issues: #489, #490, #491, #492, #493, #499

## Current Decision

State: `ready_for_operator_release_review`

Operator approval state: not approved for general availability.

Release action state: release artifacts are already public, but this record
does not create, publish, or approve a new release action.

Autonomous Artanis state: not approved. The scheduled autonomous runner remains
separately gated.

Network readiness state: `network_not_ready_for_release`.

Superseded by #505 for the package-launcher release decision:
`docs/nexus/2026-06-08-pylon-downloadable-launcher-release-0.2.5.md` records
the current public posture as `limited_launcher_release_shipped` for
`@openagentsinc/pylon@0.2.5`, while preserving native Windows, WSL Ubuntu,
hosted MDK direct payout, unrestricted earning, and autonomous Artanis
production limits.

#499 supersedes any temptation to treat this review record as permission to
cut another package release, move npm `latest`, or tell people to download
Pylon to earn bitcoin. The release-review evidence is useful, but the network
still has to prove the full install, registration, heartbeat, wallet, job,
proof, bitcoin payout, receipt, repeated multi-host smoke, and rollback path.
The canonical freeze checklist is
`docs/nexus/2026-06-07-pylon-network-readiness-release-freeze.md`.

The current safe claim is:

```text
Pylon v0.2.4 release artifacts are public, and OpenAgents product surface's public Nexus/Pylon gate
has complete release-review evidence across package publication, local clean
launcher smoke, two distinct paid-work traces, public settlement receipts, and
Artanis Forum publication. This is ready for operator release review. It is not
a claim that Pylon is generally available for every host, that Artanis is
autonomous, or that release, wallet-spend, provider-mutation, settlement, or
public-claim-upgrade authority has been granted. New releases and broad
download/earning claims remain frozen until the #499 network-readiness
checklist passes.
```

Do not say:

```text
Pylon v0.2 is ready for everyone.
Artanis is autonomously administering Pylon production.
Any agent can rely on paid Pylon work without operator review.
The evidence gate can publish releases or spend bitcoin.
```

## State Vocabulary

| State | Meaning |
| --- | --- |
| `blocked` | Required public-safe evidence is missing, partial, duplicate, non-terminal, or unsafe to project. |
| `ready_for_operator_release_review` | Evidence is complete enough for a human/operator release review, but no release approval or public-claim-upgrade authority is granted by the gate. |
| `network_not_ready_for_release` | A stronger #499 release freeze is active even though review evidence exists; no new release, npm latest move, or broad download/earning claim is allowed until the full network-readiness sequence passes. |
| `approved_for_release` | A retained operator decision explicitly approves the stronger release claim and records rollback posture. This state is not currently recorded. |
| `released` | The approved release action has been performed and verified. Package publication already exists for `0.2.4`, but general availability remains unapproved. |

## Evidence Reviewed

| Evidence class | Public-safe refs |
| --- | --- |
| GitHub release/tag | `OpenAgentsInc/openagents` release `pylon-v0.2.4`. |
| npm package | `@openagentsinc/pylon@0.2.4`, npm `latest = 0.2.4`, integrity `sha512-SXZNpqswgyaeVFrzY9P0Pn4dYy51hWjJBf9cH+z0b83pqHGx74Pp8E9Nzk8KMdH+iLpuZtdGWKAQ3SiY4Kw0bA==`. |
| Local package smoke | #490 local macOS arm64 clean HOME/cache no-launch smoke: `version: 0.2.4`, `tagName: pylon-v0.2.4`, `installMethod: release_asset`, target `darwin/arm64`, desired offline mode. |
| Local status smoke | #490 forwarded `status --json` smoke: runtime mode `offline`, authoritative status `ready`, no runtime error, and two sellable offline launch products. |
| Second-host smoke | Not proven. Local Tailscale daemon access failed, Arch SSH refused, and known macOS Tailnet IPs timed out. Linux, WSL Ubuntu, native Windows, and clean second-host evidence remain future general-availability work. |
| Pylon registration/heartbeat | `api.public.pylon.registration_heartbeat`; `workers/api/src/pylon-api-routes.test.ts`. |
| Wallet readiness | `wallet_readiness.public.bucketed.minimum_satisfied`; #436 bucketed wallet readiness only. |
| First real movement receipt | `receipt.nexus.issue_431.settlement.issue_431_authority_1780818513507`. |
| First Artanis assignment receipt | `receipt.nexus.issue_438.settlement.issue_438_artanis_1780822221`. |
| Second Artanis bridge receipt | `receipt.nexus_pylon.settlement.artanis_mdk_bridge_8b378373002501f3e896dcd3`. |
| Distinct Pylon refs | `pylon.public.artanis.bridge.8b378373`; `pylon.public.issue_438_edge_wallet`. |
| Paid-work proof refs | `artanis-mdk-bridge-8b378373002501f3e896dcd3`; `assignment.public.issue_438.issue_438_artanis_1780822221`; `proof.public.mdk_agent_wallet.real_bitcoin_moved.8b378373002501f3e896dcd3`. |
| Public report | `GET https://openagents.com/api/public/artanis/report`, `pylonOpenAgents product surfaceReleaseGate.state = ready_for_operator_release_review`, two distinct Pylons, no blocker refs, authority flags false. |
| Rendered public page | `https://openagents.com/artanis` renders `OpenAgents product surface release gate`, the evidence-complete label, and `2 / 2 distinct Pylons`. |
| Forum publication | `https://openagents.com/forum/t/88888888-4004-4004-8004-888888888888`, Artanis post #3. |
| Psionic stale-label cleanup | `OpenAgentsInc/psionic@ce3b0e0c` retitled the Qwen legal source docs as a Psionic boundary, not the OpenAgents public Pylon release. |
| Rollback drill evidence | Not yet drilled. This document is the retained rollback plan; drill receipts remain future work. |

## Authority Boundary

This record separates four different things:

- release artifact publication: done for `pylon-v0.2.4`;
- runtime readiness: partially proven through local clean launcher/status smoke;
- paid-work and settlement readiness: proven for the retained public-safe
  evidence set only;
- autonomous Artanis readiness: not proven and not approved.

The public report authority flags remain false:

- `releasePublicationAllowed`;
- `walletSpendAllowed`;
- `settlementMutationAllowed`;
- `providerMutationAllowed`;
- `publicClaimUpgradeAllowed`.

## Rollback Plan

### Bad npm Latest Or Package

Use this when `@openagentsinc/pylon@latest` points at the wrong version, the
tarball is bad, or a fixed patch is safer than leaving the current latest tag.

Verify first:

```bash
npm view @openagentsinc/pylon dist-tags version --json
npm view @openagentsinc/pylon@0.2.4 name version dist.tarball dist.integrity bin --json
```

Move `latest` back to the last known safe version if the package must be
rolled back:

```bash
npm dist-tag add @openagentsinc/pylon@0.2.2 latest
npm view @openagentsinc/pylon dist-tags version --json
```

Prefer publishing a fixed patch, such as `0.2.5`, and moving `latest` to that
patch when the current release is mostly correct but needs a narrow fix.

### Bad GitHub Release Or Tag

Use this when the GitHub release metadata is misleading or the release asset is
known invalid.

Inspect first:

```bash
gh release view pylon-v0.2.4 --repo OpenAgentsInc/openagents
```

Prefer a corrective release note:

```bash
gh release edit pylon-v0.2.4 --repo OpenAgentsInc/openagents --notes-file /tmp/pylon-v0.2.4-correction.md
```

Only delete the release and tag if the artifact is materially invalid and the
operator accepts that public history change:

```bash
gh release delete pylon-v0.2.4 --repo OpenAgentsInc/openagents --cleanup-tag
```

Then rerun the npm and GitHub release evidence checks before posting any new
public status.

### Bad Public Copy Or False Release-Gate Claim

Use this when `/artanis`, `/api/public/artanis/report`, docs, or Forum copy
overstates release, payment, settlement, or autonomy.

1. Patch the source copy or projection.
2. Run the focused local tests when code changed.
3. Deploy through the normal guarded Worker deploy path:

```bash
bun run --cwd workers/api deploy
```

4. Verify:

```bash
curl -fsS https://openagents.com/api/public/artanis/report \
  | jq '.pylonOpenAgents product surfaceReleaseGate'
```

5. Post a correction in the Pylon release work-log topic if the false claim was
publicly visible.

Do not hide a mistaken public claim by deleting the proof trail. Keep the
correction visible and cite the corrected evidence state.

### Bad Forum Post

Use this when an Artanis Forum post contains a wrong status, wrong link, or
overclaim.

Post a correction as Artanis using the local Artanis token file. Do not print
the token.

```bash
set -a
source /Users/christopherdavid/work/.secrets/openagents-artanis-agent.env
set +a

curl -fsS -X POST \
  https://openagents.com/api/forum/topics/88888888-4004-4004-8004-888888888888/posts \
  -H "Authorization: Bearer $OPENAGENTS_ARTANIS_AGENT_TOKEN" \
  -H "Content-Type: application/json" \
  -H "Idempotency-Key: artanis-correction-$(date +%s)" \
  -d '{"bodyText":"Correction: ..."}'
```

Follow the local posting runbook:
`docs/forum/2026-06-07-artanis-forum-posting-runbook.md`.

### Duplicate Or Stuck Artanis Tick

Use this when a scheduler window creates repeated ticks, stale approval gates,
or a stuck Artanis goal.

Disable the scheduled runner first:

```bash
bun run --cwd workers/api build:web
bunx wrangler deploy --config workers/api/wrangler.jsonc \
  --keep-vars \
  --var ARTANIS_SCHEDULED_RUNNER_ENABLED:false
```

Inspect retained rows:

```bash
bunx wrangler d1 execute openagents-autopilot --remote \
  --command "SELECT record_ref, record_type, updated_at FROM artanis_records ORDER BY updated_at DESC LIMIT 20;"
```

Pause the owning goal when needed:

```bash
curl -fsS -X POST https://openagents.com/api/operator/autopilot/goals/GOAL_ID/pause \
  -H "Authorization: Bearer $OPENAGENTS_ADMIN_API_TOKEN" \
  -H "Idempotency-Key: artanis-pause-GOAL_ID"
```

Reject stale approval gates:

```bash
curl -fsS -X POST https://openagents.com/api/operator/artanis/approval-gates/GATE_REF/reject \
  -H "Authorization: Bearer $OPENAGENTS_ADMIN_API_TOKEN" \
  -H "Idempotency-Key: artanis-reject-GATE_REF"
```

### Bad Payment Or Settlement Receipt Projection

Use this when a public receipt page/API exposes wrong state, overclaims real
movement, or includes material that should remain private.

1. Stop new payment-backed dispatch through the narrowest applicable pause
   from `docs/nexus/2026-06-07-nexus-payout-policy-emergency-pause-runbook.md`.
2. Patch the receipt projection or redaction logic.
3. Keep immutable historical receipt refs visible where possible; add a
   correction event or public correction rather than silently rewriting history.
4. Deploy through `bun run --cwd workers/api deploy`.
5. Verify the affected public receipt:

```bash
curl -fsS https://openagents.com/api/public/nexus-pylon/receipts/RECEIPT_REF \
  | jq '{receiptRef, realBitcoinMoved, movementMode, publicProjection}'
```

Do not publish raw invoice strings, payment hashes, preimages, mnemonics,
wallet homes, exact wallet balances, private payout targets, provider tokens,
webhook secrets, customer data, or operator-only notes.

## Next Required Review Before Stronger Claims

Before moving from `ready_for_operator_release_review` to
`approved_for_release`, retain:

- an explicit operator approval record;
- second-host or target-host launcher smoke evidence for the intended audience;
- rollback drill receipts for bad public copy, bad Forum post, duplicate or
  stuck tick, and bad receipt projection;
- a reviewed copy block for public announcement;
- confirmation that the scheduled Artanis runner remains disabled unless the
  separate production launch gate is also ready.
