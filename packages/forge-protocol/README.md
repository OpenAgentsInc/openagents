# OpenAgents Forge Protocol

`@openagentsinc/forge-protocol` is the shared contract package for
`forge.openagents.com` coordination records.

Phase 0 defines the D1 source-of-truth row shapes for Forge issues, change
records, NIP-34-aligned status rows, dispatch leases, and virtual merge-queue
ledger snapshots. It also defines metadata-only rows for private git packfile
archives: D1 carries refs, byte counts, hashes, object format, and JSON command
summaries while raw pack bytes stay in R2. Tenant auth rows define active
tenants, hashed git access tokens, and bounded git operation scopes
(`git:upload-pack`, `git:receive-pack`, `git:admin`). Runtime Workers and Pylon
code import these schemas instead of re-declaring local coordination records.

Control-plane calls use the separate `ForgeControlPlaneScope` set
(`forge:*`). Tenant git access scopes are only valid for smart Git HTTP and must
not authorize `/api/forge/*` routes. Non-admin control-plane tokens are scoped
to one tenant ref, and route handlers compare that authenticated tenant to the
query or body `tenantRef` before returning rows or persisting mutations. The
boundary contract is documented in
`docs/forge/2026-06-28-forge-boundary-contract.md`.

The package also defines the first Pylon-to-Forge dispatch messages:
`work_item`, `decision`, and `closeout`. A work item carries the tenant, work,
lease, issue/objective, scoped git target, short-lived git token reference and
prefix, and optional verification command descriptor. Decisions record Pylon
accept/reject state. Closeouts return the redacted Pylon result, packfile ref,
verification ref, artifact/proof/result refs, and settlement status.

Verification, promotion, and GitHub mirror receipts are modeled as
`ForgeVerificationReceipt`, `ForgePromotionDecisionReceipt`, and
`ForgeGitHubMirrorReceipt`. They carry refs, hashes, command metadata,
exit/verdict state, queue position, canonical target ref, bounded gate results,
mirror source/destination refs, mirror status, timestamps, artifact refs, and
log digests, but not raw logs, raw source, private provider payloads, git
tokens, or wallet material.

The package is public-safe by default: records carry refs, bounded state,
timestamps, and JSON-encoded ref arrays. They do not carry raw prompts, raw
provider payloads, private repository contents, local paths, raw git tokens,
secrets, or wallet material.

Tenant rows also define optional external-fleet posture refs:
`confidential_workspace_mode`, `attestation_ref`,
`encrypted_knowledge_pack_ref`, `refusal_reason`, and
`retention_policy_ref`. These are safe read-model fields for onboarding and
policy display only; they do not grant API, Git, promotion, mirror, or artifact
authority.
