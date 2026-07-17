---
artifact_schema: "openagents.fastfollow.implementation_receipt.v0"
artifact_id: "openagents.fastfollow.receipt.ff_d1_10.desktop_thread_export_store.20260717"
class: "implementation_receipt"
status: "implemented"
disposition: "implementation_ready_for_claim_release"
directive_id: "amp.day1_thread_fabric_surfaces"
stage: "implementation"
fast_follow_revision: 3
base_revision: "6ea8e81b0b057fad3d4d080b310cfc1756e2bc57"
claim_revision: "0d5b0dd40c379401bb7cfcda278259eebd23c4e0"
proof_rung: "private_atomic_owner_only_export_persistence"
observed_at: "2026-07-17T14:46:54Z"
---

# FF-D1-10 Desktop thread export store receipt

## Authority and obligation reconciliation

The owner-accepted ordered program and bounded FF-D1-10 claim in the
[accepted-plan ledger](../../sol/2026-07-16-fast-follow-expansion-accepted-plan.md)
admitted this packet after FF-D1-09 released. Current `origin/main` contained
the separately landed mobile/Sync lifecycle work. Active dirty worktrees owned
Desktop history, shell, renderer, main-process integration, update, and release
paths, so this packet excluded those surfaces and used two new Desktop
main-process files. Current GitHub searches found no open Fast Follow, thread
export, disclosure, or event-authority issue and no competing packet claim;
repository policy does not require a feature issue for this accepted-plan work.

This slice advances the private, explicit, owner-controlled export boundary in
Fast Follow ProductSpec `FF-AC-04`, `FF-AC-06`, and `FF-AC-12`. AssuranceSpec
inventory remains proposed proof design rather than a provider-owned verdict.
No Desktop command or pixel, save-dialog or remote transport, broader audience,
provider acceptance, installed-runtime proof, or Day 1 completion is claimed.

## Implemented packet

- added a Desktop main-process store for the exact FF-D1-09 owner-only
  canonical event bundle compilation;
- decoded and re-bound the export intent, artifact, thread, format, audience,
  UTF-8 encoding, exact bytes, 4 MiB bound, and SHA-256 identity before write;
- stored digest-addressed artifacts beneath a private mode-0700 directory via a
  unique mode-0600 temporary file and atomic rename;
- made exact retries idempotent while refusing corrupt or conflicting existing
  bytes without overwrite;
- re-opened artifacts only through an exact ref/digest pair and revalidated the
  digest, artifact schema, size, and owner-only audience before returning bytes
  inside the main-process module; and
- returned only a schema-valid ref-only `export_created` receipt from persist,
  without exposing a filesystem path or granting renderer/transport authority.

## Proof

| Check                             | Result                                               |
| --------------------------------- | ---------------------------------------------------- |
| Focused Desktop store tests       | PASS — 5/5                                           |
| Isolated store TypeScript compile | PASS                                                 |
| Fast Follow policy/spec checks    | PASS — 20/20                                         |
| Behavior-contract checks          | PASS — 36/36                                         |
| ProductSpec package test          | PASS — 104/104                                       |
| Sol document checks               | PASS — 19/19 plus manifest check                     |
| `pnpm run check`                  | PASS                                                 |
| `pnpm run check:fast`             | PASS                                                 |
| Desktop package typecheck         | BASELINE FAIL — unrelated lifecycle schema drift     |
| Targeted AssuranceSpec suite      | BASELINE FAIL — 189/190; environment digest snapshot |

The Desktop package typecheck fails on current claimed `main` because recently
landed lifecycle schema changes require `status`, `renameThread`, and
`setThreadStatus` updates in existing runtime gateway/conversation tests and
fakes. None of those files is owned or changed by this packet, and they overlap
active Desktop/Sync work. The new store independently passes strict TypeScript
compilation. The targeted AssuranceSpec suite's sole failure is an existing
environment-profile digest snapshot mismatch in `compiler.test.ts`; this packet
does not change that compiler or environment authority. Both collisions are
preserved rather than weakened or opportunistically repaired.

## Honest boundary and next packet

This receipt closes only private Desktop main-process persistence and verified
reload of owner-only canonical export artifacts. It does not wire IPC or a
Desktop command, present disclosure/export pixels, choose a user destination,
perform remote transport, broaden disclosure audiences, or prove a rendered or
installed-runtime journey. Those residuals, remaining adapters, owner
acceptance, release/deployment, and Day 1 completion remain unclaimed.
