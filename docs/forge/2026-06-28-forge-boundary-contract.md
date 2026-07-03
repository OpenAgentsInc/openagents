# forge.openagents.com Boundary Contract

Status: SU-0 boundary lock for #6768, 2026-06-28. Public-safe; no secrets,
tokens, private source, or raw logs.

This document freezes the first Forge execution boundary. #6770 added
`/api/forge/*` control-plane routes and #6771 wired smart-Git receive-pack
intake.

## Authority Split

Forge has two separate auth planes:

- **Smart Git HTTP:** tenant git tokens minted by the Forge tenant git auth
  store. These tokens grant only `ForgeGitAccessScope` values:
  `git:upload-pack`, `git:receive-pack`, and `git:admin`.
- **Control plane:** service, session, or admin credentials with
  `ForgeControlPlaneScope` values. These scopes are all prefixed with
  `forge:*` and are the only scopes accepted by `/api/forge/*`. Non-admin
  control-plane bearer tokens are also bound to one tenant ref; callers send
  that scope as `X-OpenAgents-Forge-Tenant-Ref`, and the Worker rejects any
  request body or query `tenantRef` that does not match it.

Tenant git tokens must not authorize control-plane calls. A route that accepts
`git:receive-pack` or `git:admin` for `/api/forge/*` is out of contract.

The shared typed contract lives in `@openagentsinc/forge-protocol`:

- `ForgeGitAccessScope`
- `ForgeControlPlaneScope`
- `ForgeVerificationReceipt`
- `ForgePromotionDecisionReceipt`

## Canonical Storage Boundary

The R2 packfile archive is evidence, not ref authority.

- **R2 packfile archive:** private raw pack bytes plus D1 metadata such as
  digest, byte count, content type, ref-update summary, and source refs.
- **Canonical git object/ref store:** D1 tables for receive-pack intakes, active
  refs, tip object metadata, and ref-lock receipts. A packfile is applied here
  only after smart-Git auth, pkt-line parsing, object-id validation, and
  ref-lock success.
- **D1 coordination store:** work records, change records, NIP-34-aligned
  status rows, dispatch leases, verification refs, and merge queue snapshots.
- **Blueprint gates:** promotion decisions depend on verification, issue-close
  safety, command-source verification, deletion guards, and merge-deploy gates.

Promotion is a gated canonical-ref fast-forward. It is not a metadata-only D1
flip and not a GitHub PR merge.

## Proposed Control-Plane Route Notes

The first `/api/forge/*` routes may live in the `apps/openagents.com` Worker to
reuse the existing D1/R2/DO bindings. The UI remains owned by `apps/forge/` on
`forge.openagents.com`.

| Route family | Minimum scope | Notes |
| --- | --- | --- |
| `GET /api/forge/work-records` | `forge:work:read` | List or inspect work records. |
| `POST /api/forge/work-records` | `forge:work:write` | Create or upsert work records. |
| `GET /api/forge/changes` | `forge:change:read` | List or inspect change records. |
| `POST /api/forge/changes` | `forge:change:write` | Create change records after bounded intake. |
| `PATCH /api/forge/changes/{changeRef}/status` | `forge:status:write` | Append status transitions. |
| `GET /api/forge/statuses` | `forge:change:read` | List status transitions. |
| `GET /api/forge/leases` | `forge:lease:write` | Inspect dispatch lease state. |
| `POST /api/forge/leases` | `forge:lease:write` | Acquire or release dispatch leases. |
| `GET /api/forge/queue` | `forge:queue:read` | Inspect merge queue state. |
| `POST /api/forge/queue/snapshots` | `forge:queue:write` | Persist virtual merge queue projections. |
| `GET /api/forge/refs` | `forge:change:read` | Inspect canonical git refs for a tenant/repository. |
| `GET /api/forge/verification-receipts` | `forge:change:read` | List redacted verification receipts. |
| `POST /api/forge/verification-receipts` | `forge:receipt:write` | Record redacted verification receipts. |
| `GET /api/forge/promotion-decisions` | `forge:queue:read` | List gated promotion decisions. |
| `POST /api/forge/promotion-decisions` | `forge:promotion:decide` | Record gated promotion decisions. |
| `POST /api/forge/admin/import-openagents` | `forge:admin` | Refresh the dogfood `OpenAgentsInc/openagents` import for `tenant.openagents` / `repo.openagents.openagents`. |
| `/api/forge/admin/*` | `forge:admin` | Operator-only administrative actions. |

Every route decodes through typed data structures from
`@openagentsinc/forge-protocol`. Routes must fail closed when a caller presents
only tenant smart-Git credentials or when a tenant-scoped control-plane token
attempts to read or mutate another tenant's rows, receipts, leases, refs, or
artifact metadata.

Forge tenant records may carry optional confidential-workspace posture fields:
`confidential_workspace_mode`, `attestation_ref`,
`encrypted_knowledge_pack_ref`, `refusal_reason`, and `retention_policy_ref`.
These fields are refs and bounded state only. They do not carry raw
attestations, raw knowledge packs, private repo contents, prompts, tokens, or
secrets.

## Smart-Git Route Notes

Smart-Git HTTP stays outside `/api/forge/*` and accepts only tenant git tokens.
The first receive-pack surface lives in the `apps/openagents.com` Worker:

| Route | Required git scope | Notes |
| --- | --- | --- |
| `GET /git/{tenantRef}/{repositoryRef}.git/info/refs?service=git-receive-pack` | `git:receive-pack` or `git:admin` | Advertises current canonical refs and receive-pack capabilities. |
| `POST /git/{tenantRef}/{repositoryRef}.git/git-receive-pack` | `git:receive-pack` or `git:admin` | Parses the receive-pack body, archives PACK bytes to private R2, applies canonical ref/object metadata under D1 ref locks, then writes Forge coordination rows. |

This route fails closed before coordination mutation for malformed pkt-lines,
invalid object IDs, unsafe stale ref updates, wrong repository/scope tokens, and
delete-only pushes.

For background-agent definition dispatch (#8200), Forge tenant git tokens may
also carry `ref_restrictions_json`. Smart-Git receive-pack rejects any command
whose ref is outside that token allowlist before packfile archive or
coordination mutation. Definition dispatch stores only public-safe token refs on
the work record (`git_token_refs_json`) and run row; raw token values are never
stored there, and Pylon closeout revokes the recorded token refs.

## Verification Receipt Format

`ForgeVerificationReceipt` records the public-safe result of a verifier run.
The required fields are:

- `change_ref`
- `base_ref` and `base_head`
- `head_ref` and `head_head`
- `packfile_ref` and `packfile_sha256`
- `executor_identity_ref`
- `command_ref` and `command_args`
- `exit_code`
- `verdict`
- `started_at` and `completed_at`
- `artifact_refs`
- `log_sha256`
- `source_refs`
- `redacted: true`

Receipts must not persist raw stdout, stderr, logs, source, provider payloads,
git tokens, wallet material, or secrets. Store raw private artifacts only behind
operator-only artifact refs when needed.

## Promotion Decision Receipt Format

`ForgePromotionDecisionReceipt` records the gate result that made a change
promotable or blocked it. The required fields are:

- `promotion_ref`
- `queue_ref`
- `queue_position`
- `change_ref`
- `decision`
- `target_ref`
- `base_head`
- `candidate_head`
- `promoted_head`
- `verification_ref`
- `gate_refs`
- `gate_results`
- `blocker_refs`
- `decided_by_ref`
- `decided_at`
- `source_refs`
- `redacted: true`

`target_ref` names the canonical ref being promoted. `approved` decisions
identify the fast-forward target in `promoted_head`. `blocked` decisions keep
`promoted_head` null and name the blockers. `gate_results` records one bounded
public-safe result per Blueprint gate, with evidence refs and blocker refs only.

## UI Boundary

`apps/forge/` owns the Forge product UI and deploys to `forge.openagents.com`.
It may consume `@openagentsinc/ui` tokens/primitives and the Forge API contract,
but it does not own settlement, payout, accepted-work authority, runtime
promotion, or the main `openagents.com` logged-in route tree.

The old logged-in Forge page in `apps/openagents.com` remains source material
only. It is not the expansion target.

## Implementation Follow-Through

- #6769 expands the separate Forge UI shell against this contract.
- #6770 implements `/api/forge/*` routes with `ForgeControlPlaneScope` in the
  `apps/openagents.com` Worker and persists receipt rows through
  `0254_forge_control_plane_receipts.sql`.
- #6771 implements smart-Git intake with `ForgeGitAccessScope`, R2 archive
  evidence, canonical ref locks, tip object metadata, and D1 change rows.
