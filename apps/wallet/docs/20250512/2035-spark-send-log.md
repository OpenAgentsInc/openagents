# Spark Address and Send Functionality Implementation Log

## Date: May 12, 2025

### Overview

Added functionality to display the user's Spark address and allow them to send payments directly to another Spark address, enhancing the wallet's utility for direct Spark-to-Spark transfers.

### Implementation Details

#### 1. Added State for Spark Address and Send Flow (`src/App.tsx`)

- Added `userSparkAddress` state to store and display the user's own Spark address:
  ```typescript
  const [userSparkAddress, setUserSparkAddress] = useState<string>('');
  ```

- Introduced state variables for the send-to-Spark payment flow:
  ```typescript
  const [recipientSparkAddress, setRecipientSparkAddress] = useState('');
  const [sendSparkAmount, setSendSparkAmount] = useState(BigInt(0));
  const [isSendingSparkPayment, setIsSendingSparkPayment] = useState(false);
  ```

#### 2. Extended `fetchWalletData` to Retrieve User's Spark Address

Modified the wallet data fetching function to also retrieve the user's Spark address:

```typescript
// Fetch user's Spark Address
try {
  const sparkAddr = await wallet.getSparkAddress();
  setUserSparkAddress(sparkAddr);
} catch (addrError) {
  console.error('Failed to fetch Spark address:', addrError);
  toast.error("Could not fetch your Spark address.");
}
```

#### 3. Implemented `handleSendSparkPayment` Function

Added a comprehensive function to handle the sending of payments to other Spark addresses:

```typescript
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
    const transferResult = await wallet.transfer({
      receiverSparkAddress: recipientSparkAddress.trim(),
      amountSats: amountSatsNumber,
    });

    console.log("Spark transfer initiated:", transferResult);

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

This function:
- Validates all inputs before proceeding
- Manages UI state during payment processing
- Uses the Spark SDK's transfer method
- Handles errors with specific feedback
- Clears input fields on success
- Updates wallet data after completed transactions

#### 4. Created UI Components for Displaying and Sending

**Display User's Spark Address:**
Added a Card component to display the user's own Spark address with a copy button:

```typescript
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
```

**SendSparkPaymentCard Component:**
Created a new component for sending payments to Spark addresses:

```typescript
import React from 'react';
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from '@/components/ui/card';
import { Input as UiInput } from '@/components/ui/input';
import { Button as UiButton } from '@/components/ui/button';
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
  /* props */
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
      <CardContent>
        {/* Input fields and button */}
      </CardContent>
    </Card>
  );
};
```

The component provides:
- Input field for recipient's Spark address with basic validation
- Amount input with BigInt conversion
- Send button with loading state
- Visual feedback with Sparkles icon

#### 5. Integration in the Wallet UI

Added both the address display and send components to the wallet UI, providing a cohesive workflow:

```typescript
{/* Spark Address Card */}
<Card className="mb-6">
  {/* Display user's Spark address */}
</Card>

{/* Send Payment Card (Lightning) */}
<SendPaymentCard {...props} />

{/* Send Spark Payment Card */}
<SendSparkPaymentCard
  recipientSparkAddress={recipientSparkAddress}
  setRecipientSparkAddress={setRecipientSparkAddress}
  sendSparkAmount={sendSparkAmount}
  setSendSparkAmount={setSendSparkAmount}
  handleSendSparkPayment={handleSendSparkPayment}
  isSendingSparkPayment={isSendingSparkPayment}
  disabled={!sdkRef.current}
/>

{/* Transaction History Card */}
<TransactionHistoryCard transactions={transactions} />
```

### Testing

The implementation was tested with:
- TypeScript checking (no errors)
- UI rendering and state updates
- Error handling (e.g., invalid addresses, insufficient balance)
- Clipboard interaction for the Spark address

### Conclusion

This implementation adds the ability to:
1. View and copy the user's own Spark address
2. Enter a recipient's Spark address
3. Specify an amount to send
4. Process the payment with appropriate loading states and feedback

These features complete the core wallet functionality, allowing users to:
- Receive Bitcoin via Lightning Network
- Send Bitcoin via Lightning invoices
- Receive Bitcoin via Spark address
- Send Bitcoin directly to other Spark addresses
- View transaction history

All functionality is implemented with appropriate error handling, loading states, and user feedback via toast notifications.