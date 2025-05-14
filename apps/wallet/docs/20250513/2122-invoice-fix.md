Okay, I understand the issue. Lightning "spend" invoices (meaning, Lightning payments *made by the user*) are being incorrectly displayed as "received" transactions. This indicates a bug in the logic that determines the direction of a transaction in `src/components/TransactionItem.tsx`.

The most likely cause is that the current rules for classifying transactions, particularly those of type `PREIMAGE_SWAP`, are overriding other indicators that would correctly mark the transaction as "sent" (outgoing). Specifically, an explicit `OUTGOING` value in the `transfer_direction` field should take precedence.

Here are the instructions for the coding agent to fix this:

**Objective:** Modify the transaction direction determination logic in `src/components/TransactionItem.tsx` to ensure that Lightning spend transactions are correctly identified and displayed as "sent". The primary fix involves ensuring that an explicit `transfer_direction: "OUTGOING"` is respected, even for transactions of type `PREIMAGE_SWAP`.

**File to Modify:** `src/components/TransactionItem.tsx`

**Instructions:**

1.  **Locate the `isSent` determination logic:**
    In `src/components/TransactionItem.tsx`, find the block of code that starts with `let isSent = false;` and is responsible for determining the transaction direction. This block includes checks for `transaction.transfer_direction`, `transaction.totalValue`, and `transaction.type`.

2.  **Replace the existing `isSent` determination logic:**
    Replace the entire block of code used to determine the `isSent` variable with the following revised logic. This new logic introduces flags (`explicitlyOutgoing`, `explicitlyIncoming`) to better manage precedence, ensuring that an explicit "OUTGOING" direction is not incorrectly overridden.

    ```typescript
    // START OF REPLACEMENT BLOCK

    // Initialize determination of whether this is a sent transaction
    let isSent = false; // Default to received
    let explicitlyOutgoing = false;
    let explicitlyIncoming = false;

    // First check explicit direction fields (highest priority)
    if (transaction?.transfer_direction === "OUTGOING" || transaction?.transferDirection === "OUTGOING") {
      isSent = true;
      explicitlyOutgoing = true;
    } else if (transaction?.transfer_direction === "INCOMING" || transaction?.transferDirection === "INCOMING") {
      isSent = false;
      explicitlyIncoming = true;
    }

    // If not explicitly set by direction field, try amount-based direction check (second priority)
    // This will only apply if transfer_direction was not present or was ambiguous.
    if (!explicitlyOutgoing && !explicitlyIncoming) {
      if (transaction?.totalValue) {
        const value = typeof transaction.totalValue === 'bigint'
          ? transaction.totalValue
          : BigInt(transaction.totalValue);

        if (value < BigInt(0)) {
          isSent = true; // Implied outgoing due to negative amount
        } else if (value > BigInt(0)) {
          isSent = false; // Implied incoming due to positive amount
        }
        // If value is 0 or null/undefined, isSent remains based on its initialization or previous rules (if any).
        // For safety, if amount is 0 and no direction, it defaults to 'received' (isSent = false).
      }
    }

    // Type-based specialized determination (third priority)
    // This logic primarily refines or sets 'isSent' if stronger signals (explicit direction, unambiguous amount) were absent or inconclusive.
    if (transaction?.type) {
      const txType = transaction.type.toUpperCase();

      if (txType === "PREIMAGE_SWAP") {
        // A PREIMAGE_SWAP is often a received Lightning payment.
        // However, if the direction was EXPLICITLY "OUTGOING" from the SDK, we must respect that.
        // So, only set to 'received' if it wasn't explicitly marked as 'outgoing'.
        if (!explicitlyOutgoing) {
          isSent = false;
        }
        // If 'explicitlyOutgoing' was true, 'isSent' is already true and will remain so.
      } else if (txType.includes("LIGHTNING") && txType.includes("PAYMENT")) {
        // This handles other "LIGHTNING_PAYMENT" types.
        // Apply description-based heuristic only if direction was not explicitly set by the SDK.
        if (!explicitlyOutgoing && !explicitlyIncoming) {
            // If 'isSent' is currently false (meaning neither explicit direction nor negative amount indicated 'sent')
            if (!isSent) {
                if (transaction.description && transaction.description.toLowerCase().includes("paid invoice")) {
                    isSent = true; // Description heuristic implies sent
                }
                // If no "paid invoice" in description, and other signals didn't mark it as sent,
                // it remains 'false' (received) by default from initialization or positive amount.
            }
        }
      }
      // Add more type-specific rules here if other transaction types are being misclassified.
    }

    // END OF REPLACEMENT BLOCK
    ```

3.  **Verify the changes:**
    *   Ensure the replaced block correctly integrates with the rest of the `TransactionItem` component logic (e.g., usage of `isSent` for displaying icons and amount prefixes).
    *   The rest of the component (amount calculation, date formatting, status badges, type formatting) should remain as it is unless further issues are identified.

**Explanation of the Fix:**

*   The new logic introduces `explicitlyOutgoing` and `explicitlyIncoming` flags. These flags are set if the `transfer_direction` (or `transferDirection`) field is present and clearly indicates the transaction direction.
*   The amount-based check is now only applied if the direction was not explicitly determined by the `transfer_direction` field.
*   Crucially, the rule for `PREIMAGE_SWAP` transactions is modified: `if (!explicitlyOutgoing) { isSent = false; }`. This ensures that if `transfer_direction` was "OUTGOING" (making `explicitlyOutgoing = true`), this `PREIMAGE_SWAP` rule will *not* incorrectly set `isSent` to `false`. The transaction will correctly be treated as "sent".
*   The heuristic for "LIGHTNING_PAYMENT" types based on the description ("paid invoice") is also conditioned on the absence of an explicit direction from the SDK.

This revised logic prioritizes explicit direction information from the SDK, which should lead to more accurate classification of Lightning spend transactions.

After applying these changes, test the wallet thoroughly by making Lightning payments (spending) and verifying that they appear correctly as "sent" in the transaction history. Also, ensure that received Lightning payments and other transaction types continue to be displayed correctly.
