# Resend CRM Channel Runbook

The CRM's scalable, Worker-side send channel (epic #5980, sub-issue #5984),
sibling to the local Gmail/`gws` channel. Shares the same contacts, templates,
suppression gate, and send ledger.

## Honest arming (no faked greens)

The Resend sender is **INERT by default**. It sends only when **both**:

1. `CRM_RESEND_SEND_ENABLED` is truthy on the Worker, **and**
2. Resend is configured — `RESEND_API_KEY` present **and** `RESEND_FROM_EMAIL` set.

Disabled → the route returns a **dry-run** result and never calls Resend.
Configured-but-unverified domain → Resend rejects, recorded as a `failed`
ledger row. This mirrors `email-sequence-send-service.ts` and the standing
mandate: nothing sends until armed and the **sending domain is verified**
(SPF/DKIM/DMARC).

## Endpoint

```
POST /api/operator/crm/contacts/:id/resend-send   (admin-gated)
     { templateSlug, tenant?, sendReason? }
```

Result `kind`:
- `dry_run` (200) — send disabled; nothing sent.
- `not_configured` (200) — armed but no api key / from address.
- `suppressed` (409) — address opted out / bounced / complained.
- `sent` (200) — `{ message: { providerMessageId, status:'sent', ... } }`.
- `failed` (502) — provider rejected (e.g. unverified domain); recorded.

Composition + the suppression/unsubscribe gate (`readEmailSendEligibility`) are
shared with the Gmail channel. Every attempt records a `crm_email_messages` row;
a success also records a `crm_activity` (`email_sent`). Bounce/complaint events
flow back through the existing `resend-webhooks.ts` into `email_provider_events`
+ suppression.

## Deliverability proof (the green) — OWNER-GATED

`autopilot_sites.native_email_sequences.v1` is YELLOW; its remaining blocker is
`email_deliverability_unproven`. Clearing it honestly requires a **live
send→deliver receipt** with bounce/complaint handling, which needs a real
`RESEND_API_KEY` + a **verified sending domain** — owner-gated secrets/DNS.

Once those are in place:

```
CRM_ADMIN_TOKEN=... node apps/openagents.com/scripts/crm-resend-smoke.mjs \
  --contact <seed_contact_id> --template <slug> [--tenant <ref>]
```

The script sends one live email, asserts a `sent` ledger row with a provider
message id, and prints the next manual check (confirm inbox receipt + a
`delivered` event in `email_provider_events`). That receipt is what greens the
promise — receipt-first, owner-signed. **Do not flip the promise without it.**
