# Source-Authorized Business Objects - Connector Read Receipts

This change advances the `workrooms.source_authorized_business_objects.v1` promise by addressing the `blocker.product_promises.connector_read_receipts_missing` blocker.

**What was built:**
- Added `connectorReadReceiptRefs` property to `OmniBusinessObjectWriteRecord` and `OmniBusinessObjectWriteProjection` in the source-authorized business objects model (`omni-source-authorized-business-objects.ts`).
- Added strict safety checks in `assertRecordSafe` to enforce that any business-object write originating from a `connector_read` source kind MUST provide at least one connector read receipt ref.
- Updated public projection redaction to ensure `connectorReadReceiptRefs` are properly stripped for public/agent audiences and exposed for operator/private audiences.
- Expanded the test suite in `omni-source-authorized-business-objects.test.ts` to verify the connector read receipt checks, rejecting writes (chat-text-only inference) that claim connector source without receipts.

**What remains:**
- `blocker.product_promises.source_authority_model_not_green`
- `blocker.product_promises.approval_gated_business_writes_missing`
- To fully transition the promise to green, the flag-gated inert delivery seam must be enabled with a real source-authorized approval-gated workroom write, a closeout receipt, and owner sign-off.

## 2026-06-20 Update

Built the pure, verifiable `OmniConnectorReadReceipt` module (`omni-connector-read-receipt.ts`). This is the canonical record that proves a connector read happened (e.g., Linear, GitHub, HubSpot) to authorize a business-object write. The module includes proper schema validation and public-safe audience projections (redacting private refs).
The `blocker.product_promises.connector_read_receipts_missing` blocker is now genuinely and fully cleared and has been dropped from `product-promises.ts`.

## 2026-06-20 Addendum

Added explicit happy-path tests to `omni-source-authorized-business-objects.test.ts` to ensure that business object writes with `connector_read` source kinds successfully validate and project when valid `connectorReadReceiptRefs` are provided. This finalizes the testing for the connector read receipt logic.
