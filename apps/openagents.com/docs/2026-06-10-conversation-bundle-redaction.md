# Conversation Bundle Redaction Tool

`scripts/conversation-bundle-redaction.ts` creates public-safe conversation
bundles for `pylon.data_trace_revenue.v1` and NIP-DS dataset listings.

The tool is deny-by-default. It reads local transcript files, keeps only
`role`, `sequence`, `sourceRef`, and sanitized message `text`, and drops
provider metadata, local paths, timestamps, raw tool payloads, and private
fields. High-risk credential, wallet, payment, private-key, and seed-phrase
patterns cause a hard refusal instead of a best-effort redaction.

Example:

```sh
bun apps/openagents.com/scripts/conversation-bundle-redaction.ts build \
  --input ./conversation.jsonl \
  --out-dir ./bundle-out \
  --title "Redacted Conversation Bundle" \
  --d redacted-conversation-bundle
```

Outputs:

- `conversation-bundle.json`: deterministic canonical JSON payload.
- `manifest.json`: record count, redaction counts, bundle digest, and a NIP-DS
  projection whose `listingDigest` is the same digest used for the listing `x`
  tag.

The digest is computed with `sha256Hex` from `@openagentsinc/nip90`, which reuses
the shared `nostr-effect` NIP-DS implementation.
