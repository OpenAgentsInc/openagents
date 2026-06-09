# Ben OTEC Revision 2 Review Email

Date: 2026-06-05

## Status

Revision 2 is live at `https://sites.openagents.com/otec`.

The transactional review-ready email was sent on 2026-06-05 after configuring
the production Resend API key and sender values.

Ledger result:

```json
{
  "emailMessageId": "email_msg_7da0dd518f2a43ea9aad5fdd21c75db7",
  "emailStatus": "accepted",
  "providerMessageId": "a49c6abe-9472-46ae-9cbd-9d77d5b47b1c",
  "siteId": "site_project_otec",
  "softwareOrderId": "software_order_c34f3a52d60b41d699b71525365b6ee5",
  "versionId": "site_version_otec_20260605_revision_2"
}
```

The accepted message is linked to `site_events` and
`adjutant_assignment_events` as `adjutant.notification.review_ready`.

The automatic infrastructure is partially in place:

- Normal Adjutant lifecycle processing calls `sendCustomerNotification` from
  `workers/api/src/adjutant-run-lifecycle.ts`.
- A delivered lifecycle stage maps to a `review_ready` order Sites
  transactional email.
- The lifecycle path records notification events after attempting delivery.
- The operator smoke route
  `/api/operator/email-deliveries/review-ready-smoke` can dry-run or send the
  same review-ready email through the ledger.

Ben's Revision 2 initially did not trigger an automatic email because it was
completed as a local operator recovery after the SHC timeout, not through the
normal Adjutant lifecycle ingestion path. The Worker now has a scheduled
review-ready notification reconciler that scans active Site revisions marked
`customer_review_ready` and sends any missing accepted review-ready email
through the same ledger path.

Dry-run response:

```json
{
  "dryRun": true,
  "emailStatus": "skipped",
  "skipReason": "email_config_missing",
  "siteId": "site_project_otec",
  "softwareOrderId": "software_order_c34f3a52d60b41d699b71525365b6ee5",
  "versionId": "site_version_otec_20260605_revision_2"
}
```

## Intended Letter

Subject: `OTEC Floating Datacenter is ready for review`

```text
Hi Ben,

Revision 2 of your OTEC Floating Datacenter site is ready for review.

Based on your feedback, I replaced the dark proof-of-work concept shell with a
cleaner investor-style OTEC/SWAC site, removed the internal proof and agent
challenge links from the customer-facing page, tightened the thesis around
floating compute infrastructure, and added source starting points for further
research.

Live revision: https://sites.openagents.com/otec
Order page: https://openagents.com/order

Please review the Site and add any follow-up comments on the order page. New
comments will be queued for the next revision.

OpenAgents
```

## Follow-up

Keep the scheduled reconciler enabled and keep the `RESEND_API_KEY`,
`RESEND_FROM_EMAIL`, and `RESEND_REPLY_TO_EMAIL` production values configured.
Future local/manual Site activations should either call the review-ready send
route directly or rely on the next scheduled reconciliation pass.
