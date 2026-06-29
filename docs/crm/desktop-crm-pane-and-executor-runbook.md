# Desktop CRM Pane + Local Gmail Executor Runbook

Epic #5980, sub-issue #5987. The operator-facing read pane and the machine-side
Gmail sender that drains the unified-dispatch queue.

## CRM pane (presentational)

`apps/web/src/ui/crm-contacts-panel.ts` — a self-contained Foldkit/HTML builder
(same pattern as `email-sequence-panel.ts`): renders the contact list, a
selected contact's activity timeline + send ledger, and the Gmail queue depth.
**Display-only, no send authority.** An embedding page (desktop CRM pane / web)
supplies an already-projected `CrmContactsPaneModel`.

Shape the model from the read APIs with `buildCrmContactsPaneModel(...)`:
- contacts ← `GET /api/operator/crm/contacts`
- selected.activities ← `GET /api/operator/crm/contacts/:id/activities`
- selected.ledger ← `GET /api/operator/crm/contacts/:id/emails`
- queue ← `GET /api/operator/crm/gmail-queue`

Pure helpers (`crmContactDisplayName`, `crmRelationshipTone`,
`summarizeCrmActivities`, `countCrmSent`) are unit-tested; the builder is
presentational. Approvals for `send_email` commands (#5986) surface here as the
operator approval queue.

## Local Gmail executor

`apps/openagents.com/scripts/crm-gmail-executor.mjs` — drains the Gmail queue:

```
CRM_ADMIN_TOKEN=... node apps/openagents.com/scripts/crm-gmail-executor.mjs \
  [--tenant <ref>] [--send] [--limit N]
```

For each `queued` `gmail_gws` message it sends as the operator's mailbox via
`gws` (**draft-first**; `--send` to send live), then POSTs `gmail-writeback`
with the `messageId` to **update the same ledger row** to `draft`/`sent` (no
duplicate). Env mirrors the single-contact sender (`GWS_BIN`,
`GWS_SEND_ARGS_JSON` override). Prereq: `gws auth login -s gmail` as the sending
mailbox.

This closes the unified loop: chat/agent proposes (#5986) → operator approves →
`dispatchCrmSend` queues a `gmail_gws` row (#5985) → this executor sends locally
and writes back. Resend sends never touch this path (they send server-side).
