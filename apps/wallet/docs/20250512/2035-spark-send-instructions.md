You got it. The previous steps successfully integrated transaction history. Now, let's add functionality to display the user's Spark address and allow them to send payments to another Spark address.

**Step-by-Step Instructions for Adding Spark Address Display and Send Functionality:**

**Step 1: Add State for Spark Address and Send-to-Spark Flow in `src/App.tsx`**

1.  Open `src/App.tsx`.
2.  **Add state for the user's Spark address:**
    Below your other `useState` hooks for wallet UI state, add:
    ```typescript
    const [userSparkAddress, setUserSparkAddress] = useState<string>('');
    ```
3.  **Add state for the send-to-Spark flow:**
    ```typescript
    const [recipientSparkAddress, setRecipientSparkAddress] = useState('');
    const [sendSparkAmount, setSendSparkAmount] = useState(BigInt(0)); // Or a suitable default
    const [isSendingSparkPayment, setIsSendingSparkPayment] = useState(false);
    ```

**Step 2: Fetch and Display User's Spark Address in `fetchWalletData` and UI**

1.  **Modify `fetchWalletData` in `src/App.tsx`:**
    Update this function to also fetch and set the user's Spark address.

    ```typescript
    const fetchWalletData = useCallback(async (sdk: any) => {
      if (!sdk) return;
      const wallet = sdk.wallet || sdk;

      try {
        // Fetch balance
        const balanceData = await wallet.getBalance();
        setWalletInfo({
          balanceSat: balanceData.balance || BigInt(0),
          tokenBalances: balanceData.tokenBalances
        });

        // Fetch transactions (existing code)
        // ... (keep existing transaction fetching logic) ...
        try {
          const transfersResponse = await wallet.getTransfers(20, 0);
          if (transfersResponse && transfersResponse.transfers) {
            const sortedTransactions = transfersResponse.transfers.sort(
              (a: SparkTransferData, b: SparkTransferData) =>
                new Date(b.createdTime || b.created_at_time || 0).getTime() - new Date(a.createdTime || a.created_at_time || 0).getTime()
            );
            setTransactions(sortedTransactions);
          } else {
            setTransactions([]);
          }
        } catch (txError) {
          console.error('Failed to fetch transactions:', txError);
          setTransactions([]);
        }

        // Fetch user's Spark Address
        try {
          const sparkAddr = await wallet.getSparkAddress();
          setUserSparkAddress(sparkAddr);
        } catch (addrError) {
          console.error('Failed to fetch Spark address:', addrError);
          toast.error("Could not fetch your Spark address.");
        }

      } catch (error) {
        console.error('Failed to fetch wallet data:', error);
        toast.error("Failed to fetch wallet data. Some features may be limited.");
      }
    }, []); // Keep empty dependency array
    ```

2.  **Display User's Spark Address (`src/App.tsx` `wallet_ready` screen):**
    Add a new `Card` component to display the user's Spark address. This card can be placed, for example, below the "Bitcoin Balance" card.

    ```typescript
    // ... inside renderCurrentScreen, case 'wallet_ready':
    // ... after Bitcoin Balance Card ...

            <Card className="mb-6">
              <CardHeader>
                <CardTitle>Your Spark Address</CardTitle>
                <CardDescription>Share this address to receive Spark payments.</CardDescription>
              </CardHeader>
              <CardContent>
                {userSparkAddress ? (
                  <div className="space-y-2">
                    <ScrollArea className="h-16 w-full rounded-md border p-2">
                      <div className="p-1 font-mono text-sm break-all">
                        {userSparkAddress}
                      </div>
                    </ScrollArea>
                    <UiButton
                      variant="outline"
                      size="sm"
                      className="w-full"
                      onClick={() => {
                        navigator.clipboard.writeText(userSparkAddress);
                        toast.success("Spark Address Copied!");
                      }}
                    >
                      Copy Address
                    </UiButton>
                  </div>
                ) : (
                  <p className="text-sm text-muted-foreground">Fetching your Spark address...</p>
                )}
              </CardContent>
            </Card>

            {/* ... rest of the cards (Receive Lightning, Send Lightning, Transaction History) ... */}
    ```

**Step 3: Implement `handleSendSparkPayment` Function in `src/App.tsx`**

1.  Add a new asynchronous function to handle sending payments to a Spark address:

    ```typescript
    // ... after handlePayInvoice function

    const handleSendSparkPayment = async () => {
      if (!sdkRef.current || !sdkRef.current.wallet) {
        toast.error("Wallet not connected or initialized properly.");
        return;
      }
      if (!recipientSparkAddress.trim()) {
        toast.error("Please enter a recipient's Spark address.");
        return;
      }
      if (sendSparkAmount <= BigInt(0)) {
        toast.error("Please enter a valid amount greater than 0.");
        return;
      }
      if (isSendingSparkPayment) return;

      setIsSendingSparkPayment(true);
      toast.loading("Sending Spark payment...", { id: "send-spark-payment" });

      try {
        const wallet = sdkRef.current.wallet || sdkRef.current;
        const amountSatsNumber = Number(sendSparkAmount); // SDK expects number

        console.log("Attempting to send Spark payment to:", recipientSparkAddress, "Amount:", amountSatsNumber);

        // The transfer method in Spark SDK is direct.
        // It might return a Transfer object upon successful initiation.
        const transferResult = await wallet.transfer({
          receiverSparkAddress: recipientSparkAddress.trim(),
          amountSats: amountSatsNumber,
        });

        console.log("Spark transfer initiated:", transferResult); // transferResult might be void or basic info

        toast.dismiss("send-spark-payment");
        toast.success("Spark Payment Sent Successfully!");
        setRecipientSparkAddress(''); // Clear recipient address
        setSendSparkAmount(BigInt(0)); // Clear amount

        // Re-fetch wallet data to update balance and transaction history
        await fetchWalletData(sdkRef.current);

      } catch (error) {
        console.error('Failed to send Spark payment:', error);
        toast.dismiss("send-spark-payment");
        const message = error instanceof Error ? error.message : "Unknown error during Spark payment.";

        if (message.toLowerCase().includes("insufficient balance")) {
            toast.error("Payment Failed: Insufficient balance.");
        } else if (message.toLowerCase().includes("invalid address") || message.toLowerCase().includes("receiver address")) {
            toast.error("Payment Failed: Invalid recipient Spark address.");
        } else if (message.toLowerCase().includes("amount")) {
             toast.error("Payment Failed: Invalid amount.");
        } else {
            toast.error("Spark Payment Failed", { description: message });
        }
      } finally {
        setIsSendingSparkPayment(false);
      }
    };
    ```

**Step 4: Create `SendSparkPaymentCard.tsx` Component (`src/components/SendSparkPaymentCard.tsx`)**

1.  Create a new file `src/components/SendSparkPaymentCard.tsx`.
2.  Add the following content:

    ```typescript
    import React from 'react';
    import { Card, CardContent, CardHeader, CardTitle, CardDescription } from '@/components/ui/card';
    import { UiInput } from '@/components/ui/input';
    import { UiButton } from '@/components/ui/button';
    import { Label } from '@/components/ui/label';
    import { Loader2, Sparkles } from 'lucide-react'; // Using Sparkles for Spark icon

    interface SendSparkPaymentCardProps {
      recipientSparkAddress: string;
      setRecipientSparkAddress: (address: string) => void;
      sendSparkAmount: bigint;
      setSendSparkAmount: (amount: bigint) => void;
      handleSendSparkPayment: () => Promise<void>;
      isSendingSparkPayment: boolean;
      disabled: boolean;
    }

    const SendSparkPaymentCard: React.FC<SendSparkPaymentCardProps> = ({
      recipientSparkAddress,
      setRecipientSparkAddress,
      sendSparkAmount,
      setSendSparkAmount,
      handleSendSparkPayment,
      isSendingSparkPayment,
      disabled
    }) => {
      const handleAmountChange = (e: React.ChangeEvent<HTMLInputElement>) => {
        const value = e.target.value;
        // Allow empty string for clearing, otherwise parse as BigInt
        setSendSparkAmount(value === '' ? BigInt(0) : BigInt(value.replace(/[^0-9]/g, '')));
      };

      const isFormValid = recipientSparkAddress.trim().startsWith("sprt1p") && sendSparkAmount > BigInt(0);

      return (
        <Card className="mt-6">
          <CardHeader>
            <CardTitle>Send to Spark Address</CardTitle>
            <CardDescription>Send Bitcoin instantly to another Spark wallet.</CardDescription>
          </CardHeader>
          <CardContent className="flex flex-col gap-4">
            <div className="space-y-2">
              <Label htmlFor="recipient-spark-address">Recipient's Spark Address</Label>
              <UiInput
                id="recipient-spark-address"
                type="text"
                placeholder="sprt1p..."
                value={recipientSparkAddress}
                onChange={(e) => setRecipientSparkAddress(e.target.value)}
                disabled={isSendingSparkPayment || disabled}
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="send-spark-amount">Amount (sats)</Label>
              <UiInput
                id="send-spark-amount"
                type="number" // Use number for easier input, convert to BigInt in handler
                value={sendSparkAmount.toString()}
                onChange={handleAmountChange}
                min="1"
                disabled={isSendingSparkPayment || disabled}
              />
            </div>
            <div className="flex justify-center">
              <UiButton
                onClick={handleSendSparkPayment}
                disabled={isSendingSparkPayment || disabled || !isFormValid}
              >
                {isSendingSparkPayment ? (
                  <>
                    <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                    Sending...
                  </>
                ) : (
                  <>
                    <Sparkles className="mr-2 h-4 w-4" /> {/* Spark icon */}
                    Send Spark Payment
                  </>
                )}
              </UiButton>
            </div>
          </CardContent>
        </Card>
      );
    };

    export default SendSparkPaymentCard;
    ```

**Step 5: Integrate `SendSparkPaymentCard` into `src/App.tsx`**

1.  **Import the new component:**
    At the top of `src/App.tsx`, add:
    ```typescript
    import SendSparkPaymentCard from './components/SendSparkPaymentCard';
    ```
2.  **Render the card in the `wallet_ready` state:**
    Inside the `renderCurrentScreen` function, in `case 'wallet_ready':`, add the `SendSparkPaymentCard` component. A good place might be after the "Send Payment (Lightning)" card.

    ```typescript
    // ... inside renderCurrentScreen, case 'wallet_ready':
    // ... after SendPaymentCard (for Lightning) ...

            {/* Add Send Spark Payment Card here */}
            <SendSparkPaymentCard
              recipientSparkAddress={recipientSparkAddress}
              setRecipientSparkAddress={setRecipientSparkAddress}
              sendSparkAmount={sendSparkAmount}
              setSendSparkAmount={setSendSparkAmount}
              handleSendSparkPayment={handleSendSparkPayment}
              isSendingSparkPayment={isSendingSparkPayment}
              disabled={!sdkRef.current}
            />

            <TransactionHistoryCard transactions={transactions} />
            {/* ... */}
    ```

**Step 6: Update Log File**

Add the following section to your current log file:

```markdown
### 7. Added Spark Address Display and Send-to-Spark Functionality

- **State for Spark Address and Send Flow (`src/App.tsx`):**
  - Added `userSparkAddress` state to store and display the user's own Spark address.
  - Introduced `recipientSparkAddress`, `sendSparkAmount`, and `isSendingSparkPayment` states for the send-to-Spark payment flow.
- **Fetch User's Spark Address (`fetchWalletData` in `src/App.tsx`):**
  - Extended `fetchWalletData` to call `sdkRef.current.wallet.getSparkAddress()` and update `userSparkAddress` state.
- **Display User's Spark Address (UI in `src/App.tsx`):**
  - Added a new `Card` in the `wallet_ready` screen to display `userSparkAddress`.
  - Included a "Copy Address" button for convenience.
- **Implemented `handleSendSparkPayment` Function (`src/App.tsx`):**
  - New asynchronous function to handle sending payments to a Spark address.
  - Validates SDK readiness, recipient address, and amount.
  - Calls `sdkRef.current.wallet.transfer({ receiverSparkAddress, amountSats: number })`.
  - Manages loading state (`isSendingSparkPayment`) and provides user feedback with `sonner` toasts for success or specific errors.
  - Clears input fields and re-fetches wallet data on successful payment.
- **Created `SendSparkPaymentCard.tsx` Component (`src/components/SendSparkPaymentCard.tsx`):**
  - Provides a `Card` UI for sending payments to a Spark address.
  - Includes input fields for the recipient's Spark address and the amount in satoshis.
  - Features a "Send Spark Payment" button with loading state and appropriate disabled states.
  - Includes basic client-side validation for the address format (starts with "sprt1p") and amount (>0).
- **Integrated `SendSparkPaymentCard` (`src/App.tsx`):**
  - Added the new card to the `wallet_ready` screen, allowing users to initiate Spark-to-Spark transfers.
  - Passed necessary state and handler props to the component.
```

**Step 7: Testing**

1.  Run `pnpm run t` for type checking.
2.  Run `pnpm run dev`.
3.  **Verify User's Spark Address Display:**
    *   On the `wallet_ready` screen, check if your own Spark address is displayed correctly in its new card.
    *   Test the "Copy Address" button.
4.  **Test Sending to a Spark Address:**
    *   You'll need a second Spark address to send to. If you have another instance of this wallet (or another Spark-compatible wallet), get its Spark address.
    *   In the "Send to Spark Address" card:
        *   Enter the recipient's Spark address.
        *   Enter an amount in satoshis.
        *   Click "Send Spark Payment".
    *   Observe loading states and toasts.
    *   Confirm the payment succeeds or fails with an appropriate error.
    *   Check if your balance updates.
    *   Check if the transaction appears in the history of both the sender and receiver wallets (if you control both).
5.  **Test Error Cases for Sending to Spark:**
    *   Invalid recipient Spark address format.
    *   Sending 0 or a negative amount.
    *   Attempting to send more than your available balance.

This completes the implementation for displaying the user's Spark address and sending Spark-to-Spark payments. Remember that real-time updates for incoming Spark transfers would ideally be handled by Spark SDK events if available, similar to how the Blitz example handled `transfer:claimed`. For now, `fetchWalletData` is called after successful sends to update the UI.
