# Gmail / `gws` CRM Channel Runbook

The CRM's high-touch channel: send 1:1 from the operator's own business mailbox
via the Google Workspace CLI (`gws`), driven by the native CRM in
`apps/openagents.com` (epic #5980, sub-issue #5983). Ported off the old
Laravel-coupled `scripts/crm-gmail.sh` — now driven by our CRM and **with**
write-back so every send lands in the CRM ledger.

## Why local

Sending *as you* needs *your* Gmail OAuth, which can't live in a Cloudflare
Worker. So the Gmail channel runs locally (this Mac / Autopilot Desktop). The
Worker composes the personalized message, reports send eligibility, and records
the outcome; `gws` does the actual send.

## Worker endpoints (admin-gated)

```
GET  /api/operator/crm/templates                          list templates
POST /api/operator/crm/templates                          upsert a template
     { slug, name, subjectTemplate, bodyMarkdownTemplate, tenant? }
GET  /api/operator/crm/contacts/:id/render?template=<slug>&channel=gmail_gws
     -> { message:{toEmail,subject,bodyMarkdown,bodyHtml,templateId}, eligibility }
POST /api/operator/crm/contacts/:id/gmail-writeback
     { toEmail, subject, bodyMarkdown, bodyHtml?, status:'draft'|'sent',
       providerDraftId?, providerMessageId?, templateId?, fromEmail? }
GET  /api/operator/crm/contacts/:id/emails                contact send ledger
```

Templates use `{{ token }}` personalization: `contact.first_name`,
`contact.first_name_or_there` (falls back to "there"), `contact.full_name`,
`contact.last_name`, `contact.job_title`, `contact.primary_email`, `app.name`,
`app.base_url`. Unknown tokens render empty. Bodies are markdown; a minimal,
HTML-escaping renderer produces the HTML (`**bold**`, `[text](url)`, paragraphs).

Eligibility reuses the shared `readEmailSendEligibility` gate (suppression +
unsubscribe + preference), so a suppressed/opted-out address is never sent — the
render reports `eligibility.allowed=false` and a `sent` write-back is refused
with `409`.

## Local sender

`apps/openagents.com/scripts/crm-gmail-send.mjs`:

```
CRM_ADMIN_TOKEN=<token> \
node apps/openagents.com/scripts/crm-gmail-send.mjs \
  --contact <crm_contact_id> --template <slug> [--tenant <ref>] [--send]
```

- Default is **draft-first** (creates a Gmail draft you review). `--send` sends live.
- Env: `CRM_BASE_URL` (default `https://openagents.com`), `CRM_ADMIN_TOKEN`
  (required), `GWS_BIN` (default `gws`), `CRM_FROM_EMAIL` (optional, recorded).
- The default `gws gmail +send` flag set is best-effort; if your installed `gws`
  differs, override the whole invocation with `GWS_SEND_ARGS_JSON` (a JSON array
  using `{to}`, `{subject}`, `{htmlPath}` placeholders).
- Prereq: `gws auth login -s gmail` as the sending mailbox on this machine.

## Sequence for an outreach batch

1. Import contacts (`docs/crm/csv-import-runbook.md`).
2. Upsert a template (`POST /api/operator/crm/templates`).
3. Dry-run by creating **drafts** for each contact, review in Gmail.
4. `--send` in waves (respect Gmail ~500 external/day on Workspace).
5. Verify in the ledger: `GET /api/operator/crm/contacts/:id/emails`.

The unified two-channel send service (#5985) and chat→Blueprint wiring (#5986)
build on these same endpoints; Resend (#5984) is the scalable channel for the
rest.
