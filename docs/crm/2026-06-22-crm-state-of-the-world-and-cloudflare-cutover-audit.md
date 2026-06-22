# CRM State-of-the-World + Native Cloudflare Build Audit

**Date:** 2026-06-22
**Author:** Raynor (agent)
**Trigger:** Owner wants to (a) connect "our CRM" to Autopilot Desktop / chat for *automated* CRM, through the Blueprint action system, and (b) contact ~150 people tomorrow with automated email.

> **Owner mandate (2026-06-22):** **No Laravel CRM. Do not suggest it as a runtime, an API we keep calling, or a tactical sender.** We are moving **all relevant CRM/outreach code into `apps/openagents.com` (and related Cloudflare surfaces)** and building it **multi-tenant, so our customers use the exact same CRM/outreach infrastructure we built for ourselves.** Convex (`autopilot3`) is likewise not the runtime. The old Laravel app and the autopilot3 Convex CRM are **schema/design references only** — we copy the *model shapes*, never the runtime.

---

## 0. TL;DR — the decision

**Build one CRM, natively, in `apps/openagents.com` on Cloudflare/Effect + D1, multi-tenant from day one.** Same contact/outreach engine powers our own outreach *and* every customer's. Nothing runs on Laravel or Convex.

What that means concretely, because the Cloudflare side is **not** turnkey today:

| Layer | Status on Cloudflare today | Work to do |
|---|---|---|
| **Email plumbing** (Resend provider, message/delivery ledger, campaigns, suppression, preferences, webhooks) | ✅ **Exists** in `apps/openagents.com` | Arm + **prove** it (it's currently inert behind a default-OFF flag and deliverability is unproven) |
| **Contact CRM model** (contacts, accounts, lists, activities, engagement, opportunities) | ❌ **Does not exist** — only campaign/subscriber/prospect tables | **Build it in D1**, tenant-scoped, using the autopilot3/Laravel shapes as reference |
| **The actual contacts (the ~150)** | ❌ Not on Cloudflare | **One-time import** into D1 from whatever export the owner provides (see §3) |
| **Chat → Blueprint `send_email`** | ⚠️ Blueprint recognizes the `send_email` effect kind, but chat doesn't propose it and the executor isn't armed | **Wire it** (chat proposes → approval → executor → send → write-back) |

**Honest bottom line:** "automated CRM in chat, on our infra" is a real build with four pieces, none green today. The good news: three of them (email ledger, campaign/suppression infra, Blueprint `send_email` frame) are already scaffolded in `apps/openagents.com` — we're finishing and arming, not starting from zero. The ~150 send tomorrow becomes the forcing function that proves the send path (and honestly greens `autopilot_sites.native_email_sequences.v1`).

---

## 1. The ~150 tomorrow — on our own infra

We do **not** fall back to Laravel+Gmail. We send through **our Worker (`apps/openagents.com`) via Resend**, which also dogfoods the exact path customers will use. That requires a focused sprint of three things:

1. **Get the contacts into D1.** One-time import (see §3). Owner hands off the list in whatever form is cleanest — a CSV export is ideal and keeps us off any legacy API.
2. **Arm + prove the Resend transactional send.** The send service exists but is INERT (default-OFF flag, not chained to the dispatcher) and deliverability is unproven (§2.B). Arming it + running a live send→deliver smoke with bounce/complaint handling is exactly the `email_deliverability_unproven` blocker — clearing it honestly greens the promise.
3. **Send 150 through the Worker** with suppression honored + a real unsubscribe affordance + the send recorded in the D1 ledger (so we capture who we contacted / who replied — the whole point of a CRM).

**Deliverability reality (decides whether tomorrow is realistic):** a 150-recipient send needs a **verified sending domain in Resend** with aligned SPF/DKIM/DMARC, and a warmed reputation. `RESEND_FROM_EMAIL` is currently `OpenAgents <chris+sites@openagents.com>`. **Must verify** the prod Worker has `RESEND_API_KEY`/`RESEND_WEBHOOK_SECRET` set and the from-domain is verified before any blast. If the domain isn't warmed, send in waves and expect some spam-foldering. This is the one place tomorrow's timeline could slip — flagged for the owner in §6.

---

## 2. Inventory by surface

### 2.A — Deprecated Laravel CRM — *schema reference + one-time data source ONLY*

The old Laravel app has a complete, production investor CRM (11 tables: `crm_contacts`, `crm_accounts`, `crm_contact_lists(+memberships)`, `crm_email_templates`, `crm_email_messages(+deliveries)`, `crm_activities`, `crm_engagement_snapshots`, `crm_opportunities(+roles)`, `crm_writeback_requests`; legacy `investor_accesses`). It also has the granular Sanctum ability model (`crm:read|write|email:send|export|writeback`), an approval-gated Blueprint writeback, and a classification-aware source-export.

**Per the owner mandate, this is reference material only:**
- ✅ **Use:** copy the *column shapes* and the *ability/approval concepts* into the D1 model.
- ✅ **Use (once):** if the owner chooses to pull the existing contacts out of the old prod DB, the source-export is one mechanism — but a plain CSV handoff is preferred so we take **zero** ongoing Laravel dependency.
- ❌ **Do not:** run it, call its API as a live CRM, route new work to it, or use it as the tactical sender. It is being decommissioned (its own `docs/investor-crm-admin-decommission-plan.md` always intended this — now the target is Cloudflare, not the old "Autopilot CRM" URL).

The real contacts currently live in the **old prod DB**, not as a checked-in file. Extracting them is a **one-time migration**, after which the old CRM is dead to us.

### 2.B — `apps/openagents.com` (Cloudflare) — *the foundation we build on*

Verified against `workers/api/migrations/` and `config.ts`:

**Email/outreach already present:**
`0026_email_ledger.sql` (email_messages, email_deliveries, email_drafts, email_provider_events), `0063_email_campaign_records.sql` (campaigns, steps, enrollments, sends, **suppression_entries**, **preferences**), `0064` (dispatch attempts), `0072` (targeted_site campaigns + **prospects**), `0081`/`0088` (outreach dispatches + metrics), `0181_native_lists_subscribers.sql` (`subscriber_lists`, `list_subscribers` — already **owner/team-scoped**, good multi-tenant precedent), `0191`/`0216` (`business_signup_requests` + referral attribution), `0193_cloudflare_email_provider.sql` (a Cloudflare-Email provider alongside Resend).

**Provider:** **Resend**. Bindings in `config.ts:185-188` (`RESEND_API_KEY`, `RESEND_FROM_EMAIL`, `RESEND_REPLY_TO_EMAIL`, `RESEND_WEBHOOK_SECRET`) + `wrangler.jsonc:44-45`. Webhook receiver `resend-webhooks.ts` verifies Svix signatures and auto-writes suppression on bounce/complaint.

**⚠️ The send path is deliberately INERT (this is the real work):**
- `email-sequence-send-service.ts` plans a **dry-run** and **never calls the sender** unless armed by the default-OFF **`EMAIL_SEQUENCE_SEND_ENABLED`** flag, and is **not chained into the live dispatcher**.
- `autopilot_sites.native_email_sequences.v1` is **YELLOW**; its one remaining blocker is **`email_deliverability_unproven`** — i.e., **no proven live send→deliver evidence on Cloudflare yet** (`product-promises.ts:211`, `:2598`).
- Prod secret presence + sending-domain verification in Resend: **unverified** (§1).

**Missing entirely:** the **contact / account / list / activity / engagement / opportunity** relationship model. Today's D1 tables are *campaign/subscriber/prospect* shaped, not relationship-CRM shaped. So "the CRM" is net-new schema + services here — built tenant-scoped so customers get it too.

**Blueprint:** `blueprint-routes.ts` already recognizes a **`send_email`** effect kind (alongside deploy, spend_money, post_public_claim, …): program proposes effect → `blueprint_action_submissions` (pending_approval) → operator approval → execute. **Not wired** from chat, and the executor→sender hookup isn't armed. The frame exists; the connection doesn't.

**Promise homes** any green CRM claim must route through: `autopilot_sites.native_email_sequences.v1` (YELLOW), `workrooms.source_authorized_business_objects.v1` (RED, INERT), `workrooms.omni_client_delivery_workrooms.v1`, `omni-crm-follow-up-workrooms.ts` (RED).

### 2.C — autopilot3 (Convex) — *best model reference, not the runtime*

`autopilot3/convex/schema.ts` is a near-exact port of the Laravel CRM **plus** three ideas worth copying into D1: `crmContactCommands` (approval-gated mutations — maps cleanly onto Blueprint), `crmSourceImportRuns` (bulk-import audit), `crmWritebackRequests`. Functions: `crmAdmin.ts` (`sendEmail`, `recordEmailEvent`), `crmSourceImport.ts`. **Reference only** — we are not on Convex.

### 2.D — autopilot-omega / vortex / others

`autopilot-omega` = the same Resend campaign/suppression infra that now lives in `apps/openagents.com` (shared `0026`/`0063` lineage) — not additive. `vortex` = scaffolding only (no CRM tables). `autopilot2`/`autopilot-deprecated` = no real CRM. Nothing to adopt; `apps/openagents.com` is already the consolidated home.

### 2.E — Local Gmail tooling (`scripts/crm-gmail.sh` + `gws`)

Tied to the Laravel CRM and Gmail. **Not part of the go-forward plan** per the mandate. Leaving it as-is, unused for this work.

---

## 3. The contacts — one-time import, then never look back

- The ~150 (and the rest) currently live in the **old prod DB**, not as a file.
- **Preferred handoff:** owner provides a **CSV** (or points me at one). We import once into D1. Zero ongoing legacy dependency.
- **Alternate (one-time only):** a single read of the old source-export to seed D1, then decommission.
- We'll **de-dupe, normalize, and report the exact count + a sample** before any send.
- Import is audited (`crm_source_import_runs` idea from autopilot3) so the migration is traceable.

---

## 4. Target architecture (all in `apps/openagents.com`, multi-tenant)

```
Autopilot Desktop / chat UI  (NL intent: "follow up with investors who opened but didn't reply")
        ▼
Blueprint program ──proposes──► send_email action ──► blueprint_action_submissions (pending_approval)
        │                                   │
        │                          operator approval (chat-surfaced)
        ▼                                   ▼
CRM read model (D1, tenant-scoped:   Resend send ──► email_messages / email_deliveries (D1)
contacts/accounts/lists/activities)         │
        ▲                                   ▼
        └────── webhook (resend-webhooks.ts) ──► provider_events + suppression + activity write-back
```

**Multi-tenant from the start:** every CRM table carries owner/team scope (follow the existing `subscriber_lists` precedent), so the same engine serves OpenAgents' own outreach and each customer's, with isolation. This is the product, not just an internal tool.

**Build order (native; no Laravel, no Convex):**
1. **D1 contact-CRM schema + Effect Schema contracts**, tenant-scoped: contacts, accounts, lists(+memberships), activities, engagement_snapshots, opportunities(+roles), contact_commands, source_import_runs. Read APIs. No behavior change.
2. **One-time contact import** into D1 (§3); verify counts.
3. **Arm + prove the Resend transactional send** for `crm_transactional`: verify prod secrets + domain, make the sender live behind a real (not dry-run) path, run the live send→deliver smoke with bounce/complaint handling. Greens `native_email_sequences.v1` receipt-first.
4. **Wire chat → Blueprint `send_email`**: chat proposes the action with `{tenant, contact ref, template, context}`; operator approves in-UI; executor calls the sender; webhook writes activity + delivery back.
5. **Surface in Autopilot Desktop**: contact/activity read model in the desktop CRM pane.

Each step lands with full tests + `check:deploy` green; promises move only with dereferenceable receipts + owner sign-off; no faked greens.

---

## 5. Gap analysis

| # | Gap | Severity |
|---|---|---|
| 1 | No contact/account/activity CRM model in D1 | **High** (blocks all) |
| 2 | Contacts not yet imported into D1 | **High** |
| 3 | Resend transactional send inert + unproven (flag off, not chained, domain/secrets unverified) | **High** |
| 4 | Chat → Blueprint `send_email` not wired | **High** for "automated in chat" |
| 5 | Multi-tenant scoping must be designed in (not bolted on) | Medium |
| 6 | Unsubscribe/suppression must be enforced on the live send | Medium (compliance) |
| 7 | Sending-domain reputation / SPF-DKIM-DMARC for a 150 blast | Medium (deliverability) |
| 8 | Reply tracking (inbound→activity) | Low (later) |

---

## 6. Plan

**Sprint A — ~150 tomorrow, on our infra (also proves the send path):**
1. (Owner) Hand off the contact list (CSV preferred) + confirm the segment.
2. (Me) Import → D1, de-dupe, report exact count + sample.
3. (Me) Verify prod Resend secrets + sending-domain verification; arm the transactional sender.
4. (Me) Live send→deliver smoke (a few seed addresses) with bounce/complaint handling.
5. (You) Review rendered copy on real contacts.
6. (Me) Send 150 in waves through the Worker; suppression honored; sends recorded in D1; greens `native_email_sequences.v1` if the receipts hold.

**Sprint B — the automated CRM (build order §4 steps 1→5):** D1 schema → import → proven send → chat/Blueprint wiring → desktop pane. Tenant-scoped throughout so customers get the same infra.

**Owner-gated (mirrored to `NEEDS_OWNER.md`):**
- The contact list/segment + handoff form (CSV?).
- Confirm/verify the **sending domain** in Resend (the deliverability gate that could slip tomorrow).
- Final copy/subject (no copy changes without your OK).
- Draft/preview review before the live blast.

---

## 7. Risks

- **Deliverability is the timeline risk for tomorrow.** Unverified/cold domain → spam folder + reputation hit. Verify the domain, warm-send in waves, real unsubscribe, suppression honored.
- **Multi-tenant isolation.** Because customers will use this, contact data must be strictly tenant-scoped from the first migration — no cross-tenant leakage, ever.
- **No-resale / identity rules.** Outreach copy + any agent automation must respect the standing no-resale (subscription-scoped) and public-identity rules.
- **Don't trust "it's wired" without the smoke.** Resend bindings ≠ proven delivery; treat the send path as unproven until a live receipt exists.
- **Capture the send record.** Every send writes to the D1 ledger so "who we contacted / who replied" is real.

---

## 8. Immediate next actions

1. **(Owner)** Answer the gating items in `NEEDS_OWNER.md` (list/segment + handoff, sending domain, copy, review).
2. **(Me, on go)** Open PR #1: tenant-scoped D1 contact-CRM schema + Effect Schema contracts + read APIs in `apps/openagents.com` (no behavior change).
3. **(Me)** One-time contact import → D1; report counts.
4. **(Me)** Arm + prove the Resend transactional send (the deliverability smoke); then Sprint A send.
5. **(Me)** Wire chat → Blueprint `send_email` + approval surface; then the desktop CRM pane.

---

### Appendix — key file references

- **Build here:** `apps/openagents.com/workers/api/migrations/{0026,0063,0064,0072,0081,0088,0181,0191,0193,0216}_*.sql`, `workers/api/src/{email,email-campaign-dispatcher,email-sequence-send-service,resend-webhooks,email-preferences}.ts`, `config.ts:185-188`, `wrangler.jsonc:44-45`, `blueprint-routes.ts` (send_email), `product-promises.ts` (`:211`, `:2598`, `:1464`, `:2563`).
- **Schema reference only (not a runtime):** `autopilot3/convex/{schema,crmAdmin,crmContactCommands,crmSourceImport}.ts`; deprecated Laravel `crm_*` migrations + ability model.
- **Decommissioned / not used:** `scripts/crm-gmail.sh`, the Laravel CRM API, Convex.
</content>
