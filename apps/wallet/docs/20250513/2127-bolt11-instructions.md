Okay, I understand. You want to add a step to inspect the Lightning invoice amount using the `bolt11` library *before* sending the payment. This allows the user to see and confirm the amount they are about to pay.

Here are the instructions for the coding agent:

**Objective:**

Integrate the `bolt11` library to decode Lightning invoices in the `SendPaymentCard` flow. Before calling `handlePayInvoice`, the application should decode the invoice, extract the amount, display it to the user for confirmation (e.g., in an `AlertDialog`), and only proceed with the payment if the user confirms. Also, check if the invoice has an amount, as Spark currently doesn't support zero-amount invoices.

**File to Modify:** `src/App.tsx` (primarily the `handlePayInvoice` function and related state)
**New Dependency:** `bolt11`

---

**I. Install `bolt11` Dependency**

1.  **Add the dependency:**
    Open your `package.json` file.
    Add `bolt11` to the `dependencies` section:
    ```json
    "bolt11": "^1.4.1", // Or the latest version
    ```
    (Note: Check npm for the latest stable version of `bolt11` and use that.)

2.  **Install the package:**
    Run your package manager's install command in the terminal:
    ```bash
    yarn install
    # or
    # pnpm install
    # or
    # npm install
    ```

**II. Modify `src/App.tsx`**

1.  **Import `bolt11`:**
    At the top of `src/App.tsx`, add the import for the `bolt11` library:
    ```typescript
    // ... other imports
    import * as bolt11 from 'bolt11';
    ```

2.  **Add New State for Decoded Invoice and Confirmation Dialog:**
    Add state to hold the decoded invoice details and manage the visibility of a confirmation dialog.
    ```typescript
    // ... after isSendingPayment state
    const [decodedInvoiceDetails, setDecodedInvoiceDetails] = useState<bolt11.PaymentRequestObject | null>(null);
    const [showPaymentConfirmDialog, setShowPaymentConfirmDialog] = useState(false);
    const [invoiceToPay, setInvoiceToPay] = useState<string>(''); // Store the invoice being confirmed
    ```

3.  **Update `handlePayInvoice` to `initiatePayInvoiceProcess`:**
    Rename the existing `handlePayInvoice` function to `initiatePayInvoiceProcess`. This function will now be responsible for decoding the invoice and showing the confirmation dialog. The actual payment will be handled by a new function.

    ```typescript
    // Rename existing handlePayInvoice
    const initiatePayInvoiceProcess = async () => {
      if (!sdkRef.current || !sdkRef.current.wallet) {
        toast.error("Wallet not connected or initialized properly.");
        return;
      }
      if (!sendInvoice.trim()) {
        toast.error("Please enter a Lightning invoice to pay.");
        return;
      }
      if (isSendingPayment) return; // Prevent multiple initiations

      const trimmedInvoice = sendInvoice.trim().replace(/\s+/g, '').replace(/[\u0000-\u001F\u007F-\u009F]/g, '');

      if (!trimmedInvoice.toLowerCase().startsWith('ln')) {
        toast.error("Invalid Lightning invoice format.", { description: "Must start with 'ln'." });
        return;
      }

      try {
        setIsSendingPayment(true); // Indicate processing has started
        toast.loading("Decoding invoice...", { id: "decode-invoice" });

        const decoded = bolt11.decode(trimmedInvoice);
        console.log("Decoded Bolt11 Invoice:", decoded);

        // Check for amount (Spark doesn't support zero-amount invoices yet)
        const amountSat = decoded.satoshis || (decoded.millisatoshis ? Math.floor(Number(decoded.millisatoshis) / 1000) : 0);

        if (amountSat === 0 && (!decoded.satoshis && !decoded.millisatoshis)) {
          // This means the invoice truly has no amount specified.
          toast.dismiss("decode-invoice");
          toast.error("Zero-Amount Invoice Not Supported", {
            description: "This wallet currently does not support paying invoices without a specified amount. Please use an invoice with an amount.",
          });
          setIsSendingPayment(false);
          return;
        }

        if (amountSat <= 0) {
            // This covers cases where millisatoshis might be present but very small, or explicitly zero.
            toast.dismiss("decode-invoice");
            toast.error("Invalid Invoice Amount", {
              description: "The invoice amount must be greater than 0 satoshis.",
            });
            setIsSendingPayment(false);
            return;
        }


        setDecodedInvoiceDetails(decoded);
        setInvoiceToPay(trimmedInvoice); // Store the validated invoice string
        setShowPaymentConfirmDialog(true);
        toast.dismiss("decode-invoice");

      } catch (error) {
        console.error('Failed to decode invoice:', error);
        toast.dismiss("decode-invoice");
        toast.error("Invalid Lightning Invoice", {
          description: error instanceof Error ? error.message : "Could not decode the provided invoice.",
        });
      } finally {
        // setIsSendingPayment(false); // Moved to after dialog confirmation or cancellation
      }
    };
    ```

4.  **Create `confirmAndPayInvoice` Function:**
    This new function will be called when the user confirms the payment in the dialog.

    ```typescript
    const confirmAndPayInvoice = async () => {
      if (!sdkRef.current || !sdkRef.current.wallet || !invoiceToPay) {
        toast.error("Payment cannot proceed. Wallet or invoice data missing.");
        setShowPaymentConfirmDialog(false);
        setIsSendingPayment(false);
        return;
      }
      if (isSendingPayment && !showPaymentConfirmDialog) return; // Already sending from a previous confirm call

      setShowPaymentConfirmDialog(false); // Close dialog
      // isSendingPayment should already be true if initiatePayInvoiceProcess was successful
      // If not, set it:
      if (!isSendingPayment) setIsSendingPayment(true);


      toast.loading("Processing payment...", { id: "pay-invoice" });

      try {
        const wallet = sdkRef.current.wallet || sdkRef.current;

        console.log("Attempting to pay confirmed invoice:", invoiceToPay);

        // Recommended maxFeeSats: greater of 5 sats or 0.17% (17 bps) of tx amount
        const amountMsat = decodedInvoiceDetails?.millisatoshis ? Number(decodedInvoiceDetails.millisatoshis) : (decodedInvoiceDetails?.satoshis ? Number(decodedInvoiceDetails.satoshis) * 1000 : 0);
        const amountSatsForFeeCalc = Math.floor(amountMsat / 1000);

        let calculatedMaxFee = BigInt(5); // Default 5 sats
        if (amountSatsForFeeCalc > 0) {
            const percentageFee = BigInt(Math.ceil(amountSatsForFeeCalc * 0.0017)); // 0.17%
            calculatedMaxFee = percentageFee > BigInt(5) ? percentageFee : BigInt(5);
        }
        const maxFeeSats = Number(calculatedMaxFee);

        console.log(`Paying invoice. Amount: ${amountSatsForFeeCalc} sats. Max Fee: ${maxFeeSats} sats.`);

        await wallet.payLightningInvoice({
          invoice: invoiceToPay,
          maxFeeSats: maxFeeSats // Add maxFeeSats as per Spark docs
        });

        console.log("Payment successful");

        toast.dismiss("pay-invoice");
        toast.success("Payment Sent Successfully!");
        setSendInvoice(''); // Clear the input field in SendPaymentCard
        setInvoiceToPay('');
        setDecodedInvoiceDetails(null);

        await fetchWalletData(sdkRef.current);

      } catch (error) {
        // ... (keep existing detailed error handling from the previous handlePayInvoice)
        console.error('Failed to pay invoice:', error);
        toast.dismiss("pay-invoice");
        const message = error instanceof Error ? error.message : String(error);
        // (Keep the specific error toast logic here)
        if (message.toLowerCase().includes("insufficient balance") || message.toLowerCase().includes("not enough funds")) {
            toast.error("Payment Failed: Insufficient balance.", { description: "Please check your balance and try again."});
        } else if (message.toLowerCase().includes("invalid invoice") || message.toLowerCase().includes("decode error") || message.toLowerCase().includes("malformed")) {
            toast.error("Payment Failed: Invalid invoice.", { description: "Please check the invoice string and try again."});
        } else if (message.toLowerCase().includes("route") || message.toLowerCase().includes("path not found") || message.toLowerCase().includes("no route")) {
            toast.error("Payment Failed: No route found.", { description: "Could not find a path to the destination."});
        } else if (message.toLowerCase().includes("timeout") || message.toLowerCase().includes("timed out")) {
            toast.error("Payment Failed: Timeout", { description: "The payment request timed out. Please try again." });
        } else if (message.toLowerCase().includes("invoice must be") || message.toLowerCase().includes("lightning payment request")) {
            toast.error("Payment Failed: Format issue", { description: "The invoice format was not recognized." });
        } else if (message.toLowerCase().includes("invalid amount") || message.toLowerCase().includes("amount")) {
            // This should be caught earlier by the zero-amount check now
            toast.error("Payment Failed: Amount issue", { description: "The payment amount in the invoice is invalid." });
        } else if (message.toLowerCase().includes("fee") && message.toLowerCase().includes("exceeds")) {
            toast.error("Payment Failed: Fee too high", { description: "The network fee exceeds the maximum allowed. Try again later." });
        }
        else {
            toast.error("Payment Failed", { description: message });
        }
      } finally {
        setIsSendingPayment(false);
        setDecodedInvoiceDetails(null); // Clear details after attempt
        setInvoiceToPay(''); // Clear invoice after attempt
      }
    };
    ```

5.  **Add `AlertDialog` for Payment Confirmation in `wallet_ready` screen:**
    Inside the `renderContent` function, within the `case 'wallet_ready':` block, add an `AlertDialog` component that will be controlled by `showPaymentConfirmDialog`. This dialog should display the decoded invoice amount and description.

    ```typescript
    // ... inside renderContent, case 'wallet_ready':
    // ... (existing JSX for wallet_ready screen) ...

            {/* Payment Confirmation Dialog */}
            <AlertDialog open={showPaymentConfirmDialog} onOpenChange={(open) => {
              if (!open) { // If dialog is closed (e.g. by Escape or clicking away)
                setShowPaymentConfirmDialog(false);
                setDecodedInvoiceDetails(null);
                setInvoiceToPay('');
                setIsSendingPayment(false); // Reset sending state if dialog is cancelled
              }
            }}>
              <AlertDialogContent>
                <AlertDialogHeader>
                  <AlertDialogTitle>Confirm Payment</AlertDialogTitle>
                  <AlertDialogDescription>
                    Please review the details before sending your payment.
                  </AlertDialogDescription>
                </AlertDialogHeader>
                {decodedInvoiceDetails && (
                  <div className="my-4 space-y-2">
                    <p>
                      <strong>Amount:</strong> ₿ {
                        decodedInvoiceDetails.satoshis ||
                        (decodedInvoiceDetails.millisatoshis ? Math.floor(Number(decodedInvoiceDetails.millisatoshis) / 1000) : 'N/A')
                      } sats
                    </p>
                    <p className="truncate">
                      <strong>Description:</strong> {
                        (decodedInvoiceDetails.tagsObject?.description as string) || // Access description via tagsObject
                        (decodedInvoiceDetails.tagsObject?.purpose_commit_string as string) || // Alternative for description
                        'No description'
                      }
                    </p>
                    <p className="text-xs text-muted-foreground break-all">
                      <strong>Invoice:</strong> {invoiceToPay.substring(0, 50)}...
                    </p>
                  </div>
                )}
                <AlertDialogFooter>
                  <AlertDialogCancel onClick={() => {
                     setShowPaymentConfirmDialog(false);
                     setDecodedInvoiceDetails(null);
                     setInvoiceToPay('');
                     setIsSendingPayment(false); // Explicitly reset on cancel
                  }}>Cancel</AlertDialogCancel>
                  <AlertDialogAction onClick={confirmAndPayInvoice} disabled={isSendingPayment && showPaymentConfirmDialog}>
                    {isSendingPayment && showPaymentConfirmDialog ? (
                        <>
                            <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                            Processing...
                        </>
                    ) : (
                        "Pay Now"
                    )}
                  </AlertDialogAction>
                </AlertDialogFooter>
              </AlertDialogContent>
            </AlertDialog>

            <div className="h-16" /> {/* Spacer for scroll */}
          </div>
        );
    ```

**III. Update `SendPaymentCard.tsx`**

1.  Open `src/components/SendPaymentCard.tsx`.
2.  Change the `handlePayInvoice` prop to `initiatePayInvoiceProcess` to match the renamed function in `App.tsx`.

    ```typescript
    // In SendPaymentCard.tsx props interface
    interface SendPaymentCardProps {
      // ... other props
      handlePayInvoice: () => Promise<void>; // Change this line
      // To:
      // initiatePayInvoiceProcess: () => Promise<void>;
      // ...
    }

    // In SendPaymentCard.tsx component definition
    // const SendPaymentCard: React.FC<SendPaymentCardProps> = ({
    //   ...
    //   handlePayInvoice, // Change this
    //   // To:
    //   // initiatePayInvoiceProcess,
    //   ...
    // }) => {
    // ...
    // <UiButton
    //   onClick={handlePayInvoice} // Change this
    //   // To:
    //   // onClick={initiatePayInvoiceProcess}
    // ...
    ```
    *Self-correction: It's simpler to keep the prop name `handlePayInvoice` in `SendPaymentCard.tsx` and just pass `initiatePayInvoiceProcess` to it from `App.tsx`. This avoids changing the `SendPaymentCard`'s internal prop name.*

    No changes needed in `SendPaymentCard.tsx` if you keep the prop name `handlePayInvoice` and just pass the new `initiatePayInvoiceProcess` function to it from `App.tsx`.
    In `App.tsx`, when rendering `SendPaymentCard`:
    ```jsx
    <SendPaymentCard
      // ... other props
      handlePayInvoice={initiatePayInvoiceProcess} // Pass the new function here
      // ...
    />
    ```

**IV. Testing**

1.  Ensure `bolt11` is installed.
2.  Run `pnpm run t` (or your type-checking script).
3.  Run `pnpm run dev`.
4.  **Test Decoding and Confirmation:**
    *   Go to the "Send Payment (Lightning)" card.
    *   Enter a valid Lightning invoice.
    *   Click "Pay Invoice".
    *   The confirmation dialog should appear, showing the amount and description from the decoded invoice.
5.  **Test Payment:**
    *   Click "Pay Now" in the dialog.
    *   The payment should proceed as before, with toasts and updates to balance/history.
6.  **Test Cancellation:**
    *   Click "Cancel" in the dialog. The payment should not proceed.
    *   The `isSendingPayment` state should be reset.
7.  **Test Invalid Invoice:**
    *   Enter an invalid/malformed Lightning invoice.
    *   Click "Pay Invoice".
    *   An error toast about decoding failure or invalid invoice should appear. The confirmation dialog should not open.
8.  **Test Zero-Amount Invoice:**
    *   If you can generate a zero-amount invoice, test pasting it.
    *   An error toast "Zero-Amount Invoice Not Supported" should appear.
    *   Ensure the amount check distinguishes between a truly zero-amount invoice and one where `satoshis` is null but `millisatoshis` implies a non-zero amount. The logic provided attempts to do this.

This adds a crucial confirmation step, improving user experience and safety when sending Lightning payments. Remember to handle the `maxFeeSats` in `confirmAndPayInvoice` appropriately for your application's fee strategy. The example uses a recommended calculation from Spark.
