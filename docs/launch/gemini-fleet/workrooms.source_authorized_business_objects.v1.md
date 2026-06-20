# Source-Authorized Business Objects - Connector Read Receipts

This change advances the `workrooms.source_authorized_business_objects.v1` promise by addressing the `blocker.product_promises.connector_read_receipts_missing` blocker.

**What was built (Previous):**
- Added `connectorReadReceiptRefs` property to `OmniBusinessObjectWriteRecord`.
- Added strict safety checks requiring connector read receipt refs for `connector_read` sources.

**What was built (Current):**
- Created the typed contract and projection for the connector read receipt itself in `apps/openagents.com/workers/api/src/omni-connector-read-receipts.ts`.
- Implemented `OmniConnectorReadReceiptRecord` and `OmniConnectorReadReceiptProjection` to prove that an agent/runtime actually read a specific piece of data from a connector source.
- Added strict public-safe projection rules (`projectOmniConnectorReadReceipt`) ensuring unsafe material (like raw connector payloads, customer data, and private paths) is never exposed to public/agent audiences.
- Added full test suite in `omni-connector-read-receipts.test.ts` verifying redaction and projection safety.

**What remains:**
- `blocker.product_promises.source_authority_model_not_green`
- `blocker.product_promises.approval_gated_business_writes_missing`
- To fully transition the promise to green, the flag-gated inert delivery seam must be enabled with a real source-authorized approval-gated workroom write, a closeout receipt, and owner sign-off.
