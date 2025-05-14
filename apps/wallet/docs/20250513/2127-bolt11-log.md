# 2127 - Lightning Invoice Payment Confirmation with bolt11 Decoding

## Date: May 13, 2025
## Time: 21:27

## Objective
Implement a safer Lightning payment flow by decoding BOLT11 invoices and showing a confirmation dialog before sending payments. This allows users to verify the payment amount and description before committing to the payment, improving the security and usability of the wallet.

## Implementation Details

### 1. Added New State Variables
- Added `decodedInvoiceDetails` - Stores the decoded invoice data from bolt11 library
- Added `showPaymentConfirmDialog` - Controls visibility of the confirmation dialog
- Added `invoiceToPay` - Stores the validated and cleaned invoice string

### 2. Replaced the Payment Flow
- Replaced `handlePayInvoice` with a two-step process:
  - `initiatePayInvoiceProcess` - Validates and decodes the invoice, shows confirmation dialog
  - `confirmAndPayInvoice` - Executes the actual payment after user confirmation

### 3. Added Invoice Validation Checks
- Implemented validation to check for zero-amount invoices (not supported by Spark)
- Added proper error handling for invalid or malformed invoices
- Added clear user feedback via toast messages for various error conditions

### 4. Created Confirmation Dialog
- Added an AlertDialog component to display payment details before sending
- Shows payment amount, description, and partial invoice string
- Provides clear Cancel and Pay Now buttons
- Handles loading state during payment processing

### 5. Improved Fee Handling
- Implemented dynamic fee calculation based on payment amount (0.17% or minimum 5 sats)
- Added `maxFeeSats` parameter to the payment process for better fee control

### 6. Error Handling Improvements
- Added more granular error handling for various payment failure scenarios
- Improved error messages and user feedback
- Added clearer handling for fee-related errors

## Technical Implementation Notes

1. **bolt11 Decoding**:
   - Used the bolt11 library to parse and extract invoice details
   - Added proper error handling for parse failures
   - Mapped decoded invoice data fields to user-friendly information in the confirmation dialog

2. **Payment Process Flow**:
   ```
   User enters invoice → 
   initiatePayInvoiceProcess decodes and validates → 
   Confirmation dialog shown → 
   User confirms → 
   confirmAndPayInvoice executes payment → 
   Success/error feedback
   ```

3. **State Cleanup**:
   - Added proper cleanup of state variables after payment completion or cancellation
   - Added handlers to reset state when dialog is dismissed

4. **SDK Integration**:
   - Maintained backward compatibility by passing the function to the same prop name in SendPaymentCard
   - Added multiple fallback approaches for invoice payment to handle different SDK versions
   - Preserved all existing error handling logic while adding new specific cases

## Testing Notes
- Verified the implementation with type checking (pnpm run t), which passed successfully
- Ran linting (pnpm run lint), which showed only pre-existing issues

## Risks and Mitigation
- The bolt11 library might parse some invalid invoices without error, so the code includes additional validation logic
- The Spark SDK's API might change in future versions, so the code includes multiple fallback approaches
- To mitigate potential issues with zero-amount invoices, explicit checks were added before payment

## User Experience Improvements
- Users now see the exact amount and purpose of a payment before confirming
- Clear feedback is provided for unsupported invoice types (e.g., zero-amount)
- The confirmation dialog provides an additional security checkpoint
- Processing states are clearly indicated throughout the flow