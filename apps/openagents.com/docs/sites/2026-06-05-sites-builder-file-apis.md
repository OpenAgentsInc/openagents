# Sites Builder File APIs

Issue #195 adds the first customer-safe file inspection API for VibeSDK-style
Sites builder sessions.

## Implemented Surface

All endpoints require an authenticated browser session and reuse the builder
session ownership rule:

- the session owner can read it
- the session customer can read it
- OpenAgents admins can read operator-safe projections
- unrelated users receive `404` rather than a signal that the session exists

Endpoints:

- `GET /api/sites/builder-sessions/:sessionId/files`
- `GET /api/sites/builder-sessions/:sessionId/files/tree`
- `GET /api/sites/builder-sessions/:sessionId/files/read?path=<path>`
- `GET /api/sites/builder-sessions/:sessionId/files/export`

The list endpoint returns the latest visible snapshot per path with bounded
metadata: path, sequence, language, content hash, byte size, preview presence,
timestamps, and visibility. Customer sessions only receive files that are
marked `customer` and have preview text. Admin sessions can see operator-safe
operator/internal metadata and source/artifact refs.

The tree endpoint returns the same latest-file projection plus path segments.
This keeps the first UI simple while leaving room for a nested tree component
later.

The read endpoint returns the latest visible file for a path and includes the
bounded `previewText`. Customer reads of operator-only files, internal files,
files without preview text, missing files, and unrelated sessions all return
`404`.

The export endpoint currently returns a JSON manifest:

```json
{
  "exportKind": "customer_safe_preview_manifest",
  "sourceArchiveAvailable": false,
  "fullSourceExport": "future_artifact_token_required"
}
```

This is intentionally not a raw source archive. Full source export still needs
artifact-token authority, size limits, secret scanning, and download/clone
receipt handling.

## Guardrails

- file paths still pass the repository path validator
- customer file reads require `visibility: "customer"` and preview text
- list/tree/export deduplicate to latest snapshot by path
- export is a safe JSON manifest, not a source download
- no builder file endpoint leaks session existence to unrelated users
- tests cover list, read, tree, export, and unrelated-user denial

## Remaining Work

- operator/admin upload and snapshot-write API
- archive/download tokens for full source export
- UI tree and code viewer integration with the builder session event stream
- artifact manifest signing and source archive retention policy
