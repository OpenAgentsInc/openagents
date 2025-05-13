# Transaction Display Fixes

## Issues Addressed
1. Inconsistent transaction direction indicators
2. Inconsistent transaction type labeling
3. Issues with amount sign display

## Changes Made

### Transaction Direction Detection Improvements
- Added explicit check for "INCOMING" direction field
- Implemented tiered approach to direction detection:
  1. First check explicit direction fields (highest priority)
  2. Then check amount sign (negative means sent)
  3. Then use transaction type-specific logic (such as PREIMAGE_SWAP always being received)
- Added special handling for Lightning transactions with descriptions containing "paid invoice"

### Amount Handling Improvements
- Improved BigInt conversion with proper type checking
- Ensured always displaying positive amounts, with sign prefix based on direction
- Fixed handling of negative amounts for determining transaction direction
- Added support for more amount field formats

### Transaction Type Formatting
- Added explicit handling for On-chain transactions
- Added more transaction types: Swap and Invoice
- Improved network detection (Lightning, On-chain, Bitcoin, Spark)
- Consistent format for all transaction types: [Network] [Action] [Direction]

### Status Badge Improvements  
- Enhanced status badge variant detection with more status types
- Added comprehensive checking for completed/success states
- Added comprehensive checking for pending/in-progress states
- Added comprehensive checking for failed/error states
- Created a formatStatus function to produce more readable status labels
- Added proper capitalization of status words

## Testing
To verify these changes, check transaction history for:
1. Consistent direction indicators (arrows and +/- signs)
2. Consistent and readable transaction type labels  
3. Proper status badge colors based on transaction state
4. Correct formatting of transaction amounts