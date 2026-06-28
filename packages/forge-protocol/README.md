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

The package is public-safe by default: records carry refs, bounded state,
timestamps, and JSON-encoded ref arrays. They do not carry raw prompts, raw
provider payloads, private repository contents, local paths, secrets, or wallet
material.
