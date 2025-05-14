Okay, I understand. We'll focus *only* on replacing the problematic `bolt11` decoding for Lightning invoices with `light-bolt11-decoder` and moving this specific Lightning decoding logic into a utility file. We will ignore the `sparkPaymenWrapper`, Bitcoin address validation, and Spark address decoding from the example code for now, as per your instruction to focus on the "relevant utils for our current issue."

The "current issue" is the `bolt11` library failing. `light-bolt11-decoder` is a good alternative as it's generally more lightweight and less prone to crypto polyfill issues in browser environments.

Here are the instructions:

---

**I. Install/Verify Dependencies**

1.  **Ensure `light-bolt11-decoder` is installed:**
    If you haven't already, or if you were using `bolt11` and want to switch completely:
    ```bash
    pnpm remove bolt11 # If you want to remove the old one
    pnpm add light-bolt11-decoder
    ```
    (If you already installed `light-bolt11-decoder` based on the example, ensure it's in `package.json`.)

**II. Create Lightning Invoice Utility File**

1.  Create a new file: `src/lib/invoice-utils.ts`
2.  Add the following code to this file:

    ```typescript
    // src/lib/invoice-utils.ts
    import { decode as decodeLnInvoice, type DecodedLightningInvoice } from 'light-bolt11-decoder';

    export interface DecodedLnInvoiceInfo {
      paymentRequest: string; // The original, cleaned invoice string
      amountSat: number;     // Amount in satoshis
      description?: string;
      timestamp?: number;     // Invoice creation timestamp (seconds)
      expiry?: number;        // Invoice expiry in seconds from timestamp
      paymentHash?: string;
      payeeNodeKey?: string;
    }

    export function decodeLightningInvoice(invoiceString: string): DecodedLnInvoiceInfo | null {
      const cleanedInvoice = invoiceString.trim().toLowerCase();

      if (!cleanedInvoice.startsWith('lnbc') && !cleanedInvoice.startsWith('lntb') && !cleanedInvoice.startsWith('lnbcrt')) {
        console.error('Invalid Lightning invoice prefix.');
        return null;
      }

      try {
        const decoded: DecodedLightningInvoice = decodeLnInvoice(cleanedInvoice);

        let amountSat = 0;
        const amountSection = decoded.sections.find(s => s.name === 'amount');
        if (amountSection && amountSection.value) {
          // Amount in light-bolt11-decoder is in millisatoshis
          amountSat = Math.floor(Number(amountSection.value) / 1000);
        }

        const descriptionSection = decoded.sections.find(s => s.name === 'description');
        const description = descriptionSection?.value?.toString();

        const timestampSection = decoded.sections.find(s => s.name === 'timestamp');
        const timestamp = timestampSection?.value ? Number(timestampSection.value) : undefined;

        const expirySection = decoded.sections.find(s => s.name === 'expiry');
        const expiry = expirySection?.value ? Number(expirySection.value) : undefined;

        const paymentHashSection = decoded.sections.find(s => s.name === 'payment_hash');
        const paymentHash = paymentHashSection?.value?.toString();

        const payeeNodeKey = decoded.payeeNodeKey;


        return {
          paymentRequest: cleanedInvoice, // Return the cleaned invoice for payment
          amountSat,
          description,
          timestamp,
          expiry,
          paymentHash,
          payeeNodeKey
        };
      } catch (error) {
        console.error('Failed to decode Lightning invoice with light-bolt11-decoder:', error);
        return null;
      }
    }
    ```

**III. Modify `src/App.tsx`**

1.  **Update Imports:**
    *   Remove any imports related to the old `bolt11` or `bolt11Pkg`.
    *   Remove the custom `decodeBolt11` helper function if it's still present in `App.tsx`.
    *   Import the new `decodeLightningInvoice` and `DecodedLnInvoiceInfo` from `invoice-utils.ts`.

    ```typescript
    // Remove these if they exist:
    // import bolt11Pkg from 'bolt11';
    // import * as bolt11 from 'bolt11'; // Or any other bolt11 import
    // function decodeBolt11(invoice: string): any { ... } // Remove this helper from App.tsx

    // Add this:
    import { decodeLightningInvoice, type DecodedLnInvoiceInfo } from './lib/invoice-utils';
    // Adjust path if necessary
    ```

2.  **Update `decodedInvoiceDetails` State Type:**
    Change the type definition for `decodedInvoiceDetails` to use `DecodedLnInvoiceInfo`.

    ```typescript
    // Remove or comment out old type:
    // type PaymentRequestObject = { /* ... */ };

    // Update state to use the new interface:
    const [decodedInvoiceDetails, setDecodedInvoiceDetails] = useState<DecodedLnInvoiceInfo | null>(null);
    ```

3.  **Modify `initiatePayInvoiceProcess` Function:**
    This function will now use the new `decodeLightningInvoice` utility.

    ```typescript
    const initiatePayInvoiceProcess = async () => {
      if (!sdkRef.current) {
        toast.error("Wallet not connected.");
        return;
      }
      const trimmedUserInput = sendInvoice.trim(); // User input from the text field
      if (!trimmedUserInput) {
        toast.error("Please enter a Lightning invoice to pay.");
        return;
      }
      if (isSendingPayment) return;

      setIsSendingPayment(true); // Indicate processing has started (decoding is part of processing)
      toast.loading("Decoding invoice...", { id: "decode-invoice" });

      try {
        // Use the new utility function for decoding
        const decoded = decodeLightningInvoice(trimmedUserInput);

        if (!decoded) {
          toast.error("Invalid Lightning Invoice", {
            description: "Could not decode the provided invoice. Please ensure it's a valid BOLT11 invoice.",
          });
          setIsSendingPayment(false);
          toast.dismiss("decode-invoice");
          return;
        }

        // Check for amount (Spark doesn't support zero-amount invoices yet)
        // The 'decoded.amountSat' is already in satoshis from our utility
        if (decoded.amountSat <= 0) {
          toast.error("Invalid or Zero-Amount Invoice", {
            description: "This wallet currently supports paying invoices with an amount greater than 0 satoshis.",
          });
          setIsSendingPayment(false);
          toast.dismiss("decode-invoice");
          return;
        }

        setDecodedInvoiceDetails(decoded);
        setInvoiceToPay(decoded.paymentRequest); // Use the cleaned paymentRequest from the decoder
        setShowPaymentConfirmDialog(true);
        toast.dismiss("decode-invoice");
        //setIsSendingPayment(false); // Keep true until payment attempt or cancel

      } catch (error) { // This catch is for unexpected errors, specific decode errors handled by decodeLightningInvoice returning null
        console.error('Error in initiatePayInvoiceProcess:', error);
        toast.dismiss("decode-invoice");
        toast.error("Error Processing Invoice", {
          description: error instanceof Error ? error.message : "An unexpected error occurred.",
        });
        setIsSendingPayment(false);
      }
    };
    ```

4.  **Modify `confirmAndPayInvoice` Function:**
    Ensure this function uses the `amountSat` from `decodedInvoiceDetails` for the fee calculation. The core payment logic remains the same.

    ```typescript
    const confirmAndPayInvoice = async () => {
      if (!sdkRef.current || !invoiceToPay || !decodedInvoiceDetails) {
        toast.error("Payment cannot proceed. Wallet or invoice data missing.");
        setShowPaymentConfirmDialog(false);
        setIsSendingPayment(false);
        return;
      }

      setShowPaymentConfirmDialog(false);
      toast.loading("Processing payment...", { id: "pay-invoice" });

      try {
        const wallet = sdkRef.current;

        // decodedInvoiceDetails.amountSat is already in satoshis
        const amountSatsForFeeCalc = decodedInvoiceDetails.amountSat;

        let calculatedMaxFee = BigInt(5);
        if (amountSatsForFeeCalc > 0) {
            const percentageFee = BigInt(Math.ceil(amountSatsForFeeCalc * 0.0017)); // 0.17%
            calculatedMaxFee = percentageFee > BigInt(5) ? percentageFee : BigInt(5);
        }
        const maxFeeSats = Number(calculatedMaxFee);

        console.log(`Paying invoice: ${invoiceToPay}. Amount: ${amountSatsForFeeCalc} sats. Max Fee: ${maxFeeSats} sats.`);

        await wallet.payLightningInvoice({
          invoice: invoiceToPay, // This is the BOLT11 string from decoded.paymentRequest
          maxFeeSats: maxFeeSats,
        });

        toast.dismiss("pay-invoice");
        toast.success("Payment Sent Successfully!");
        setSendInvoice('');
        setInvoiceToPay('');
        setDecodedInvoiceDetails(null);
        await fetchWalletData(sdkRef.current);

      } catch (error) {
        console.error('Failed to pay invoice:', error);
        toast.dismiss("pay-invoice");
        const message = error instanceof Error ? error.message : String(error);
        // (Keep your existing specific error toast logic here)
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
            toast.error("Payment Failed: Amount issue", { description: "The payment amount in the invoice is invalid." });
        } else if (message.toLowerCase().includes("fee") && message.toLowerCase().includes("exceeds")) {
            toast.error("Payment Failed: Fee too high", { description: "The network fee exceeds the maximum allowed. Try again later." });
        } else {
            toast.error("Payment Failed", { description: message });
        }
      } finally {
        setIsSendingPayment(false);
        setDecodedInvoiceDetails(null);
        setInvoiceToPay('');
      }
    };
    ```

5.  **Update Confirmation Dialog Display (`AlertDialog` in `wallet_ready` screen):**
    Adjust the dialog to display details from the `DecodedLnInvoiceInfo` structure.

    ```typescript
    // ... inside renderContent, case 'wallet_ready':
    // Find the AlertDialog for payment confirmation

    {decodedInvoiceDetails && (
      <div className="my-4 space-y-2">
        <p>
          <strong>Amount:</strong> ₿ {decodedInvoiceDetails.amountSat.toLocaleString()} sats
        </p>
        <p className="truncate">
          <strong>Description:</strong> {decodedInvoiceDetails.description || 'No description'}
        </p>
        <p className="text-xs text-muted-foreground break-all" title={invoiceToPay}>
          <strong>Invoice:</strong> {invoiceToPay.substring(0, 50)}...
        </p>
        {/* Optionally display other info like expiry if needed */}
        {/*
        {decodedInvoiceDetails.timestamp && decodedInvoiceDetails.expiry && (
          <p className="text-xs text-muted-foreground">
            Expires: {new Date((decodedInvoiceDetails.timestamp + decodedInvoiceDetails.expiry) * 1000).toLocaleString()}
          </p>
        )}
        */}
      </div>
    )}
    // ... rest of AlertDialog ...
    ```

**IV. Testing**

1.  Ensure `light-bolt11-decoder` is installed and `bolt11` (the old one) is potentially removed or not used.
2.  Run `pnpm run t` (type check) and `pnpm run lint`.
3.  Run `pnpm run dev`.
4.  **Test Lightning Invoice Decoding:**
    *   Enter a valid Lightning invoice in the "Send Bitcoin (Lightning)" card.
    *   Click "Pay Invoice".
    *   The confirmation dialog should appear with the correct amount and description.
    *   The console should not show the `CipherBase` error previously seen with `bolt11`.
5.  **Test Payment:**
    *   Confirm the payment. It should proceed as before.
6.  **Test Invalid Invoice Prefixes:**
    *   Enter text that doesn't start with `lnbc`, `lntb`, or `lnbcrt`. The `decodeLightningInvoice` utility should return `null`, and `initiatePayInvoiceProcess` should show an "Invalid Lightning Invoice" toast.
7.  **Test Zero/Negative Amount Invoices:**
    *   If `light-bolt11-decoder` successfully decodes an invoice but `amountSat` is <= 0, your check in `initiatePayInvoiceProcess` should catch this and show the appropriate error toast.

**V. Update Log File**

Create or update `docs/20250513/2127-bolt11-log.md` (or a new log file if this is a distinct task from the previous `bolt11` attempt):

```markdown
# Date: May 13, 2025
# Task: Refactor Lightning Invoice Decoding using `light-bolt11-decoder`

## Objective
Replace the previous `bolt11` library integration (which caused crypto polyfill issues) with `light-bolt11-decoder` for parsing Lightning invoices before payment. Move the decoding logic to a dedicated utility file.

## Implementation Details

### 1. Dependencies:
- Ensured `light-bolt11-decoder` is installed.
- (Optional: Removed the old `bolt11` dependency if it was causing conflicts).

### 2. Created Utility File (`src/lib/invoice-utils.ts`):
- Defined `DecodedLnInvoiceInfo` interface to structure the output of the decoding function (amount in sats, description, original payment request, etc.).
- Implemented `decodeLightningInvoice(invoiceString: string): DecodedLnInvoiceInfo | null`:
    - Takes a BOLT11 string, cleans it (trims, toLowerCase).
    - Validates the invoice prefix (`lnbc`, `lntb`, `lnbcrt`).
    - Uses `decode` from `light-bolt11-decoder`.
    - Extracts amount (converts from millisatoshis to satoshis), description, timestamp, expiry, payment hash, and payee node key.
    - Returns the `DecodedLnInvoiceInfo` object or `null` if decoding fails or prefix is invalid.
    - This utility is focused solely on Lightning invoice decoding.

### 3. Updated `src/App.tsx`:
- **Imports:** Removed old `bolt11` imports and helper. Imported `decodeLightningInvoice` and `DecodedLnInvoiceInfo` from `invoice-utils.ts`.
- **State:** Changed the type of `decodedInvoiceDetails` state to `DecodedLnInvoiceInfo | null`.
- **`initiatePayInvoiceProcess` Function:**
    - Now calls `decodeLightningInvoice` from the new utility.
    - Handles the `null` return case (decoding error) by showing an appropriate toast.
    - Continues to check for zero/negative amounts from the decoded `amountSat` and shows an error if applicable.
    - Sets `decodedInvoiceDetails` and `invoiceToPay` upon successful decoding to trigger the confirmation dialog.
- **`confirmAndPayInvoice` Function:**
    - Uses `decodedInvoiceDetails.amountSat` for calculating `maxFeeSats`.
    - The core `wallet.payLightningInvoice` call remains the same, using `invoiceToPay` (the cleaned invoice string) and `maxFeeSats`.
- **Confirmation Dialog:**
    - Updated to display `amountSat` and `description` from the `DecodedLnInvoiceInfo` object.

## Outcome:
- The application now uses `light-bolt11-decoder` for a more reliable Lightning invoice decoding experience in the browser, avoiding previous crypto-related errors.
- Invoice decoding logic is modularized in `src/lib/invoice-utils.ts`.
- The pre-payment confirmation flow (displaying amount/description) remains functional.
- Error handling for invalid invoice formats and zero-amount invoices is maintained.
```

This approach isolates the Lightning decoding, makes it more robust by using a library better suited for browser environments, and keeps the `App.tsx` cleaner by moving utility logic out.
