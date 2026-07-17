---
artifact_schema: "openagents.fastfollow.implementation_receipt.v0"
artifact_id: "openagents.fastfollow.receipt.ff_d1_12.desktop_thread_export_file_transport.20260717"
class: "implementation_receipt"
status: "implemented"
disposition: "bounded_packet_landed"
directive_id: "amp.day1_thread_fabric_surfaces"
stage: "implementation"
fast_follow_revision: 3
base_revision: "fe8963884b6aba8a7c1a6ebdc1818c7a13302051"
claim_revision: "f91ae18772f768793084a961fb8054d8f43bffe3"
proof_rung: "owner_selected_atomic_canonical_export_file_transport"
observed_at: "2026-07-17T15:14:24Z"
---

# FF-D1-12 Desktop thread export file transport receipt

## Authority and obligation reconciliation

The owner-accepted ordered program and bounded FF-D1-12 claim in the
[accepted-plan ledger](../../sol/2026-07-16-fast-follow-expansion-accepted-plan.md)
admitted this packet after FF-D1-11 released. Current `origin/main`, prior Day 1
receipts/releases, Fast Follow revision 3, the accepted plan, relevant
ProductSpec and AssuranceSpec obligations, repository invariants, open issues,
known baselines, Git configuration, and active worktrees were reconciled before
mutation. GitHub searches found no relevant Fast Follow/export/disclosure issue
or competing claim.

Active work still owned Desktop `main.ts`, history, shell, renderer, update,
and release surfaces. This packet therefore used two new main-process transport
files and did not touch those collisions. It advances the explicit,
owner-controlled and privacy-preserving export boundary under Fast Follow
ProductSpec `FF-AC-04`, `FF-AC-06`, and `FF-AC-12`. AssuranceSpec inventory
remains proposed proof design, not a provider-owned verdict.

## Implemented packet

- added a main-process-only file transport whose caller supplies only an
  unknown ref-only disclosure receipt;
- decoded and restricted that receipt to an owner-only
  `canonical_event_bundle` `export_created` outcome before loading or picker
  activity;
- verified-loaded bytes only through the receipt's exact FF-D1-10 artifact
  ref/digest pair;
- proposed a bounded sanitized `.json` filename and accepted a destination
  only from an injected host-owned selector;
- treated cancellation as a non-error, required an absolute `.json` path, and
  required explicit replace authority before an existing target could change;
- staged mode-0600 bytes in the selected directory, used an exclusive hard-link
  publication for no-replace and atomic rename for authorized replacement, and
  removed or rolled back temporary/partial new files on failure; and
- returned only ref/digest and replace-authority facts, never destination path,
  artifact bytes, raw evidence, native errors, or broader audience authority.

## Proof

| Check                                 | Result                                           |
| ------------------------------------- | ------------------------------------------------ |
| Focused transport/command/store tests | PASS — 16/16                                     |
| Isolated transport TypeScript compile | PASS                                             |
| Fast Follow policy/spec checks        | PASS — 20/20                                     |
| Behavior-contract checks              | PASS — 36/36                                     |
| ProductSpec package test              | PASS — 104/104                                   |
| Sol document checks                   | PASS — 19/19 plus manifest check                 |
| `pnpm run check`                      | PASS                                             |
| `pnpm run check:fast`                 | PASS                                             |
| Desktop package typecheck             | BASELINE FAIL — unrelated lifecycle schema drift |
| Targeted AssuranceSpec suite          | BASELINE FAIL — 189/190; environment digest      |

The Desktop typecheck reproduces only the pre-existing lifecycle-schema
failures recorded by FF-D1-10 and FF-D1-11: existing conversation/gateway
fixtures omit thread `status`, and one live-subscription fake omits
`renameThread` and `setThreadStatus`. The new transport produces no errors and
passes an isolated strict compile. The targeted AssuranceSpec suite reproduces
the unrelated environment-profile digest snapshot mismatch at 189/190. Neither
collision is owned, absorbed, weakened, or claimed fixed here.

The implementation landed on `main` as
`dc9e62769160d115520bbde0f22af2a148401694`. The fetched remote tree exactly
matched the fully checked local implementation tree.

## Honest boundary and next packet

This receipt closes only owner-selected local JSON transport after canonical
export creation and private persistence. It does not add IPC/preload wiring,
render disclosure/export pixels, authorize broader audiences, provide remote
transport, or prove an installed/runtime-rendered journey. Those residuals,
remaining adapters, owner acceptance, release/deployment, and Day 1 completion
remain unclaimed.
