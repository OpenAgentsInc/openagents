# CRM State-of-the-World + Cloudflare Cutover Audit

**Date:** 2026-06-22
**Author:** Raynor (agent)
**Trigger:** Owner wants to (a) connect "our CRM" to Autopilot Desktop / chat for *automated* CRM, probably through the Blueprint action system, and (b) contact ~150 people tomorrow with automated email. This audit inventories every place CRM/outreach code and data live across the workspace, states honestly what works today, and lays out a two-track plan: the tactical "150 tomorrow" send and the real chat/Blueprint-driven CRM on Cloudflare.

> **Scope note / repo routing.** The product surface owner is `apps/openagents.com/` in this repo. The full historical CRM lives in the **deprecated Laravel** clone at `~/work/deprecated/openagents.com/` and in **`autopilot3`** (Convex). Per the workspace contract, new product CRM work belongs on the Cloudflare/Effect stack in `apps/openagents.com/`, **not** in Convex (`autopilot3`) and **not** by reviving the Laravel app.

---

## 0. TL;DR — the one decision that matters

We are sitting on **three different CRMs** and they do not agree:

| Surface | Has the full CRM data model? | Has the real contact DATA? | Can send live email? | On the chosen stack? |
|---|---|---|---|---|
| **Deprecated Laravel** (`deprecated/openagents.com`) | ✅ Yes (11 tables, full investor CRM) | ✅ **Yes — this is where the real people are** (live prod DB) | ✅ Yes (Laravel Mail; also via Gmail tool) | ❌ No (deprecated) |
| **autopilot3** (Convex) | ✅ Yes (near-exact port of the Laravel model) | ⚠️ Schema only, no checked-in data | ⚠️ Record-only (no live provider wired) | ❌ No (Convex — wrong stack) |
| **Current Cloudflare** (`apps/openagents.com`) | ❌ **No contact/account/opportunity model** — only campaigns, subscribers, prospects, signups | ❌ No | ⚠️ **Resend bindings exist but the send path is INERT (default-OFF flag, not chained to the dispatcher) and deliverability is UNPROVEN** | ✅ **Yes** |

**The gap in one sentence:** the stack we want to build on (Cloudflare) has the *email plumbing* but **not the contact model and not the data**; the place that has the *contacts and the model* (Laravel) is deprecated; and the most complete *port* of the model (autopilot3) is on the wrong runtime (Convex).

**Therefore the honest answer to "automated CRM in chat via Blueprint, on Cloudflare" is: it is a real build, not a flip-a-flag.** It requires porting the contact/account/activity model into D1, arming + proving the Resend send path, and wiring the chat→Blueprint `send_email` action. None of those three exist green today.

**And the honest answer to "150 people tomorrow" is: yes, but via the tactical Laravel+Gmail path (Track 1 below), not via the not-yet-built Cloudflare chat CRM.**

---

## 1. Can we actually email 150 people tomorrow?

**Yes — through the existing local Gmail draft/send tool against the live Laravel CRM.** This is the only path where *both* the model and the real contacts already exist and a sender is proven.

- Tool: `~/work/scripts/crm-gmail.sh` (present, executable, dated 2026-04-30).
- Sender: `gws` (Google Workspace CLI) at `/Users/christopherdavid/code/googleworkspace-cli` (present).
- Data source: production Laravel CRM at `OPENAGENTS_COM_CRM_BASE_URL` (= `https://openagents.com`, still the Laravel backend for `/api/admin/crm/*`).
- Auth: `~/work/.secrets/openagents-com-crm-production.env` (present; defines `OPENAGENTS_COM_CRM_BASE_URL`, `OPENAGENTS_COM_CRM_OPERATOR_EMAIL`, `OPENAGENTS_COM_CRM_TOKEN` — values not printed).
- Runbook: `~/work/docs/2026-04-30-gws-gmail-crm-runbook.md`.

**Hard constraints that decide tomorrow's plan:**

1. **Draft-first by default.** The tool creates Gmail drafts unless `--send` is passed. Good safety default; review before blast.
2. **Single contact per invocation.** No bulk endpoint; 150 people = a wrapper loop of 150 calls (each: resolve contact → render template via CRM preview → create draft/send).
3. **Gmail quota is the real ceiling.** A standard Google Workspace user is ~**500 external recipients/day** (consumer Gmail ~100/day). 150 in one day is fine for a Workspace account *with sending history*; a cold account risks throttling/spam-foldering. **Verify which account `gws` is authed as before blasting.**
4. **No write-back of the Gmail send into the CRM** today. Sending via the Gmail tool does **not** record a `crm_activity`/`crm_email_message` row automatically — so "who did we contact" is not captured unless we add it. (Sending via the Laravel CRM's own `POST /api/admin/crm/contacts/{id}/emails` endpoint *does* record it, but that uses the Laravel Mail driver, whose provider config must be verified — see §2.A.)
5. **Deliverability/compliance.** 150 cold-ish outbound emails needs: a real unsubscribe affordance, suppression honored, SPF/DKIM/DMARC aligned for the sending domain, and respect for the no-resale/identity rules. Drafts-first + manual review covers tomorrow; automation needs suppression wired (see §5).

→ **Track 1 in §6** is the concrete tomorrow plan. **Owner decisions needed** are flagged there and mirrored to `NEEDS_OWNER.md`.

---

## 2. Inventory by surface

### 2.A — Deprecated Laravel CRM (`~/work/deprecated/openagents.com`) — *the source of truth for data*

A complete, production-grade, investor-oriented CRM. **This is where the real people and the real model are.** It is the backend the Gmail tool already talks to.

**Data model (11 core tables, all migrated 2026-04-29):**
- `crm_contacts` — primary_email (unique), names, job_title, `contact_type` (default `investor`), `relationship_stage`, `workos_user_id`, `portal_access_status`, engagement timestamps, `engagement_score`, `account_id`, owner.
- `crm_accounts` — orgs/funds (name, domain, account_type, website).
- `crm_contact_lists` + `crm_contact_list_memberships` — segmentation. **Seeded system lists:** `investor_portal_approved`, `investor_roster`.
- `crm_email_templates` — markdown+HTML templates with `available_variables`. **Seeded template:** `investor-portal-follow-up`.
- `crm_email_messages` + `crm_email_deliveries` — send ledger + provider delivery records (status, opened_at, clicked_at, replied_at, provider ids).
- `crm_activities` — audit log (email_sent, portal_view/click/login, manual_touch), dedup by `(source_record_type, source_record_id)`.
- `crm_engagement_snapshots` — cached 30/90-day rollups + engagement score.
- `crm_opportunities` + `crm_opportunity_contact_roles` — deal pipeline.
- `crm_writeback_requests` — Blueprint writeback audit (idempotency, approval, rollback posture).
- Legacy `investor_accesses` — older flat table; **synced-on-read** into `crm_*` via `CrmInvestorSyncService`.

**API (`routes/api.php`, `/api/admin/crm/*`):** full read set (`crm:read`), write set (`crm:write`), email send (`crm:email:send`), Blueprint source-export (`crm:export`, server-to-server, with data-classification + field redaction + cursor pagination, schema `2026-05-08.crm-source-export.v1`), and Blueprint writeback (`crm:writeback`). Auth is **Sanctum tokens with granular abilities**; admin web UI at `/admin/crm/*` (`auth` + `approved:admins`).

**Email send path:** `CrmTransactionalEmailService` → Laravel `Mail` facade → `MAIL_MAILER` driver (supports smtp/ses/postmark/resend/log). **⚠️ Verify the live prod `MAIL_MAILER`** — local default is `log` (no real send). Records a `crm_email_message` + `crm_email_delivery` + `email_sent` activity on each send.

**Operator tooling:** `php artisan crm:issue-operator-token` mints scoped Sanctum tokens.

**Decommission intent already on record:** `docs/investor-crm-admin-decommission-plan.md` plus an `OPENAGENTS_CRM_ADMIN_MODE` (`transitional`→`read_only`→`redirect`) switch — i.e., this CRM was *always meant to move off Laravel*. That target is now Cloudflare (this audit), superseding the old "Autopilot CRM" redirect URL.

**Current DB state in the local checkout:** schema-complete, system lists + 1 template seeded, **0 contact rows locally** — meaning the real contacts live in the **production** Laravel DB, reachable via the CRM API (`GET /api/admin/crm/contacts`) or source-export, **not** as a checked-in file anywhere.

### 2.B — Current Cloudflare `apps/openagents.com` — *the email plumbing, no contact model*

This is the stack we will build on. What exists today (verified against `workers/api/migrations/` and `config.ts`):

**Email/outreach migrations present:**
`0026_email_ledger.sql` (email_messages, email_deliveries, email_drafts, email_provider_events), `0063_email_campaign_records.sql` (campaigns, steps, enrollments, sends, **suppression_entries**, **preferences**), `0064_email_campaign_dispatch_attempts.sql`, `0072_targeted_site_outreach.sql` (targeted_site campaigns + **prospects**), `0081`/`0088` (outreach dispatches + metrics), `0181_native_lists_subscribers.sql` (`subscriber_lists`, `list_subscribers`), `0191`/`0216` (`business_signup_requests` + referral attribution), `0193_cloudflare_email_provider.sql` (a Cloudflare Email Routing provider alongside Resend).

**Email provider:** **Resend** is the configured provider. Bindings exist in `config.ts:185-188` (`RESEND_API_KEY`, `RESEND_FROM_EMAIL`, `RESEND_REPLY_TO_EMAIL`, `RESEND_WEBHOOK_SECRET`) and `wrangler.jsonc:44-45` (`RESEND_FROM_EMAIL = "OpenAgents <chris+sites@openagents.com>"`, reply-to `chris+sites@openagents.com`). Webhook receiver `resend-webhooks.ts` verifies Svix signatures and auto-writes suppression on bounce/complaint. A Cloudflare-Email provider also exists (migration 0193).

**⚠️ The critical honesty correction to the "it's all wired" read:** the send path is **deliberately INERT**.
- `email-sequence-send-service.ts` plans a **dry-run** and **never calls the sender** unless armed by the default-OFF **`EMAIL_SEQUENCE_SEND_ENABLED`** flag, and is **NOT chained into the live dispatcher** (it sends no live email).
- The public lead-capture route is mounted but behind default-OFF **`SITE_FORM_CAPTURE_ENABLED`**.
- The promise `autopilot_sites.native_email_sequences.v1` is **YELLOW**, and its **one remaining blocker is `email_deliverability_unproven`** — i.e., **there is no proven live send→deliver evidence on Cloudflare yet** (`product-promises.ts:211`, `:2598`).
- Whether `RESEND_API_KEY` / `RESEND_WEBHOOK_SECRET` are actually set as prod Worker secrets, and whether the sending domain is verified in Resend, is **unverified** and must be checked before any Cloudflare send.

**What's missing entirely on Cloudflare:** a **contact / account / contact-list / activity / engagement / opportunity** model. The D1 tables are *campaign- and subscriber-* shaped (audience lists, enrollments, sends, prospects, signups), **not** the relationship-CRM shape that Laravel/autopilot3 have. So "port the CRM to Cloudflare" = real schema + service work, not a data copy onto an existing model.

**Blueprint action system:** `blueprint-routes.ts` already recognizes a **`send_email`** effect kind (alongside deploy, create_pull_request, spend_money, post_public_claim, etc.). Flow = program proposes effect → stored in `blueprint_action_submissions` (pending_approval) → operator approval → execute. **But there is no wiring from the chat message handler to propose a `send_email` action**, and the executor→sender hookup is not armed. So "automated CRM in chat via Blueprint" has the *frame* but not the *connection*.

**Relevant promises:** `autopilot_sites.native_email_sequences.v1` (YELLOW), `workrooms.source_authorized_business_objects.v1` (RED — typed model + approval-gated write engine exist but `effectsApplied` is always false / INERT), `workrooms.omni_client_delivery_workrooms.v1`, and the `omni-crm-follow-up-workrooms.ts` reference (RED). These are the registry homes any future green CRM claim must go through.

### 2.C — autopilot3 (Convex) — *the most complete model port, wrong runtime*

`autopilot3/convex/schema.ts` defines a near-exact port of the Laravel CRM (crmAccounts, crmContacts, crmContactLists/Memberships, crmActivities, crmEmailTemplates, crmEmailMessages/Deliveries, crmEngagementSnapshots, crmOpportunities/Roles) **plus** extras worth stealing: `crmContactCommands` (approval-gated mutations), `crmSourceImportRuns` (bulk-import audit), `crmWritebackRequests`. Functions in `convex/crmAdmin.ts` (`sendEmail`, `recordEmailEvent`, `listEmailMessages`), `crm.ts`, `crmContactCommands.ts`, `crmSourceImport.ts`. API routes mirror the Sanctum scopes (`crm:read|write|email:send|export|writeback`). Email is **record-only** (generic `providerName`, no live provider).

**Use:** the **best design reference** for the D1 port — especially the *command/approval* and *source-import* tables, which map cleanly onto the Blueprint action model. Do **not** adopt Convex as the runtime (contract says Cloudflare/Effect).

### 2.D — autopilot-omega / vortex / autopilot2 / autopilot-deprecated

- **autopilot-omega:** essentially the *same* Resend campaign/suppression/preference infra that now lives in `apps/openagents.com` (shared migration lineage `0026`/`0063`, `resend-webhooks.ts`, `email-campaign-dispatcher.ts`, `packages/email-templates`). Predecessor/sibling, not additive. Has `docs/2026-06-04-previous-resend-gmail-email-systems-audit.md` worth a read.
- **vortex:** scaffolding only — a `lastResendEmailId` on prelaunch access and a planned "crm" subsystem literal; **no CRM tables**.
- **autopilot2 / autopilot-deprecated:** no real CRM (voice/UI experiments only).

### 2.E — Local Gmail tooling (`scripts/crm-gmail.sh` + `gws`)

Already covered in §1. Net: a **working, draft-first, single-contact** sender against the **live Laravel CRM**, no CRM write-back, Gmail-quota bound. This is Track 1's engine.

---

## 3. Where the contact data actually lives

- **Authoritative store:** the **production Laravel DB** behind `https://openagents.com` (`crm_contacts`, `crm_accounts`, `crm_contact_lists`, legacy `investor_accesses`). Reachable via the CRM API with the production Sanctum token.
- **Not** checked in anywhere as CSV/JSON/SQL — no contact export files exist in the workspace (all four sweeps confirm). Local Laravel checkout has 0 contact rows.
- **Likely "the 150":** the `investor_roster` / `investor_portal_approved` lists, or a `contact_type=investor` filter. **Exact count is unknown from docs** — must query `GET /api/admin/crm/contacts?...` (or source-export) to get the real list and number.

**Action:** the first concrete step tomorrow is to **pull the contact list to a local working file** (via the export endpoint) so we know exactly who/how-many and can review before any send.

---

## 4. How the pieces *should* connect (target architecture)

The owner's ask — "automated CRM, in chat, via Blueprint, on Cloudflare, reflected in Autopilot Desktop" — maps onto components that mostly already have homes:

```
Autopilot Desktop / chat UI
        │  (natural-language intent: "follow up with the 12 investors who opened but didn't reply")
        ▼
Blueprint program  ──proposes──►  send_email action  (blueprint_action_submissions, pending_approval)
        │                                   │
        │                          operator approval (chat-surfaced)
        ▼                                   ▼
CRM read model (D1: contacts/      Resend send  ──►  email_messages / email_deliveries (D1)
accounts/lists/activities)                  │
        ▲                                   ▼
        └────────── webhook (resend-webhooks.ts) ──► provider_events + suppression + activity write-back
```

**Build order (no Laravel revival, no Convex):**
1. **Port the contact CRM model into D1** in `apps/openagents.com` — `crm_contacts`, `crm_accounts`, `crm_contact_lists(+memberships)`, `crm_activities`, `crm_engagement_snapshots`, `crm_opportunities(+roles)`. Reuse the autopilot3/Laravel column shapes; add the `crm_contact_commands` + `crm_source_import_runs` ideas. Effect Schema contracts for each row type.
2. **One-time data import** from the Laravel source-export (`/api/admin/crm/source-export/contacts|accounts|...`) into D1. The Laravel export endpoint already exists and is classification-aware — this is the clean migration path.
3. **Arm + prove the Resend send path** for `crm_transactional`: verify prod secrets + domain, flip `EMAIL_SEQUENCE_SEND_ENABLED` semantics into a real transactional sender, run a **live send→deliver smoke** with bounce/complaint handling (this is exactly the `email_deliverability_unproven` blocker — clearing it greens the promise honestly, receipt-first).
4. **Wire chat→Blueprint `send_email`** — chat message handler proposes a `send_email` action with `{recipient contact ref, template, context}`; operator approves in-UI; executor calls the transactional sender; webhook writes the `crm_activity` + delivery back. This is the "automated CRM" the owner wants, with the approval gate built in.
5. **Reflect in Autopilot Desktop** — contact/activity read model surfaced in the desktop CRM pane (and, per the Verse direction, optionally as world reflections later — out of scope here).

This is also the *correct* execution of the long-standing Laravel→off-Laravel decommission plan (§2.A), just pointed at Cloudflare instead of the old "Autopilot CRM" URL.

---

## 5. Gap analysis — what's missing for *automated* CRM

| # | Gap | Where | Severity |
|---|---|---|---|
| 1 | No contact/account/activity CRM model on Cloudflare | `apps/openagents.com` D1 | **High** (blocks everything) |
| 2 | Contacts not migrated off Laravel prod | Laravel → D1 | **High** |
| 3 | Live Resend send unproven + INERT (flag off, not chained, domain/secrets unverified) | `apps/openagents.com` | **High** |
| 4 | Chat→Blueprint `send_email` not wired | chat handler ↔ blueprint-routes | **High** for "automated in chat" |
| 5 | Gmail tool doesn't write back sends to CRM | `scripts/crm-gmail.sh` | Medium (lose contact history tomorrow) |
| 6 | No bulk/batch send endpoint | both stacks | Medium (loop works tomorrow) |
| 7 | Unsubscribe/suppression not enforced on the tactical path | Track 1 | Medium (compliance) |
| 8 | Reply tracking not wired (no inbound→activity) | both | Low (later) |
| 9 | Sending-domain reputation / SPF-DKIM-DMARC for a 150 blast | infra | Medium (deliverability) |

---

## 6. The plan — two tracks

### Track 1 — Tomorrow: send ~150 (tactical, Laravel + Gmail)

1. **Pull the list.** `GET /api/admin/crm/contacts` (or source-export) with the prod token → save locally → **confirm exactly who and how many** (the "150"). Filter to the intended segment (likely `investor_roster`).
2. **Confirm/author the template.** Reuse `investor-portal-follow-up` or author a new one via the CRM template API; render a preview for 2–3 real contacts and eyeball it.
3. **Confirm the Gmail identity + quota.** Verify which account `gws` is authed as (`gws auth ...`) and that it can take ~150 external/day. If it's a cold/consumer account, **split across days or use a warmed Workspace account.**
4. **Generate drafts first.** Loop `crm-gmail.sh --contact-id <id> --template ... --draft` over the list (small sleep between calls). Review the drafts in Gmail.
5. **Send in waves** with delays; monitor bounces.
6. **Capture who we contacted.** Because the Gmail path doesn't write back, either (a) send via the Laravel CRM `POST .../emails` endpoint instead (records activity, *if* prod `MAIL_MAILER` is a real driver — verify), or (b) keep the local list as the record and backfill activities later. **Recommend (a) if the prod mailer is real**, else (b).

**Owner-gated for Track 1 (mirrored to `NEEDS_OWNER.md`):**
- Which segment = "the 150"? (investor_roster? a new list? a CSV you'll hand me?)
- Which Gmail/Workspace identity sends, and is it warmed for ~150/day?
- Final email copy/template + subject (no copy changes without your sign-off).
- Draft-first review by you before any live send? (recommended: yes.)

### Track 2 — The real thing: chat/Blueprint-driven CRM on Cloudflare

Build order = §4 steps 1→5. Sequencing recommendation:
- **First PR:** D1 contact CRM schema + Effect Schema contracts + read APIs (step 1). No behavior change, no flips.
- **Second PR:** Laravel→D1 source import (step 2), run once, verify counts.
- **Third PR:** arm + **prove** the Resend transactional send (step 3) — this is the honest path to greening `native_email_sequences.v1` (receipt-first, owner-signed).
- **Fourth PR:** chat→Blueprint `send_email` wiring + approval surface (step 4).
- **Fifth:** desktop CRM pane (step 5).

Each lands with full tests + `check:deploy` green; no faked greens; promises move state only with dereferenceable receipts + owner sign-off.

---

## 7. Risks / things to not screw up

- **Deliverability of a 150 blast.** Cold domain/account → spam folder + reputation damage. Warm identity, real unsubscribe, suppression honored, SPF/DKIM/DMARC aligned for whatever From-domain we use.
- **No-resale / identity rules.** Outreach copy and any agent automation must respect the standing no-resale (subscription-scoped) and public-identity rules. Don't auto-send anything that makes a claim we can't back.
- **Don't revive Laravel as the go-forward.** It is the *data source* for a one-time import only; the contract says build on Cloudflare/Effect.
- **Don't trust "it's wired" without the smoke.** Resend bindings ≠ proven delivery. The YELLOW promise's remaining blocker is exactly this — treat it as unproven until a live send→deliver receipt exists.
- **Capture the send record.** If tomorrow's send leaves no CRM activity trail, we lose "who did we contact / who replied," which is the whole point of a CRM.

---

## 8. Immediate next actions

1. **(Owner)** Answer the four Track-1 gating questions in `NEEDS_OWNER.md`.
2. **(Me, on owner's go)** Pull + de-dupe the contact list from prod, report exact count + a sample, render template previews.
3. **(Me)** Stand up Track-1 draft generation; you review drafts; we send in waves.
4. **(Me, Track 2)** Open the first PR: D1 contact-CRM schema + contracts + read APIs in `apps/openagents.com`, using the autopilot3/Laravel model as reference. Then the Laravel→D1 import, then the proven Resend send, then the chat→Blueprint wiring.

---

### Appendix — key file references

- Laravel CRM: `deprecated/openagents.com/database/migrations/2026_04_29_*`, `app/Http/Controllers/Api/AdminCrm*Controller.php`, `app/Services/CrmTransactionalEmailService.php`, `app/Console/Commands/CrmIssueOperatorTokenCommand.php`, `docs/investor-crm-*.md`.
- Cloudflare email infra: `apps/openagents.com/workers/api/migrations/{0026,0063,0064,0072,0081,0088,0181,0191,0193,0216}_*.sql`, `workers/api/src/{email,email-campaign-dispatcher,email-sequence-send-service,resend-webhooks,email-preferences}.ts`, `workers/api/src/config.ts:185-188`, `workers/api/wrangler.jsonc:44-45`, `workers/api/src/blueprint-routes.ts` (send_email), `workers/api/src/product-promises.ts` (`:211`, `:2598`, `:1464`, `:2563`).
- autopilot3 Convex reference: `autopilot3/convex/{schema,crm,crmAdmin,crmContactCommands,crmSourceImport}.ts`.
- Local tooling: `scripts/crm-gmail.sh`, `/Users/christopherdavid/code/googleworkspace-cli`, `docs/2026-04-30-gws-gmail-crm-runbook.md`, `docs/2026-04-29-*crm*.md`.
- Secrets (names only): `.secrets/openagents-com-crm-production.env`, `.secrets/openagents-com-crm-local.env`.
</content>
</invoke>
