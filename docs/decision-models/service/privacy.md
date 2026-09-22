# Privacy and retention

What each deployment lane stores, for how long, who can read it, and
what deletion does. This page describes the code as it runs; an
operator that logs more — a TLS terminator's access log, a backend's
own telemetry — owns that disclosure separately.

## The payload boundary

The gateway never persists request or response payloads. A decision
call's state, questions, and answers exist in memory for the forward
and leave behind only the sealed `ExecutionReceipt`: identities,
counts, timing, and SHA-256 digests of the request and result — never
their content. Raw payload logging is not a flag that defaults off; no
such flag exists. The backend process sees the payload while it
answers — that is the stated boundary, and a remote backend widens it
deliberately.

## Retention by record

| Record | Where | Lifetime |
| --- | --- | --- |
| Request/response payloads | memory only | the forward's duration |
| Execution receipts | `receipts.jsonl` | append-only, operator-managed |
| Quota ledger | `quota-ledger.jsonl` | append-only; unsettled holds orphan as `unknown`, never free |
| Credentials | `keys.json` | digests only, until revoked or rotated |
| Feedback | `feedback/` | until operator triage; `redact` replaces content with digests |
| Updates subscriptions | `updates/` | `unsubscribed` records persist as the opt-out audit |
| Jobs | `jobs/` | `job_retention_ms`, default seven days; `DELETE` removes a terminal record early |
| Accounts, sessions, access history | `accounts` document | bounded history; sessions expire by their own TTL |
| Billing | `billing` document | plan, invoice, and event journals — financial records are retained |
| Skills | `skills/` | published versions are permanent public artifacts; withdrawn versions stay in the audit trail |
| Manifest revisions | `registry.json` archives | every revision stays under its digest — an earlier answer is always explainable |

## Deletion and export

- `POST /v1/feedback` submissions support operator `redact` — content
  becomes digests, attachment bytes are removed.
- `DELETE /v1/updates` unsubscribes; the record survives marked
  `unsubscribed`, which is what a durable opt-out looks like.
- `GET /v1/workspaces/{id}/usage/export` is the caller's usage export
  where `accounts` is configured.
- `DELETE /v1/jobs/{id}` removes a terminal job's record.
- Deleting the registry directory deletes everything else; the install
  verification proves a full delete works.

## Subprocessors and providers

- **Model backend**: a subprocess or host the operator declares per
  door. Same host by default; a remote backend is a subprocessor the
  deployment must disclose to its tenants.
- **Review model**: the skills directory's review stage may call a
  configured `POST /v1/systemone` door — including a remote one — on
  submitted skill content only.
- **Payment provider**: only where `billing` is configured; signed
  webhooks are the inbound path and the provider sees checkout details.
- **Email**: none. No code in this repository sends mail; product
  updates are a stored preference, and the sender is a separate system
  the operator discloses.

## Access

- The operator of the host can read the registry — digests and records,
  not payloads, because payloads are not stored.
- A **shared** lane cannot claim dedicated isolation: tenants share the
  process, the receipt file, and the backend. Per-credential scoping —
  feedback, updates, job reads — answers another credential's records
  as not-found rather than disclosing them.
- A **dedicated** lane isolates the backend, not the operator: the host
  can still read the process's memory and its forwarded calls. "The
  host cannot see inputs" is a claim no lane here makes — it would need
  a mechanism this system does not have.
- An **on-device** lane keeps the whole workflow local; a remote
  reviewer, telemetry path, or external fetch is what changes that
  boundary, and any of them must be stated, not assumed absent.

## Encryption

TLS terminates at the edge terminator (`deploy/caddy`, `deploy/nginx`);
the gateway speaks plain HTTP on a private interface and the backend
listens on localhost. At rest there is no encryption layer — the
boundary is the `0750` service-owned registry directory. Cleartext
credentials exist nowhere on disk: `keys.json` is digests.
