# Sprint A — the ~150 Dual-Channel Send Runbook

Epic #5980, sub-issue #5988. End-to-end operator runbook for contacting ~150
people on our own infra (Gmail/`gws` + Resend), dry-run-first.

## 0. Prereqs (owner-gated)

- Admin token for `Authorization: Bearer` on `/api/operator/crm/*`.
- The contact list as CSV (or already imported).
- Gmail: `gws auth login -s gmail` as the sending mailbox (~500 external/day on Workspace).
- Resend (only for the Resend portion): `RESEND_API_KEY` + a **verified sending
  domain** on the Worker, and `CRM_RESEND_SEND_ENABLED` armed. Until then, Resend
  sends are dry-run/`not_configured` (honest — see `resend-channel-runbook.md`).

## 1. Import contacts

`POST /api/operator/crm/import` (see `csv-import-runbook.md`). Verify count:
`GET /api/operator/crm/import-runs` and `GET /api/operator/crm/contacts?limit=10`.

## 2. Author the template

`POST /api/operator/crm/templates { slug, name, subjectTemplate, bodyMarkdownTemplate }`.
Personalize with `{{ contact.first_name_or_there }}`, `{{ app.name }}`, etc.

## 3. Decide the split

Pick which contacts go **Gmail** (1:1, high-touch, from your mailbox) vs
**Resend** (scalable). The batch endpoint takes one channel per call, so the
split is just two runs over two id lists.

## 4. Dry-run first (no writes)

```
CRM_ADMIN_TOKEN=... node apps/openagents.com/scripts/crm-send-batch.mjs \
  --template <slug> --channel gmail_gws --from-contacts --search <segment>
```

Dry-run is the **default**. It composes + runs the shared suppression/unsubscribe
gate per contact and reports `would_send` vs `suppressed` vs `failed` — so you
see exactly who would be contacted before anything happens.

## 5. Review rendered copy

Spot-check a few: `GET /api/operator/crm/contacts/:id/render?template=<slug>`.

## 6. Send in waves

```
# Gmail portion — queues for the local executor (still draft-first there)
... crm-send-batch.mjs --template <slug> --channel gmail_gws --ids-file gmail_ids.txt --send --wave 25 --pause-ms 2000
# then drain + send the Gmail queue from your machine (draft-first):
CRM_ADMIN_TOKEN=... node apps/openagents.com/scripts/crm-gmail-executor.mjs        # review drafts in Gmail
CRM_ADMIN_TOKEN=... node apps/openagents.com/scripts/crm-gmail-executor.mjs --send # then send

# Resend portion (only once the Worker is armed + domain verified)
... crm-send-batch.mjs --template <slug> --channel resend --ids-file resend_ids.txt --send --wave 50
```

`--send` opts out of dry-run. Even then: Gmail only **queues** (your executor +
Gmail-draft review gate the actual send); Resend sends only when armed. Waves +
`--pause-ms` respect daily limits.

## 7. Verify + capture

Every send is recorded: `GET /api/operator/crm/contacts/:id/emails` (ledger) and
the contact's activity timeline. Bounces/complaints flow into suppression via the
Resend webhook, so re-runs automatically skip them.

## Safety summary

- Dry-run by default at **every** layer (batch script, Gmail executor, Resend flag).
- Suppression/unsubscribe enforced once, for both channels.
- No copy is changed without the owner; the live blast is owner-driven.
- The Resend deliverability green (`native_email_sequences.v1`) still needs the
  owner-gated verified-domain receipt — not claimed here.
