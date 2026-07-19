# OB-4 (#8561) — Batch approval operator surface

**Date:** 2026-07-09
**Status:** Operator UI + admin OpenAuth surface landed. Live 100/day proof and Sarah S-8 email channel remain residual.

## What shipped

### Already on main (API domain, commit `2d39c1faf5`)

- `GET /api/operator/crm/commands/batch-queue` — static admin-token gate
- `POST /api/operator/crm/commands/batch-approve` — static admin-token gate
- Domain: `crm-approval-batch.ts` (`listCrmApprovalQueue`, `batchApproveCrmSendCommands`)
- Sarah reply-routing CRM plumbing (`crm-reply.ts`)

### This change (operator-facing)

| Layer | Path / module |
| --- | --- |
| Admin OpenAuth API | `GET /api/admin/ops/crm/batch-queue` |
| Admin OpenAuth API | `POST /api/admin/ops/crm/batch-approve` |
| Worker routes | `crm-approval-batch-admin-routes.ts` |
| Aiur proxy | `/api/admin/ops/crm/batch-*` via `admin-credits-proxy.ts` |
| Aiur UI | Ops page → `CrmBatchApprovalPanel` (`apps/aiur/src/ops/crm-batch-console.tsx`) |

The Aiur panel lists pending `crm_contact_commands` (`send_email`, status `proposed`) grouped by day + segment, supports select-all / per-row selection, and posts a one-tap batch approve.

## Invariant: `lead_gen_agent.no_send_without_approval_receipt.v1` — PRESERVED

Batch approve is **UX only**, not new send authority.

1. Operator (or static admin token) calls batch-approve with a list of command ids.
2. `batchApproveCrmSendCommands` walks each id and calls the **unchanged** `approveAndExecuteCrmSendCommand` one-by-one.
3. Each command keeps its own per-send result / receipt on `crm_contact_commands`.
4. A batch action *additionally* writes one `crm_command_batches` rollup receipt.
5. A server-side daily send cap (default 100/day) stops mid-batch once hit. Remaining items return disposition `capped` and are **never sent**.

Nothing in the admin routes, Aiur proxy, or UI invents a bulk-send primitive or bypasses the proposal → approval → `dispatchCrmSend` path.

## Residuals (issue stays open)

1. **Live 100/day + &lt.30 Min operator time** — needs a real drafted day and measured ops timing (OB-6 operatorMinutes still `not_measured` until timing is wired).
2. **OB-1 arming (#8558)** — full ramp-config governance not landed. Daily cap is a fixed stand-in.
3. **Sarah S-8 email channel** — lives in the private Sarah service. Until `SARAH_EMAIL_CHANNEL_LIVE` / S-8 calls `recordCrmReplyEvent` (or the inbound webhook), `routedTo` stays the v0 `operator_notification` fallback.
