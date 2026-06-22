# CRM State-of-the-World + Native Cloudflare Build Audit

**Date:** 2026-06-22
**Author:** Raynor (agent)
**Trigger:** Owner wants to (a) connect "our CRM" to Autopilot Desktop / chat for *automated* CRM, through the Blueprint action system, and (b) contact ~150 people tomorrow with automated email — **a bunch sent from the owner's personal business email via the Google Workspace CLI (`gws`), some via Resend.**

> **Owner mandate (2026-06-22):**
> 1. **No Laravel CRM** as a runtime, an API we keep calling, or a sender. Move **all relevant CRM/outreach code into `apps/openagents.com`** (and related Cloudflare/desktop surfaces), **multi-tenant, so customers use the exact same CRM/outreach infra we built for ourselves.** Convex (`autopilot3`) is also not the runtime; Laravel + autopilot3 are **schema/design references only.**
> 2. **Keep the Gmail path — port it in.** The `gws` Google Workspace CLI send-from-personal-business-email capability that worked with Laravel is a **first-class send channel** in the new CRM, alongside Resend. Some outreach goes out 1:1 from the owner's own mailbox (high-touch, lands in primary inbox); some goes via Resend (scalable/transactional). Both are driven by the **same** D1 CRM (contacts, templates, suppression, ledger).

---

## 0. TL;DR — the decision

**Build one multi-tenant CRM, natively, in `apps/openagents.com` on Cloudflare/Effect + D1, with a two-channel sender: Gmail-via-`gws` (the owner's personal business mailbox, executed locally) *and* Resend (Worker-side).** Same contact model, templates, suppression, and send ledger feed both channels. Same engine powers our outreach and every customer's. Nothing runs on Laravel or Convex.

| Layer | Status today | Work |
|---|---|---|
| **Resend channel** (provider, message/delivery ledger, campaigns, suppression, preferences, webhooks) | ✅ Exists in `apps/openagents.com` but **INERT** (default-OFF flag, not chained) and **deliverability unproven** | Arm + **prove** it |
| **Gmail/`gws` channel** (send-from-personal-business-email) | ⚠️ Exists as a **Laravel-coupled local script** (`scripts/crm-gmail.sh` + `gws`) | **Port it in**: drive from our D1 CRM, write back to our ledger, draft-first; runs as a local/desktop executor (Gmail OAuth can't live in a Worker) |
| **Contact CRM model** (contacts, accounts, lists, activities, engagement, opportunities) | ❌ Does not exist on Cloudflare (only campaign/subscriber/prospect tables) | **Build in D1**, tenant-scoped, using autopilot3/Laravel shapes as reference |
| **The contacts (the ~150)** | ❌ Not on Cloudflare (in the old prod DB) | **One-time CSV import** into D1 |
| **Chat → Blueprint `send_email`** | ⚠️ Blueprint recognizes the `send_email` effect; chat doesn't propose it; executor not armed | **Wire it** (chat proposes → approval → executor → send via chosen channel → write-back) |

**Honest bottom line:** real build, four+ pieces, none green today — but three are already scaffolded (Resend ledger/campaigns, the Blueprint `send_email` frame, the proven-elsewhere `gws` mechanism). The ~150 send is the forcing function: it proves *both* channels and honestly greens `autopilot_sites.native_email_sequences.v1`.

---

## Implementation status (epic #5980, branch `crm/epic-5980`)

Build log — landed on the branch (merges to main at epic end):

- ✅ **#5981 — D1 contact CRM model + read APIs.** Migration `0218` (tenant-scoped contacts/accounts/lists/activities/engagement/opportunities/commands/import-runs); `crm-store.ts`; admin reads under `/api/operator/crm/*`.
- ✅ **#5982 — CSV import.** `crm-import.ts` (parser + header mapping + de-dupe + audited import-run) + `POST /api/operator/crm/import`. Runbook: `csv-import-runbook.md`.
- ✅ **#5983 — Gmail/`gws` channel.** `crm-email.ts` (templates + render + ledger), `crm-email-routes.ts`, `scripts/crm-gmail-send.mjs` (local sender with write-back). Runbook: `gmail-gws-channel-runbook.md`.
- ✅ **#5984 — Resend channel.** `crm-resend.ts` (INERT by default) + `POST .../resend-send` + `scripts/crm-resend-smoke.mjs`. Deliverability green is **owner-gated** (verified domain + key). Runbook: `resend-channel-runbook.md`.
- ✅ **#5985 — Unified two-channel send.** `crm-send.ts` `dispatchCrmSend({channel})` — suppression/unsubscribe gate enforced once, ledger written for both; `POST .../send` + `GET /gmail-queue` for the local executor.
- ⏳ **#5986** chat→Blueprint `send_email{channel}`, **#5987** desktop CRM pane + local executor, **#5988** the ~150 send orchestration — in progress.

Shared suppression gate is `readEmailSendEligibility` (reused). Everything tenant-scoped; no Laravel/Convex runtime; Gmail OAuth stays local.

---

## 1. The ~150 tomorrow — dual-channel, on our infra

We send through **our own CRM**, not Laravel. Two channels, owner picks per-recipient/segment:

- **Gmail via `gws` (personal business mailbox)** — for the "bunch" the owner wants sent 1:1 from their own address. Executed **locally** (Mac / Autopilot Desktop), because sending *as you* needs *your* Google OAuth, which a Cloudflare Worker can't hold. The local executor pulls the contact + rendered template from our CRM API, calls `gws gmail +send` (**draft-first by default**), and **writes the result back** to our D1 ledger as a `crm_activity` + `crm_email_message` (channel=`gmail_gws`).
- **Resend (Worker-side)** — for the portion sent via our automated transactional path.

**Sprint to get there:**
1. **Contacts → D1** (one-time CSV import, §3).
2. **Port the `gws` Gmail sender** to drive from our CRM + write back (the old script did this against Laravel; repoint it at our CRM API).
3. **Arm + prove the Resend transactional send** (the `email_deliverability_unproven` blocker → green, receipt-first).
4. **Send the ~150** across both channels; suppression honored on both; every send recorded in the D1 ledger.

**Timeline risks:**
- **Gmail:** `gws` must be authed as the owner's Workspace account on this Mac (`gws auth login -s gmail`); Workspace caps ~500 external/day (consumer ~100) — 150 is fine for a warmed Workspace account.
- **Resend:** the prod Worker must have `RESEND_API_KEY`/`RESEND_WEBHOOK_SECRET` set and a **verified sending domain** (SPF/DKIM/DMARC). From-address is `OpenAgents <chris+sites@openagents.com>`. **Verify before any blast.**

---

## 2. Inventory by surface

### 2.A — Deprecated Laravel CRM — *schema reference + one-time data source ONLY*

Complete production investor CRM: 11 tables (`crm_contacts`, `crm_accounts`, `crm_contact_lists(+memberships)`, `crm_email_templates`, `crm_email_messages(+deliveries)`, `crm_activities`, `crm_engagement_snapshots`, `crm_opportunities(+roles)`, `crm_writeback_requests`; legacy `investor_accesses`), Sanctum ability model (`crm:read|write|email:send|export|writeback`), approval-gated Blueprint writeback, classification-aware source-export.

**Reference only:** copy the column shapes + ability/approval concepts into D1. If the owner pulls existing contacts from the old prod DB, that's a **one-time** extraction (CSV preferred). **Do not** run it, call it as a live CRM, or use it as a sender. It is being decommissioned (its own `docs/investor-crm-admin-decommission-plan.md` always intended this; target is now Cloudflare).

### 2.B — `apps/openagents.com` (Cloudflare) — *the foundation*

Verified against `workers/api/migrations/` + `config.ts`:

**Already present (email/outreach):** `0026_email_ledger.sql` (email_messages, email_deliveries, email_drafts, email_provider_events), `0063_email_campaign_records.sql` (campaigns, steps, enrollments, sends, **suppression_entries**, **preferences**), `0064`, `0072` (targeted_site campaigns + **prospects**), `0081`/`0088`, `0181_native_lists_subscribers.sql` (`subscriber_lists`, `list_subscribers` — already **owner/team-scoped**, a good multi-tenant precedent), `0191`/`0216` (business_signup), `0193_cloudflare_email_provider.sql` (Cloudflare-Email provider alongside Resend).

**Provider:** Resend. Bindings `config.ts:185-188`, `wrangler.jsonc:44-45`. Webhook `resend-webhooks.ts` verifies Svix + auto-suppresses on bounce/complaint.

**⚠️ Send path INERT:** `email-sequence-send-service.ts` plans a dry-run, never calls the sender unless armed by default-OFF **`EMAIL_SEQUENCE_SEND_ENABLED`**, not chained into the dispatcher. `autopilot_sites.native_email_sequences.v1` is **YELLOW**, sole remaining blocker `email_deliverability_unproven` (`product-promises.ts:211`, `:2598`). Prod secrets + domain verification **unverified**.

**Missing entirely:** the contact/account/list/activity/engagement/opportunity relationship model. Today's tables are campaign/subscriber/prospect shaped. Net-new schema here, tenant-scoped.

**Blueprint:** `blueprint-routes.ts` recognizes a **`send_email`** effect kind → `blueprint_action_submissions` (pending_approval) → approval → execute. Not wired from chat; executor→sender not armed. Frame exists, connection doesn't. The executor must support **both** channels (dispatch to Resend or hand a job to the local Gmail executor).

**Promise homes:** `autopilot_sites.native_email_sequences.v1` (YELLOW), `workrooms.source_authorized_business_objects.v1` (RED, INERT), `workrooms.omni_client_delivery_workrooms.v1`, `omni-crm-follow-up-workrooms.ts` (RED).

### 2.C — autopilot3 (Convex) — *best model reference, not the runtime*

Near-exact port of the Laravel CRM **plus** copyable ideas: `crmContactCommands` (approval-gated mutations → maps onto Blueprint), `crmSourceImportRuns` (import audit), `crmWritebackRequests`. Reference only — not on Convex.

### 2.D — autopilot-omega / vortex / others

`autopilot-omega` = same Resend infra now in `apps/openagents.com` (shared `0026`/`0063` lineage), not additive. `vortex` = scaffolding only. `autopilot2`/`-deprecated` = no real CRM. `apps/openagents.com` is the consolidated home.

### 2.E — Gmail / `gws` send path — *first-class channel, PORT IT IN*

- `~/work/scripts/crm-gmail.sh` (present, executable, 2026-04-30): resolves a contact + template from the **Laravel** CRM API, renders a preview, calls `gws gmail +send` (**draft-first**, `--send` to send live).
- `gws` = Google Workspace CLI at `/Users/christopherdavid/code/googleworkspace-cli` (present). Sends as the authed Google account (the owner's personal business mailbox). Handles MIME/RFC5322 + drafts/send. Built for humans *and* agents.
- Runbook: `~/work/docs/2026-04-30-gws-gmail-crm-runbook.md`.

**Port plan (keep the mechanism, drop the Laravel coupling):**
- Repoint the resolver at **our** CRM API (D1 in `apps/openagents.com`): resolve contact + template, render server-side, return the composed message.
- Keep `gws` as the local sender (Gmail OAuth stays local — Workers can't send *as you*).
- **Add the missing write-back:** after `gws` drafts/sends, POST the outcome back to our CRM as `crm_email_message` (channel=`gmail_gws`, provider_message_id/draft_id) + `crm_activity` (`email_sent`). The old script never did this — fixing it is what makes Gmail sends show up in the CRM.
- Home the executor in a **related surface**: a workspace operator runtime now, and **Autopilot Desktop** as the integrated local executor so chat→Blueprint can route a `gmail_gws` send to the operator's machine.
- Honor **suppression/preferences** before any Gmail send, same as Resend.

---

## 3. The contacts — one-time import, then never look back

The ~150 (and the rest) live in the old prod DB, not a file. **Preferred:** owner provides a **CSV**; we import once into D1 (de-dupe, normalize, report exact count + sample), audited via a `crm_source_import_runs` row. **Alternate (one-time):** a single source-export read, then decommission. Zero ongoing legacy dependency either way.

---

## 4. Target architecture (all in `apps/openagents.com` + desktop, multi-tenant)

```
Autopilot Desktop / chat UI  (NL intent: "follow up 1:1 with investors who opened but didn't reply")
        ▼
Blueprint program ──proposes──► send_email{channel} ──► blueprint_action_submissions (pending_approval)
        │                                   │
        │                          operator approval (chat-surfaced)
        ▼                                   ▼
CRM read model (D1, tenant-scoped:   send dispatch ──┬─► Resend (Worker)  ─► email_messages/deliveries (D1)
contacts/accounts/lists/activities)                  └─► Gmail via gws (LOCAL executor, owner mailbox)
        ▲                                                     │  └─► write-back: crm_email_message + crm_activity
        └────── webhook (resend-webhooks.ts) / local write-back ──► provider_events + suppression + activity
```

**Two-channel send abstraction:** one `SendChannel` selector (`gmail_gws | resend`) per send/campaign over a shared `CrmContact`/`CrmEmailTemplate`/suppression/ledger. Resend executes in the Worker; `gmail_gws` executes on the operator's machine and reports back via the CRM API.

**Multi-tenant from day one:** every CRM table carries owner/team scope (follow the `subscriber_lists` precedent). Same engine for us and customers, with isolation. This is the product.

**Build order (native; no Laravel runtime, no Convex):**
1. **D1 contact-CRM schema + Effect Schema contracts**, tenant-scoped (contacts, accounts, lists(+memberships), activities, engagement_snapshots, opportunities(+roles), contact_commands, source_import_runs) + read APIs.
2. **One-time CSV contact import** into D1.
3. **Port the `gws` Gmail channel** (resolve from our CRM + write-back + draft-first + suppression).
4. **Arm + prove the Resend channel** (live send→deliver smoke, bounce/complaint handling → greens `native_email_sequences.v1`).
5. **Unified two-channel send abstraction** (channel selection + suppression/preferences/unsubscribe across both).
6. **Wire chat → Blueprint `send_email{channel}`** + approval surface; executor routes to Resend or the local Gmail executor.
7. **Autopilot Desktop CRM pane** (contacts/activities read model) + integrated local Gmail executor.

Each step: full tests + `check:deploy` green; promises move only with dereferenceable receipts + owner sign-off; no faked greens.

---

## 5. Gap analysis

| # | Gap | Severity |
|---|---|---|
| 1 | No contact/account/activity CRM model in D1 | **High** (blocks all) |
| 2 | Contacts not imported into D1 | **High** |
| 3 | Gmail/`gws` channel not ported into the new CRM (Laravel-coupled + no write-back) | **High** (owner wants Gmail tomorrow) |
| 4 | Resend transactional send inert + unproven | **High** |
| 5 | Chat → Blueprint `send_email{channel}` not wired | **High** for "automated in chat" |
| 6 | Two-channel abstraction + tenant scoping must be designed in | Medium |
| 7 | Suppression/unsubscribe enforced across **both** channels | Medium (compliance) |
| 8 | Resend sending-domain reputation / SPF-DKIM-DMARC | Medium (deliverability) |
| 9 | Gmail send-as-owner write-back to CRM ledger | Medium |
| 10 | Reply tracking (inbound→activity) | Low (later) |

---

## 6. Plan

**Sprint A — ~150 tomorrow, dual-channel, on our infra (proves both channels):**
1. (Owner) Hand off the list (CSV) + confirm which segment goes Gmail vs Resend.
2. (Me) Import → D1, de-dupe, report exact count + sample.
3. (Me) Port the `gws` Gmail sender to our CRM (+ write-back, draft-first); confirm `gws` is authed as the owner's mailbox.
4. (Me) Verify Resend secrets + domain; arm the transactional sender; live deliverability smoke.
5. (You) Review rendered copy on real contacts (Gmail drafts + a Resend test).
6. (Me) Send in waves across both channels; suppression honored; all sends in the D1 ledger; greens `native_email_sequences.v1` if receipts hold.

**Sprint B — automated CRM (build order §4 steps 1→7):** schema → import → Gmail channel → proven Resend → unified two-channel → chat/Blueprint wiring → desktop pane. Tenant-scoped throughout.

**Owner-gated (mirrored to `NEEDS_OWNER.md`):** the list/segment + Gmail-vs-Resend split + CSV handoff; which **Gmail identity** `gws` sends as; the Resend **sending domain** verification; final copy/subject; preview review before the live blast.

---

## 7. Risks

- **Deliverability is the timeline risk.** Resend: unverified/cold domain → spam. Gmail: unwarmed mailbox / over-quota → throttle. Verify both, wave-send, real unsubscribe, suppression honored on **both** channels.
- **Multi-tenant isolation.** Contact data strictly tenant-scoped from the first migration — no cross-tenant leakage.
- **Gmail OAuth stays local.** Never put the owner's Google credentials in a Worker; the Gmail channel is a local/desktop executor by design.
- **No-resale / identity rules.** Outreach copy + automation respect the standing no-resale (subscription-scoped) + public-identity rules.
- **Don't trust "wired" without the smoke.** Bindings ≠ proven delivery.
- **Capture the send record.** Every send (both channels) writes to the D1 ledger — fix the Gmail write-back gap the old script had.

---

## 8. Immediate next actions

1. **(Owner)** Answer the gating items in `NEEDS_OWNER.md`.
2. **(Me, on go)** PR #1: tenant-scoped D1 contact-CRM schema + contracts + read APIs.
3. **(Me)** One-time CSV import → D1; report counts.
4. **(Me)** Port the `gws` Gmail channel (our CRM + write-back); arm + prove Resend; then the Sprint A dual-channel send.
5. **(Me)** Unified two-channel abstraction → chat/Blueprint `send_email{channel}` → desktop CRM pane.

Tracked as a GitHub epic + sub-issues (see `NEEDS_OWNER.md` / the epic link).

---

### Appendix — key file references

- **Build here:** `apps/openagents.com/workers/api/migrations/{0026,0063,0064,0072,0081,0088,0181,0191,0193,0216}_*.sql`, `workers/api/src/{email,email-campaign-dispatcher,email-sequence-send-service,resend-webhooks,email-preferences}.ts`, `config.ts:185-188`, `wrangler.jsonc:44-45`, `blueprint-routes.ts` (send_email), `product-promises.ts` (`:211`, `:2598`).
- **Gmail channel to port in:** `scripts/crm-gmail.sh`, `/Users/christopherdavid/code/googleworkspace-cli` (`gws`), `docs/2026-04-30-gws-gmail-crm-runbook.md`.
- **Schema reference only (not a runtime):** `autopilot3/convex/{schema,crmAdmin,crmContactCommands,crmSourceImport}.ts`; deprecated Laravel `crm_*` migrations + ability model.
- **Not used as a runtime:** the Laravel CRM API, Convex.
</content>
