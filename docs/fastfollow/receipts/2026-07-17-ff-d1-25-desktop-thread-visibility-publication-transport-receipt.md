---
artifact_schema: "openagents.fastfollow.implementation_receipt.v0"
artifact_id: "openagents.fastfollow.receipt.ff_d1_25.desktop_thread_visibility_publication_transport.20260717"
class: "implementation_receipt"
status: "implemented"
disposition: "bounded_packet_landed"
directive_id: "amp.day1_thread_fabric_surfaces"
stage: "implementation"
fast_follow_revision: 3
base_revision: "e2a38a225e8df469c0d5a8ff24d9a171a06b6424"
claim_revision: "1fd8c5913573998a33fd7badfda7b8e5b4fca626"
implementation_revision: "f449d6200f3acfa30f484c612c1469b589d82c5a"
proof_rung: "desktop_thread_visibility_owner_publication_transport"
observed_at: "2026-07-17T19:57:33Z"
---

# FF-D1-25 Desktop public-visibility publication transport receipt

## Authority and obligation reconciliation

The owner-accepted ordered program and durable FF-D1-25 claim in the
[accepted-plan ledger](../../sol/2026-07-16-fast-follow-expansion-accepted-plan.md)
admitted this packet after FF-D1-24 released. Current `origin/main`, prior Day
1 receipts and releases, Fast Follow revision 3, the accepted plan, relevant
ProductSpec and AssuranceSpec obligations, repository invariants, open issues,
known baselines, Git configuration, and active worktrees were reconciled before
mutation.

The tested implementation tree was landed on current `origin/main` at
`f449d6200f3acfa30f484c612c1469b589d82c5a`. This documentation-only release
annotation records that remote fact; it does not extend scope.

No target-authoritative named-group membership source exists, and active work
continues to own Desktop `main.ts` and broad renderer surfaces. This packet
therefore uses only two new Desktop implementation paths and the claimed
receipt/ledger paths. It does not infer group membership or collide with the
separately owned Fast Follow teardown update. AssuranceSpec inventory remains
proposed proof design rather than a provider-owned verdict.

## Implemented packet

- added one Effect transport from an exact FF-D1-23 owner authorization and an
  applied `internet_readable` disclosure receipt to the existing authenticated
  `/api/share` projection service;
- required exact receipt, thread, visibility-version, owner-basis, and supported
  server-source bindings before reading a host-custodied access token;
- sent exactly one public-share request containing only the source kind/ref and
  public audience marker, leaving authoritative loading and redaction to the
  existing server share builder;
- accepted only a bounded active response with an exact shape and a canonical
  URL on the configured service origin, returning only share and receipt refs;
  and
- failed closed without dispatch on invalid or unsupported requests and without
  retry on ambiguous delivery, reporting `publication_outcome_unknown` because
  the existing create route has no reviewed idempotency contract.

## Proof

| Check                                                | Result                                      |
| ---------------------------------------------------- | ------------------------------------------- |
| Focused publication/authority/disclosure/share tests | PASS — 26/26                                |
| Desktop package typecheck                            | PASS                                        |
| Fast Follow package checks                           | PASS — 13/13                                |
| Behavior-contract checks                             | PASS — 36/36                                |
| ProductSpec focused test                             | PASS — 104/104                              |
| Sol document tests and manifest                      | PASS — 19/19                                |
| `pnpm run check`                                     | PASS                                        |
| `pnpm run check:fast`                                | PASS                                        |
| Targeted AssuranceSpec suite                         | BASELINE FAIL — 189/190; environment digest |
| Root Fast Follow coverage                            | BASELINE FAIL — 6/7; teardown seed owned    |

The targeted AssuranceSpec suite reproduced only the known environment-profile
digest snapshot mismatch. Root Fast Follow teardown coverage still fails only
because committed `FASTFOLLOW.md` does not yet reference the separately owned
mobile-component teardown. A broader accidental workspace test invocation also
surfaced unrelated existing failures before it was stopped; the exact targeted
ProductSpec and AssuranceSpec commands above establish the bounded proof. This
packet neither absorbed nor weakened unrelated repairs.

## Honest boundary and next packet

This receipt closes only owner-authorized public-visibility transport into the
existing server-side redacted share builder. It does not add named-group or
workspace publication, reconcile an ambiguous create, compose the transport in
`main.ts`, connect a renderer command, render disclosure pixels, prove an
installed runtime journey, or release/deploy anything. Those residuals, owner
acceptance, and Day 1 completion remain unclaimed.
