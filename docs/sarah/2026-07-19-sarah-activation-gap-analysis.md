# Sarah immediate-activation gap analysis (2026-07-19)

> **Owner direction (2026-07-19):** "I want Sarah active immediately, using the
> new sandbox stuff being worked on, delegating coding, and updating me via
> push notifications in my mobile app."
>
> This document decomposes that direction into what is already live, what is
> flag-gated, and what is genuinely missing, with the issue ledger that covers
> every gap. Companion: `2026-07-19-sarah-program-assessment.md` (full program
> assessment). Keep this document updated as gaps close.

## 1. Verdict at a glance

| Owner requirement | Status | Gate / gap | Issue |
| --- | --- | --- | --- |
| Sarah active in mobile | 🟡 **Live but admission-gated** | Admin-email allowlist is one hardcoded address; bootstrap receipt mints on first admitted request | [#9065](https://github.com/OpenAgentsInc/openagents/issues/9065) SARAH-ACT-1 |
| Delegating coding | 🟢 **Live in code** | Operational only: owner-linked Pylon Codex capacity must be online | #9065 (checklist item) |
| Using managed sandboxes | 🟡 **Code-landed, default-off** | SBX-09 live GCP acceptance, then two env vars | [#9033](https://github.com/OpenAgentsInc/openagents/issues/9033) SBX-09 (now P0) |
| Push notifications to mobile | 🔴 **Two hard gaps** | Mobile never registers a push token; nothing emits notify-events | [#9062](https://github.com/OpenAgentsInc/openagents/issues/9062), [#9063](https://github.com/OpenAgentsInc/openagents/issues/9063) |
| Sarah updates me proactively | 🔴 **Missing** | No code path originates a Sarah→owner message on delegation settlement | [#9064](https://github.com/OpenAgentsInc/openagents/issues/9064) |

Fastest path to the full owner experience: **#9065 (activation) → #9062 +
#9063 in parallel (push) → #9064 (proactive updates) → #9033 (sandbox flag)**.
Nothing except SBX-09 requires new infrastructure; everything else is wiring
between systems that already exist and test green.

## 2. Requirement 1 — "Sarah active immediately"

**What is live.** The `principal.sarah` runtime is deployed (TestFlight build
119 + server deploy). The mobile app pins Sarah in the conversation UI; turns
run on the hosted-runtime dispatch tick (`runHostedRuntimeTurnDispatch`) with
Gemma 4 buffered function calling via `gemma4-adapter.ts`.

**The gate.** `hasSarahThreadAuthority`
(`apps/openagents.com/workers/api/src/sarah-owner-routes.ts:86–139`) requires
three conjunctive facts:

1. `isSarahThreadForOwner(ownerUserId, threadRef)`.
2. The owner's primary or auth-identity email passes `isOpenAgentsAdminEmail`
   — `admin-identity.ts` hardcodes exactly **one** address
   (`chris@openagents.com`). Any other sign-in identity refuses.
3. An admitted bootstrap receipt
   (`receipt.authority.sarah.bootstrap.<threadRef>.rev4`, outcome `succeeded`)
   exists in `sarah_authority_decision_receipts`. There is **no separate
   bootstrap endpoint** — `ensureSarahPrincipal` mints it on the first admitted
   request to `/api/mobile/sarah`.

**Gap.** If the owner's live mobile account resolves to any non-allowlisted
email, activation silently never happens. #9065 verifies the identity mapping,
converts the hardcoded literal to config-driven owner admission (owner-named
identities only — this is an authority-adjacent surface), and records one live
bootstrap proof.

## 3. Requirement 2 — "delegating coding"

**Already live.** `makeSarahRuntimeTools` (`sarah-runtime-tools.ts:684–693`)
gives Sarah these tools today, independent of the sandbox flag:

| Tool | Purpose |
| --- | --- |
| `codex_workers_capacity` | Read owner-linked Pylon Codex capacity |
| `codex_workers_start` | **Dispatch bounded Codex workers** (`codex_agent_task`, exact-commit pinned, real assignment refs) |
| `codex_workers_status` | Follow dispatched work |
| `full_auto_status` / `full_auto_control` | Read + pause/resume/stop an existing Full Auto run (pending until Desktop applies) |
| `sarah_harness_status` / `sarah_harness_review_history` | Conditional harness tools |
| `managed_sandbox_*` (8 tools) | Present but **refusing** until the broker flag admits (see §4) |

**Gap.** None in code. Operationally the owner-linked Pylon must be online
with advertised Codex capacity, else `codex_workers_start` returns honest
blockers. Folded into the #9065 activation checklist.

## 4. Requirement 3 — "using the new sandbox stuff"

**What is landed.** SBX-00..07 (epic #9023) are all closed and code-landed:
lifecycle authority, generation-fenced Postgres store, GCP runtime adapter,
Box v1 facade, long-running turns/interrupt, I/O + quotas, IDE integration,
and Sarah's own broker (`sarah-managed-sandbox.ts`, 8 closed tools, each
routed receipt-first through `authorizeSarahOperation`).

**The gate — precisely.** Sandbox tool admission requires **all four** of
(`index.ts:6926–6933`, `:10596–10603`):

- `MANAGED_SANDBOX_BROKER_ENABLED=true` — **absent from
  `scripts/cloudrun/env-production.yaml` → OFF in production**
- `OA_MANAGED_SANDBOX_IMAGE_DIGEST=sha256:…` — **absent in production**
- `OA_CLOUD_CONTROL_URL` — ✅ already set (oa-cloud-run-bridge)
- `OA_CLOUD_CONTROL_TOKEN` — ✅ already provided via Secret Manager
- (`KHALA_SYNC_DB` binding — ✅ present)

When off, Sarah **sees** the eight tools and gets typed refusals
(`runtimeAdmitted=false` fails `condition.existing_runtime_gate` before any
target effect) — she can explain the blocker honestly but cannot mutate.

**Why the flag is off, and the honest path to on.** Authority (Sarah profile
rev 4 `condition.managed_sandbox_runtime_admission`) and the roadmap both
require **SBX-09 (#9033)** — independent live GCP acceptance: create→ready→
real Codex/Claude turn→interrupt/settle→stop/resume/delete, cross-owner
denial, fault matrix, zero-residue cleanup, cost, rollback. "Active
immediately" therefore means **execute SBX-09 now**, not skip it: profile
text, SDK status, or a provider object cannot substitute for the live proof,
and flipping the flag without it would put an unproven GCP mutation path in a
model's hands. SBX-09 is re-prioritized to **P0** per this owner direction
(comment on #9033), and its acceptance journey should include the Sarah
create→dispatch→settle→delete path so the flag flip rides the same proof.

After SBX-09: set the two missing env vars in the Cloud Run deploy config and
the capability is live end-to-end.

**Deliberately not in scope:** SBX-08 (#9031, mobile/web sandbox supervision
UI) improves observability of Sarah-created sandboxes but does not gate Sarah's
broker; SBX-10 (#9032) stays deferred.

## 5. Requirement 4 — "updating me via push notifications"

This is where the real construction is. The plumbing exists on both ends and
tests green; **two links in the chain were never wired**, and the proactive
trigger doesn't exist at all.

### What exists

- **Mobile:** `expo-notifications@57` with a full permission + preference flow
  (`src/settings/expo-mobile-notification-settings.ts`, prefs
  `attention`/`completion`/`approvals` in SecureStore) and notification-tap
  deep-link handling (`src/app.tsx`).
- **Server:** a complete push module in
  `apps/openagents.com/workers/api/src/push/` — Expo-relay sender with
  batching and `DeviceNotRegistered` pruning (`push-sender.ts`), device-token
  registry (`POST|DELETE /api/mobile/push-tokens`), per-user preferences,
  typed notify-event ingest (`POST /api/internal/push/notify-events`), and
  fixed payload templates keyed by kind
  (`turn_completed | turn_needs_input | turn_failed | credit_low`).
- **Fallback signal:** the Khala Sync `attention` projection
  (`runtime-mutators.ts`) — pull-based, works today, but only while the app is
  open.

### Gap A — the device is never enrolled ([#9062](https://github.com/OpenAgentsInc/openagents/issues/9062))

No mobile code calls `Notifications.getExpoPushTokenAsync()` or
`POST /api/mobile/push-tokens`. The token registry is empty; **no device can
receive any push today**. (The settings health snapshot reads the *native*
`getDevicePushTokenAsync` token, which the server's Expo relay cannot use —
that seam gets resolved in the same issue.) #9062 wires registration on
sign-in/permission-grant, rotation, sign-out removal, and proves live
delivery + tap-deep-link into the Sarah thread on a real device.

### Gap B — nothing emits events ([#9063](https://github.com/OpenAgentsInc/openagents/issues/9063))

`dispatchNotifyEvent` is called by nothing outside the push module: the only
entry is the interim admin-bearer HTTP ingest, whose documented future callers
(org cloud executor, metering) are not merged. And the event vocabulary has no
Sarah/delegation kinds. #9063 makes the hosted-runtime dispatch tick call
`dispatchNotifyEvent` **in-process** on Sarah turn completed/failed/
needs-input, and adds bounded kinds for delegation outcomes (worker closeout,
sandbox turn settled/failed, sandbox lifecycle failure). Payloads stay the
small fixed template set — receipts by ref, nothing private in APNs payloads.

### Gap C — Sarah never speaks first ([#9064](https://github.com/OpenAgentsInc/openagents/issues/9064))

The cron path only **drains owner-queued turns**; grep confirms no code
originates a Sarah→owner message. If the owner dispatches workers and closes
the app, outcomes sit in receipts until asked. #9064 appends one bounded,
receipt-backed status message to the owner thread per delegation settlement
(idempotent, cites the exact target receipt, honest about
pending/failed/refused per SARAH-AC-09/14), paired with the #9063 push. No new
authority grants — append-only on the owner's own thread.

### Later hardening (not blocking)

Async Expo `/getReceipts` polling and direct APNs/FCM remain documented
follow-ups in `push-sender.ts`; the interim admin-bearer ingest route stays
for future external callers.

## 6. Issue ledger for this direction

| Issue | Title | Priority | Blocks |
| --- | --- | --- | --- |
| [#9065](https://github.com/OpenAgentsInc/openagents/issues/9065) | SARAH-ACT-1 — identity admission, bootstrap receipt, capacity readiness | P0 | Everything (owner must be admitted) |
| [#9062](https://github.com/OpenAgentsInc/openagents/issues/9062) | SARAH-PUSH-1 — mobile Expo push-token registration + live delivery proof | P0 | Any push reaching the device |
| [#9063](https://github.com/OpenAgentsInc/openagents/issues/9063) | SARAH-PUSH-2 — emit notify-events from Sarah turns + delegation settlement | P0 | Push having anything to say |
| [#9064](https://github.com/OpenAgentsInc/openagents/issues/9064) | SARAH-PROACTIVE-1 — receipt-backed delegation-outcome thread updates | P0 | "Sarah updates me" without asking |
| [#9033](https://github.com/OpenAgentsInc/openagents/issues/9033) | SBX-09 — live GCP acceptance + rollout (pre-existing, re-prioritized P0) | P0 | Sandbox mutation flag flip |
| [#9031](https://github.com/OpenAgentsInc/openagents/issues/9031) | SBX-08 — mobile/web sandbox supervision (pre-existing) | P1 | Observability only, not gating |

Dependency shape: #9065 is independent and first. #9062 ∥ #9063 (join for the
live end-to-end proof). #9064 depends on #9063 for its paired push. #9033 is
independent of the push lane and unblocks the sandbox tools whenever its
acceptance passes.

## 7. What this direction does NOT change

- No revival of the public `/sarah` surface, avatar/OAV stack, or prospect CRM
  — the tombstone and Era-1 retirement stand.
- No new authority grants: every lane above operates inside root rev 6 /
  Sarah profile rev 4. #9065's allowlist change is admission plumbing for
  identities the owner explicitly names, not an authority expansion.
- SBX-09 is accelerated, not bypassed. The flag stays off until its live
  proof lands; refusing to shortcut it is what keeps "Sarah with cloud
  mutation authority" defensible.
- The AssuranceSpec deficit (SARAH-AC-01..20 `needs_design`, see the program
  assessment §5) is unchanged by this direction and remains the release-grade
  gap.
