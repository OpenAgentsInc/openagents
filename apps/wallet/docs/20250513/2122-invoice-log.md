# 2122 - Lightning Invoice Direction Fix

## Date: May 13, 2025
## Time: 21:22

## Issue
Lightning "spend" invoices (where the user paid an invoice) were being incorrectly displayed as "received" transactions in the transaction history. This was due to a bug in the logic that determines the direction of a transaction in the `TransactionItem` component, specifically with how transactions of type `PREIMAGE_SWAP` were being handled.

## Root Cause
The issue was in the `TransactionItem.tsx` file, where transactions of type `PREIMAGE_SWAP` were always being marked as received, regardless of their actual direction as indicated by the `transfer_direction` field:

```typescript
// Old problematic code
if (txType === "PREIMAGE_SWAP") {
  isSent = false; // Override - PREIMAGE_SWAP is always a received payment
}
```

This code overrode any previous determination, including explicit direction indicators from the SDK such as `transfer_direction: "OUTGOING"`.

## Fix Implemented
I implemented the following changes to fix the issue:

1. Added explicit flags to track direction determination:
   - `explicitlyOutgoing` and `explicitlyIncoming` flags to better track when the direction has been explicitly set by the SDK
   - These flags prevent lower-priority heuristics from overriding explicit direction indicators

2. Changed the type-based determination logic:
   - For `PREIMAGE_SWAP` transactions: Only set to received if not explicitly marked as outgoing
   - For `LIGHTNING_PAYMENT` transactions: Only apply description-based heuristics if no explicit direction was given

3. Updated `formatType` method:
   - Modified the `PREIMAGE_SWAP` type formatting to respect the transaction direction rather than always displaying as "received"
   - Now correctly shows "Lightning Payment Sent" for outgoing PREIMAGE_SWAP transactions

## Verification
- Ran the TypeScript type checker (`pnpm run t`) to verify no type errors were introduced
- Ran the linter (`pnpm run lint`) to check for any code style issues
- All existing linting issues are unrelated to the changes made and were present before

## Testing Considerations
The fix should be tested with real-world transactions to verify:
1. Lightning payments sent by the user now correctly display as "sent" transactions with the proper icon and amount prefix
2. Received Lightning payments still correctly display as "received"
3. Other transaction types (Spark transfers, on-chain transactions) continue to display correctly

## Summary
This fix ensures that the explicit direction indicators from the SDK are respected and given highest priority in determining transaction direction. The main change is that `PREIMAGE_SWAP` transactions are no longer blindly marked as "received" but instead respect the explicit direction provided by the SDK.