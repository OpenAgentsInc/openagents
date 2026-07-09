# `openagents.codex_auth_grant.v1`

Status: implementation scaffold for `CND-046`

The Codex auth grant contract is the Cloud-side input from Vortex for a
session-scoped Codex VM workroom. It carries refs and policy only. It does not
carry raw ChatGPT/Codex tokens, API keys, device auth IDs, code verifiers, or
`auth.json` content.

## Grant Fields

| Field | Purpose |
| --- | --- |
| `workroom_id` | Private workroom receiving the Codex auth context. |
| `user_ref` / `organization_ref` / `project_ref` | Vortex-owned actor scope. |
| `provider_account_ref` | Sanitized Vortex provider-account ref. |
| `grant_ref` | Short-lived grant ref selected for one session. |
| `provider_secret_ref` | Server-side secret-store ref such as `secret://...`; never raw credential content. |
| `requested_mode` | `exec`, `mcp_server`, or `sdk_thread`. |
| `issued_at_ms` / `expires_at_ms` | TTL. Current contract rejects grants longer than two hours. |
| `audit_context` | Non-secret source context such as issue/run/workroom refs. |

## Receipt Fields

`openagents.codex_auth_receipt.v1` records:

- grant materialization;
- `codex login status` checks;
- VM-local auth cleanup;
- failure decisions.

Receipts include only refs, decision, reason, and digests. They do not include
auth file contents, command output, environment variables, or token material.

## Validation Rules

- `provider_secret_ref` must use an approved secret-ref prefix:
  `secret://`, `vault://`, `gcp-secret://`, `cloud-secret://`,
  `provider-account://`, or `codex-auth://`.
- Grant TTL must be positive and no longer than two hours.
- Refs and receipt reasons reject common secret markers such as API keys,
  bearer tokens, access/refresh/id tokens, device codes, code verifiers,
  `auth.json`, wallet seeds, private keys, and private topology markers.
- Receipt digests must be `sha256:` refs.
