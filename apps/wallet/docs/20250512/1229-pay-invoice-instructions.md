Okay, the transaction history display is looking good. Now, let's add a card for paying/sending Lightning invoices.

**Step-by-Step Instructions for Adding Send/Pay Invoice Functionality:**

**Step 1: Add State for Sending Payments in `src/App.tsx`**

1.  Open `src/App.tsx`.
2.  Add new state variables to manage the send payment flow:

    ```typescript
    // ... other useState hooks
    const [sendInvoice, setSendInvoice] = useState('');
    const [isSendingPayment, setIsSendingPayment] = useState(false);
    // You might want a state to store decoded invoice details if you plan to show them
    // const [decodedSendInvoice, setDecodedSendInvoice] = useState<any>(null); // 'any' for now, define type later
    ```

**Step 2: Implement `handlePayInvoice` Function in `src/App.tsx`**

1.  Add a new asynchronous function to handle paying an invoice:

    ```typescript
    // ... after handleGenerateInvoice or similar functions

    const handlePayInvoice = async () => {
      if (!sdkRef.current || !sdkRef.current.wallet) {
        toast.error("Wallet not connected or initialized properly.");
        return;
      }
      if (!sendInvoice.trim()) {
        toast.error("Please enter a Lightning invoice to pay.");
        return;
      }
      if (isSendingPayment) return;

      setIsSendingPayment(true);
      toast.loading("Processing payment...", { id: "pay-invoice" });

      try {
        const wallet = sdkRef.current.wallet || sdkRef.current; // Use the wallet instance

        // The Spark SDK's payLightningInvoice takes the BOLT11 string directly.
        // No separate decode step is strictly necessary for just paying,
        // but you might want to decode it for UI display (amount, memo) before sending.
        // For this step, we'll just pay directly.

        console.log("Attempting to pay invoice:", sendInvoice);
        const paymentResult = await wallet.payLightningInvoice({ invoice: sendInvoice });
        // The payLightningInvoice in Spark SDK returns a Promise<void> on success or throws an error.
        // It doesn't return detailed payment result object directly like some other SDKs might.
        // Success is implied if no error is thrown.
        console.log("Payment successful (implied, no error thrown):", paymentResult); // paymentResult will be undefined

        toast.dismiss("pay-invoice");
        toast.success("Payment Sent Successfully!");
        setSendInvoice(''); // Clear the input field

        // Re-fetch wallet data to update balance and transaction history
        await fetchWalletData(sdkRef.current);

      } catch (error) {
        console.error('Failed to pay invoice:', error);
        toast.dismiss("pay-invoice");
        const message = error instanceof Error ? error.message : "Unknown error during payment.";

        // More specific error messages
        if (message.toLowerCase().includes("insufficient balance")) {
            toast.error("Payment Failed: Insufficient balance.", { description: "Please check your balance and try again."});
        } else if (message.toLowerCase().includes("invalid invoice") || message.toLowerCase().includes("decode error")) {
            toast.error("Payment Failed: Invalid invoice.", { description: "Please check the invoice string and try again."});
        } else if (message.toLowerCase().includes("route") || message.toLowerCase().includes("path not found")) {
            toast.error("Payment Failed: No route found.", { description: "Could not find a path to the destination. The recipient might be offline or there might be network issues."});
        } else {
            toast.error("Payment Failed", { description: message });
        }
      } finally {
        setIsSendingPayment(false);
      }
    };
    ```

**Step 3: Create `SendPaymentCard.tsx` Component (`src/components/SendPaymentCard.tsx`)**

1.  Create a new file `src/components/SendPaymentCard.tsx`.
2.  Add the following content:

    ```typescript
    import React from 'react';
    import { Card, CardContent, CardHeader, CardTitle, CardDescription } from '@/components/ui/card';
    import { UiInput } from '@/components/ui/input'; // Renamed to avoid conflict
    import { UiButton } from '@/components/ui/button'; // Renamed to avoid conflict
    import { Label } from '@/components/ui/label';
    import { Loader2, Send } from 'lucide-react';

    interface SendPaymentCardProps {
      sendInvoice: string;
      setSendInvoice: (invoice: string) => void;
      handlePayInvoice: () => Promise<void>;
      isSendingPayment: boolean;
      disabled: boolean; // To disable when SDK is not ready
    }

    const SendPaymentCard: React.FC<SendPaymentCardProps> = ({
      sendInvoice,
      setSendInvoice,
      handlePayInvoice,
      isSendingPayment,
      disabled
    }) => {
      return (
        <Card className="mt-6">
          <CardHeader>
            <CardTitle>Send Payment (Lightning)</CardTitle>
            <CardDescription>Pay a Lightning invoice.</CardDescription>
          </CardHeader>
          <CardContent className="flex flex-col gap-4">
            <div className="space-y-2">
              <Label htmlFor="send-invoice">Lightning Invoice (BOLT11)</Label>
              <UiInput
                id="send-invoice"
                type="text"
                placeholder="lnbc..."
                value={sendInvoice}
                onChange={(e) => setSendInvoice(e.target.value)}
                disabled={isSendingPayment || disabled}
              />
            </div>
            <div className="flex justify-center">
              <UiButton
                onClick={handlePayInvoice}
                disabled={isSendingPayment || disabled || !sendInvoice.trim()}
              >
                {isSendingPayment ? (
                  <>
                    <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                    Sending...
                  </>
                ) : (
                  <>
                    <Send className="mr-2 h-4 w-4" />
                    Pay Invoice
                  </>
                )}
              </UiButton>
            </div>
          </CardContent>
        </Card>
      );
    };

    export default SendPaymentCard;
    ```

**Step 4: Integrate `SendPaymentCard` into `src/App.tsx`**

1.  **Import the new component:**
    At the top of `src/App.tsx`, add:
    ```typescript
    import SendPaymentCard from './components/SendPaymentCard';
    ```
2.  **Render the card in the `wallet_ready` state:**
    Inside the `renderCurrentScreen` function, in the `case 'wallet_ready':` block, add the `SendPaymentCard` component, perhaps between the "Receive" card and the "Transaction History" card.

    ```typescript
    // ... inside renderCurrentScreen, case 'wallet_ready':
              </CardContent>
            </Card> {/* This is the end of the Receive Bitcoin Card */}

            {/* Add Send Payment Card here */}
            <SendPaymentCard
              sendInvoice={sendInvoice}
              setSendInvoice={setSendInvoice}
              handlePayInvoice={handlePayInvoice}
              isSendingPayment={isSendingPayment}
              disabled={!sdkRef.current} // Disable if SDK is not ready
            />

            <TransactionHistoryCard transactions={transactions} />
            {/* ... */}
    ```

**Step 5: Update Log File**

Add the following section to your current log file (e.g., `docs/20250512/1128-spark-it-log.md` or `1159-log.md`):

```markdown
### 6. Added Send/Pay Invoice Functionality

- **Added State for Sending (`src/App.tsx`):**
  - Introduced `sendInvoice` (string) to hold the invoice to be paid.
  - Added `isSendingPayment` (boolean) to manage the loading state of the pay button.
- **Implemented `handlePayInvoice` Function (`src/App.tsx`):**
  - Asynchronous function to process the payment.
  - Validates if the SDK is ready and an invoice is provided.
  - Calls `sdkRef.current.wallet.payLightningInvoice({ invoice: sendInvoice })`.
  - Shows loading and success/error toasts using `sonner`.
  - Clears the invoice input field on success.
  - Re-fetches wallet data (balance and transactions) after a successful payment.
  - Includes more specific error messages for common payment failures (insufficient balance, invalid invoice, no route).
- **Created `SendPaymentCard.tsx` Component (`src/components/SendPaymentCard.tsx`):**
  - Provides a `Card` UI for pasting/entering a Lightning invoice.
  - Includes an `Input` field for the invoice and a "Pay Invoice" `Button`.
  - Button shows a loading state (`Loader2` icon) and disables input while `isSendingPayment` is true.
  - Button is also disabled if the SDK is not ready or if the invoice input is empty.
- **Integrated into `wallet_ready` Screen (`src/App.tsx`):**
  - Added `<SendPaymentCard ... />` to the main wallet view.
  - Passed necessary state and handler functions as props.
```

**Step 6: Testing**

1.  Run `pnpm run t` (or your type-checking script) to ensure no type errors.
2.  Run `pnpm run dev` (or your dev script).
3.  **Navigate to the wallet_ready screen.**
4.  **Test Paying an Invoice:**
    *   Obtain a valid Lightning invoice (e.g., from another wallet or a service like `ln.ാൽby`).
    *   Paste the invoice into the "Lightning Invoice (BOLT11)" input field in the "Send Payment" card.
    *   Click the "Pay Invoice" button.
    *   Observe the loading state and toast notifications.
    *   Verify that the payment succeeds (or fails with an appropriate error message).
    *   Check if your balance updates correctly.
    *   Check if the new outgoing transaction appears in the "Transaction History" card.
5.  **Test Error Cases:**
    *   Try paying an invalid invoice.
    *   Try paying an invoice that requires more funds than available in the wallet.
    *   Try paying an expired invoice (if you can generate one).

This completes the basic functionality for paying Lightning invoices using the Spark SDK. You can further enhance this by:
*   Adding QR code scanning for paying invoices.
*   Decoding the invoice before payment to display details like amount and memo to the user for confirmation.
*   More sophisticated fee estimation or selection if the Spark SDK provides options for `payLightningInvoice` (e.g., `maxFeeSats`). The current SDK version used by Blitz (`0.1.14`) might not have `maxFeeSats` in `payLightningInvoice` directly, so the SDK handles fees.
