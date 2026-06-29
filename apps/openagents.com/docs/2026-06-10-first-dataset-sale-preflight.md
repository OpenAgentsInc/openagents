# First Dataset Sale Preflight

Date: 2026-06-10

Issue: `OpenAgentsInc/openagents#4645`

This is preflight evidence for the data-stream sale lane. It does not claim a
settled dataset sale. No owner-approved private conversation bundle was
exported, no buyer paid, no entitlement was granted, no transition receipt was
recorded, and no registry edit was made.

## Fixture Handoff

Command:

```bash
tmpdir=$(mktemp -d)
bun apps/openagents.com/scripts/conversation-bundle-redaction.ts build \
  --input apps/openagents.com/scripts/fixtures/conversation-bundle-redaction/clean.jsonl \
  --out-dir "$tmpdir" \
  --title "Fixture Conversation Bundle" \
  --d fixture-conversation-bundle \
  --summary "Fixture summary" \
  --generated-at 2026-06-10T12:00:00.000Z
bun apps/openagents.com/scripts/nip-ds.ts draft \
  --file "$tmpdir/conversation-bundle.json" \
  --title "Fixture Conversation Bundle" \
  --d fixture-conversation-bundle \
  --summary "Fixture summary" \
  --price-sats 50
rm -rf "$tmpdir"
```

Result:

```json
{
  "redactionDigest": "03207e08c263282909eac8ea93babd293cc9756ecf8c8012f746c3a907dfd87c",
  "nipDsDigest": "03207e08c263282909eac8ea93babd293cc9756ecf8c8012f746c3a907dfd87c",
  "digestsMatch": true,
  "deliveryDigestVerified": true,
  "eventKinds": [30404, 30406, 5960, 6960]
}
```

The preflight found and fixed a handoff bug: the redaction tool had written
`conversation-bundle.json` with a trailing newline, while the manifest digest
was computed over the canonical JSON payload without that newline. The artifact
now writes the exact bytes used for the manifest digest, so the NIP-DS listing
digest matches the redaction manifest digest.

## Tests

```bash
bun test apps/openagents.com/scripts/conversation-bundle-redaction.test.ts packages/nip90/src/index.test.ts --max-concurrency=1
```

Result: 7 passed, 0 failed.

Coverage:

- clean fixture produces a public-safe bundle manifest;
- seeded credential material refuses before writing a sellable bundle;
- written bundle bytes hash to the manifest digest;
- NIP-DS listing/offer/request/result helpers validate through
  `@openagentsinc/nip90`;
- delivery descriptor digest verification is green for the fixture bundle.

## Remaining Blockers

The #4645 acceptance criteria still require:

- owner-approved export of a real conversation bundle;
- public-safe redaction manifest for that real bundle;
- listing and offer on the scoped relay;
- operator-approved funded purchase;
- delivery entitlement with digest verification;
- public settlement receipt with stream kind `data`;
- product-promise transition receipts before any registry edit.

The live public data settlement counters remain empty as of this preflight:

```json
{
  "nip90MarketSettlementStats.data.jobsSettledTotal": 0,
  "nip90MarketSettlementStats.data.satsSettledTotal": 0,
  "nip90MarketSettlementStats.data.receiptRefs": []
}
```
