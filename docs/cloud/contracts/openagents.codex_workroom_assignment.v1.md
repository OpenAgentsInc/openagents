# `openagents.codex_workroom_assignment.v1`

Status: implementation scaffold for `CND-045`

This contract is the Cloud-side input for the first Codex VM workroom runner.
It lets Vortex/Autopilot ask Cloud to run one bounded Codex task on a managed
node while Probe can consume the same normalized event shape later.

## Assignment Fields

| Field | Purpose |
| --- | --- |
| `assignment_id` | Stable run id from Vortex/Autopilot. |
| `workroom_id` | Private no-wallet workroom id. |
| `target_node_id` | First target is `oa-gcp-shc-katy-01`. |
| `user_ref` / `organization_ref` / `project_ref` | Vortex-owned actor scope. |
| `provider_account_ref` | Sanitized ChatGPT/Codex provider-account ref. |
| `auth_grant_ref` | Session grant produced by `openagents.codex_auth_grant.v1`. |
| `repo_ref` | Non-secret repo/project context. |
| `prompt` | Bounded instruction for Codex. |
| `required_artifacts` | Artifact filenames Codex must create in the workroom. |
| `sandbox` | `read_only`, `workspace_write`, or `danger_full_access`; SHC Codex smoke uses `danger_full_access` inside the no-wallet VM boundary. |
| `timeout_ms` | Optional run timeout, capped at one hour. |
| `wallet_authority` | Must be `false`. |
| `audit_context` | Non-secret source context such as issue/run refs. |

## Event Fields

`openagents.codex_workroom_event.v1` records normalized runner events:

- `queued`
- `started`
- `log`
- `redacted`
- `artifact`
- `receipt`
- `completed`
- `failed`
- `timeout`
- `cancelled`
- `cleanup`

Events carry sequence numbers, sanitized messages, optional artifact digest
refs, optional receipt digest refs, and their own `sha256:` event digest.
They do not carry raw process environment, `auth.json` content, access tokens,
refresh tokens, API keys, wallet material, or broad GCP credentials.

## Validation Rules

- `wallet_authority` must be `false`.
- At least one required artifact is required.
- Artifact names are bounded names, not paths.
- Assignment and event fields reject common secret markers.
- Artifact, receipt, and event refs must be `sha256:` digests.
- Assignment timeouts must be positive and no longer than one hour.
- `danger_full_access` is an explicit profile, not an implicit fallback. Use it
  when the enclosing VM/container boundary is the sandbox and the workroom has
  no wallet authority or broad host/cloud credentials.

## Current Command

```bash
oa-workroomd codex run \
  --assignment-file ./codex-workroom-assignment.json \
  --codex-bin codex \
  --state-dir ./workroom-state \
  --json
```

The command expects `oa-workroomd codex auth materialize` to have already
created a session-scoped Codex auth state for the matching
`auth_grant_ref`.
