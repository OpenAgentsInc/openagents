# Pay Invoice Implementation Log

## Date: May 12, 2025

### Overview

Added the ability to pay Lightning invoices to the OpenAgents Wallet application, complementing the existing functionality to generate and receive Lightning payments.

### Implementation Details

#### 1. Added State for Sending (`src/App.tsx`)
- Introduced `sendInvoice` (string) to hold the invoice to be paid.
- Added `isSendingPayment` (boolean) to manage the loading state of the payment button.

```typescript
const [sendInvoice, setSendInvoice] = useState('');
const [isSendingPayment, setIsSendingPayment] = useState(false);
```

#### 2. Created `SendPaymentCard.tsx` Component

Created a new component at `src/components/SendPaymentCard.tsx` that provides:
- A `Card` UI for pasting/entering a Lightning invoice
- An input field for the invoice string 
- A "Pay Invoice" button with loading state
- Proper disabled states based on SDK readiness and input validation

The component has a clean interface that takes:
```typescript
interface SendPaymentCardProps {
  sendInvoice: string;
  setSendInvoice: (invoice: string) => void;
  handlePayInvoice: () => Promise<void>;
  isSendingPayment: boolean;
  disabled: boolean; // To disable when SDK is not ready
}
```

#### 3. Implemented `handlePayInvoice` Function (`src/App.tsx`)

Added an asynchronous function to process Lightning payments:
- Validates if the SDK is ready and an invoice is provided
- Calls the Spark SDK's `payLightningInvoice({ invoice: sendInvoice })` method
- Shows loading and success/error toasts using `sonner`
- Clears the invoice input field on successful payment
- Re-fetches wallet data (balance and transactions) after payment completion
- Includes specific error handling for common payment issues:
  - Insufficient balance
  - Invalid invoice
  - No route found to recipient

#### 4. Integrated into Wallet UI

Added the `SendPaymentCard` component to the main wallet view, positioned between the "Receive Bitcoin" card and the "Transaction History" card, creating a logical flow:
1. View Balance
2. Receive Bitcoin
3. Send Bitcoin (new)
4. View Transaction History

### Testing Notes

The implementation has been tested with:
- Type checking to ensure TypeScript compliance
- Payment validations against empty/invalid inputs
- Error handling for various failure cases
- UI responsiveness during payment processing

### Next Steps / Future Enhancements

Potential future improvements include:
- QR code scanning for invoice input
- Invoice decoding before payment to show amount, recipient, and memo for confirmation
- Advanced fee controls if supported by the SDK
- Ability to add personal notes to outgoing payments
- Saving frequently used recipients

### Conclusion

The send payment functionality completes the basic wallet capabilities, allowing users to both receive and send Bitcoin over Lightning Network. This implementation follows the same design patterns and error handling as the existing receive functionality, providing a consistent user experience.