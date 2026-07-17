---
artifact_schema: "openagents.fastfollow.implementation_receipt.v0"
artifact_id: "openagents.fastfollow.receipt.ff_d1_26.desktop_thread_visibility_workspace_publication_transport.20260717"
class: "implementation_receipt"
status: "implemented"
disposition: "bounded_packet_landed"
directive_id: "amp.day1_thread_fabric_surfaces"
stage: "implementation"
fast_follow_revision: 3
base_revision: "8342ad394292473564fb2f15429a65c2fc675562"
claim_revision: "f22f0ae89d2156ccf3509ed1d885fba368143c17"
implementation_revision: "pending_remote_landing"
proof_rung: "desktop_thread_visibility_workspace_publication_transport"
observed_at: "2026-07-17T20:14:36Z"
---

# FF-D1-26 Desktop workspace-members publication transport receipt

## Authority and obligation reconciliation

The owner-accepted ordered program and durable FF-D1-26 claim in the
[accepted-plan ledger](../../sol/2026-07-16-fast-follow-expansion-accepted-plan.md)
admitted this packet after FF-D1-25 released. Current `origin/main`, prior Day
1 receipts and releases, Fast Follow revision 3, the accepted plan, relevant
ProductSpec and AssuranceSpec obligations, repository invariants, open issues,
known baselines, Git configuration, and active worktrees were reconciled before
mutation.

Canonical export boot composition still lacks a production canonical accepted-
event source, and no target-authoritative named-group membership source exists.
Active T3 UI work also continues to own Desktop `main.ts` and broad renderer
surfaces. This packet therefore uses only two new Desktop implementation paths
and the claimed receipt/ledger paths. It does not infer group membership,
collide with active host/UI work, or absorb the separately owned Fast Follow
teardown update. AssuranceSpec inventory remains proposed proof design rather
than a provider-owned verdict.

## Implemented packet

- added one Effect transport from an exact applied `workspace_members`
  visibility receipt and bounded FF-D1-23 authorization decision to the
  existing authenticated `/api/share` projection service;
- derived the audience team only from exact `scope.team.<teamId>` identity and
  required a team-thread source to carry the same team id;
- validated the complete request before reading a host-custodied access token,
  then sent exactly one request containing only source identity and the exact
  `TeamMembers` audience;
- left authoritative source access and live membership checks in the existing
  server share service, never uploading local transcript or native bytes;
- accepted only a bounded active response with the expected audience label and
  a canonical URL on the configured service origin; and
- failed closed without dispatch on invalid or expanded requests and without
  retry on ambiguous delivery, because the existing create route has no
  reviewed idempotency contract.

## Proof

| Check                                              | Result                                      |
| -------------------------------------------------- | ------------------------------------------- |
| Focused workspace/authority/disclosure/share tests | PASS — 58/58                                |
| Desktop package typecheck                          | PASS                                        |
| Fast Follow package checks                         | PASS — 13/13                                |
| Behavior-contract checks                           | PASS — 36/36                                |
| ProductSpec focused test                           | PASS — 104/104                              |
| Sol document tests and manifest                    | PASS — 19/19                                |
| `pnpm run check`                                   | PASS                                        |
| `pnpm run check:fast`                              | PASS                                        |
| Targeted AssuranceSpec suite                       | BASELINE FAIL — 189/190; environment digest |
| Root Fast Follow coverage                          | BASELINE FAIL — 6/7; teardown seed owned    |

The targeted AssuranceSpec suite reproduced only the known environment-profile
digest snapshot mismatch. Root Fast Follow teardown coverage still fails only
because committed `FASTFOLLOW.md` does not yet reference the separately owned
mobile-component teardown. This packet neither absorbed nor weakened either
unrelated repair.

## Honest boundary and next packet

This receipt closes only workspace-members publication into the existing
server-side redacted share builder. It does not add named-group authority or
publication, reconcile an ambiguous create, supply canonical export evidence,
compose resources in `main.ts`, connect a renderer command, render disclosure
pixels, prove an installed runtime journey, or release/deploy anything. Those
residuals, owner acceptance, and Day 1 completion remain unclaimed.
