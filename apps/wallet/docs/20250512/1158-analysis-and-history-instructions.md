Okay, I've reviewed the agent's actions and the current state of the `App.tsx`. The Spark SDK integration for initialization, balance, and invoice generation seems to be in place. Now, let's add the transaction history functionality.

**Step-by-Step Instructions for Adding Transaction History:**

**Step 1: Define Transaction Type and State in `src/App.tsx`**

1.  Open `src/App.tsx`.
2.  **Add a type definition for a Spark transfer/transaction.** Based on the previous Blitz wallet example and common Spark SDK return structures for `getTransfers`, we can define something like this near your other interface definitions:

    ```typescript
    // ... other imports
    // Add this new type/interface for Spark transactions
    interface SparkTransfer {
      id: string;
      createdAt: string; // ISO string or can be parsed to Date
      updatedAt: string; // ISO string
      network: string;
      type: string; // e.g., "TRANSFER", "PREIMAGE_SWAP", "COOPERATIVE_EXIT"
      status: string; // e.g., "COMPLETED", "PENDING"
      transferDirection: "INCOMING" | "OUTGOING"; // Or use an enum if Spark SDK provides one
      totalValue: bigint; // Assuming amount is in satoshis as bigint
      description?: string;
      fee?: bigint;
      // Add other relevant fields from Spark's getTransfers response as needed
      // For example, if it returns sender/receiver public keys, etc.
      // The Blitz example had:
      // senderIdentityPublicKey?: string;
      // receiverIdentityPublicKey?: string;
      // initial_sent?: bigint;
      // address?: string;
    }
    ```
    *Self-correction: The Blitz example `sparkContext.jsx` and `txStorage.js` suggests the Spark SDK `getTransfers` call returns objects with `id`, `createdTime` (number/string), `updatedTime`, `type`, `transferDirection`, `totalValue`, `description`, `fee`. The field names might be slightly different (e.g. `created_at_time` vs `createdAt`). Let's use the more likely field names as seen in the Blitz example's Spark SDK usage.*

    Update the type:
    ```typescript
    // At the top of App.tsx, after SparkWallet import
    import { SparkWallet, Network as SparkNetwork, type TokenInfo, type Transfer as SdkSparkTransfer } from '@buildonspark/spark-sdk';

    // ... other interfaces
    // Use the SdkSparkTransfer type if available, or define a compatible one
    // For simplicity, let's assume SdkSparkTransfer is what getTransfers items look like.
    // If not, adjust this interface to match the actual return type of sdk.getTransfers()
    interface DisplaySparkTransfer extends SdkSparkTransfer {
      // Add any additional properties your UI might need that aren't in SdkSparkTransfer
      // For example, a formatted date string, or a display icon
      formattedDate?: string;
      displayAmount?: string;
      isSent?: boolean;
    }
    ```
    *Self-correction 2: The Spark SDK exports `Transfer` type. We should use that or extend it.*
    The Blitz example used `created_at_time`, `updated_at_time`, `total_sent`, `transfer_direction`. The SDK likely returns objects with snake_case properties. Let's define a type that expects these.

    ```typescript
    // Add this interface (adjust based on actual SDK return for getTransfers)
    export interface SparkTransferData { // Renamed to avoid conflict with SDK's Transfer
      id: string;
      created_at_time: string; // Assuming ISO string, might need to be Date or number
      updated_at_time: string;
      network: string;
      type: string;
      status: string;
      transfer_direction: "INCOMING" | "OUTGOING";
      total_sent: bigint; // This was totalValue in Blitz example, Spark docs usually use total_sent for the value sent
      description?: string;
      fee?: bigint;
      sender_identity_public_key?: string;
      receiver_identity_public_key?: string;
      // Add other fields as returned by sparkWallet.getTransfers()
    }
    ```

3.  **Add state for transactions:**
    Below your other `useState` hooks in `App()`, add:
    ```typescript
    const [transactions, setTransactions] = useState<SparkTransferData[]>([]);
    ```

**Step 2: Fetch Transactions in `fetchWalletData`**

1.  Modify the `fetchWalletData` function to also fetch transactions.

    ```typescript
    const fetchWalletData = useCallback(async (sdk: any) => { // Keep 'any' for sdkRef for now
      if (!sdk) return;
      const wallet = sdk.wallet || sdk; // Keep this fallback

      try {
        // Fetch balance (already there)
        const balanceData = await wallet.getBalance();
        setWalletInfo({
          balanceSat: balanceData.balance || BigInt(0),
          tokenBalances: balanceData.tokenBalances
        });

        // Fetch transactions
        console.log("Fetching transaction history...");
        // Fetch last 10-20 transactions for now.
        // The Spark SDK getTransfers might return { transfers: SparkTransferData[] }
        const transfersResponse = await wallet.getTransfers(20, 0); // Fetch 20 transactions, offset 0
        console.log("Transaction history response:", transfersResponse);

        if (transfersResponse && transfersResponse.transfers) {
          // Sort by created_at_time descending (newest first)
          const sortedTransactions = transfersResponse.transfers.sort(
            (a: SparkTransferData, b: SparkTransferData) =>
              new Date(b.created_at_time).getTime() - new Date(a.created_at_time).getTime()
          );
          setTransactions(sortedTransactions);
        } else {
          setTransactions([]);
        }

      } catch (error) {
        console.error('Failed to fetch wallet data or transactions:', error);
        toast.error("Failed to fetch wallet data. Some features may be limited.");
        setTransactions([]); // Clear transactions on error
      }
    }, []); // Keep empty dependency array for now
    ```

**Step 3: Create Transaction Item Component (`src/components/TransactionItem.tsx`)**

1.  Create a new file `src/components/TransactionItem.tsx`.
2.  Add the following content:

    ```typescript
    import React from 'react';
    import { SparkTransferData } from '@/App'; // Adjust path if App.tsx is elsewhere
    import { ArrowUpRight, ArrowDownLeft, Clock } from 'lucide-react';
    import { Badge } from '@/components/ui/badge';
    import { cn } from '@/lib/utils';

    interface TransactionItemProps {
      transaction: SparkTransferData;
    }

    const TransactionItem: React.FC<TransactionItemProps> = ({ transaction }) => {
      const isSent = transaction.transfer_direction === "OUTGOING";
      const amountDisplay = `${isSent ? '-' : '+'} ${transaction.total_sent.toString()} sats`;

      let dateDisplay = "Date unknown";
      try {
        dateDisplay = new Date(transaction.created_at_time).toLocaleString(undefined, {
          year: 'numeric', month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit'
        });
      } catch (e) {
        console.warn("Could not parse transaction date:", transaction.created_at_time);
      }

      const getStatusBadgeVariant = (status: string): "default" | "secondary" | "destructive" | "outline" => {
        switch (status.toUpperCase()) {
          case 'COMPLETED':
            return "default"; // Default is usually primary color
          case 'PENDING':
          case 'TRANSFER_STATUS_SENDER_KEY_TWEAKED': // Example pending statuses
          case 'LIGHTNING_PAYMENT_INITIATED':
            return "secondary";
          case 'FAILED':
          case 'TRANSFER_STATUS_RETURNED':
            return "destructive";
          default:
            return "outline";
        }
      }

      const descriptionOrType = transaction.description || transaction.type.replace(/_/g, ' ').toLowerCase().replace(/\b\w/g, l => l.toUpperCase());

      return (
        <div className="flex items-center justify-between p-3 border-b border-border last:border-b-0">
          <div className="flex items-center gap-3">
            {isSent ? (
              <ArrowUpRight className="h-5 w-5 text-destructive" />
            ) : (
              <ArrowDownLeft className="h-5 w-5 text-green-500" />
            )}
            <div>
              <p className="text-sm font-medium truncate max-w-[150px] sm:max-w-xs" title={descriptionOrType}>
                {descriptionOrType}
              </p>
              <p className="text-xs text-muted-foreground">{dateDisplay}</p>
            </div>
          </div>
          <div className="text-right">
            <p className={cn(
              "text-sm font-semibold",
              isSent ? "text-destructive" : "text-green-600 dark:text-green-500"
            )}>
              {amountDisplay}
            </p>
            <Badge variant={getStatusBadgeVariant(transaction.status)} className="mt-1 text-xs">
              {transaction.status.replace(/_/g, ' ').toLowerCase().replace('transfer status ', '').replace('lightning payment ','')}
            </Badge>
          </div>
        </div>
      );
    };

    export default TransactionItem;
    ```

**Step 4: Create Transaction History Card Component (`src/components/TransactionHistoryCard.tsx`)**

1.  Create a new file `src/components/TransactionHistoryCard.tsx`.
2.  Add the following content:

    ```typescript
    import React from 'react';
    import { Card, CardContent, CardHeader, CardTitle, CardDescription } from '@/components/ui/card';
    import { ScrollArea } from '@/components/ui/scroll-area';
    import TransactionItem from './TransactionItem';
    import { SparkTransferData } from '@/App'; // Adjust path if App.tsx is elsewhere
    import { ListCollapse } from 'lucide-react';

    interface TransactionHistoryCardProps {
      transactions: SparkTransferData[];
    }

    const TransactionHistoryCard: React.FC<TransactionHistoryCardProps> = ({ transactions }) => {
      return (
        <Card className="mt-6">
          <CardHeader>
            <CardTitle>Transaction History</CardTitle>
            <CardDescription>Your recent Bitcoin transactions.</CardDescription>
          </CardHeader>
          <CardContent>
            {transactions.length > 0 ? (
              <ScrollArea className="h-[300px] w-full rounded-md border"> {/* Adjust height as needed */}
                <div className="p-1">
                  {transactions.map((tx) => (
                    <TransactionItem key={tx.id} transaction={tx} />
                  ))}
                </div>
              </ScrollArea>
            ) : (
              <div className="flex flex-col items-center justify-center h-[150px] text-center">
                <ListCollapse className="h-12 w-12 text-muted-foreground mb-2" />
                <p className="text-sm text-muted-foreground">No transactions yet.</p>
                <p className="text-xs text-muted-foreground">Your transactions will appear here once you send or receive Bitcoin.</p>
              </div>
            )}
          </CardContent>
        </Card>
      );
    };

    export default TransactionHistoryCard;
    ```

**Step 5: Integrate Transaction History Card into `src/App.tsx`**

1.  **Import the new component:**
    At the top of `src/App.tsx`, add:
    ```typescript
    import TransactionHistoryCard from './components/TransactionHistoryCard';
    ```
2.  **Render the card in the `wallet_ready` state:**
    Inside the `renderCurrentScreen` function, in the `case 'wallet_ready':` block, add the `TransactionHistoryCard` component below the "Receive Bitcoin (Lightning)" card.

    ```typescript
    // ... inside renderCurrentScreen, case 'wallet_ready':
              </CardContent>
            </Card> {/* This is the end of the Receive Bitcoin Card */}

            {/* Add Transaction History Card here */}
            <TransactionHistoryCard transactions={transactions} />

            <div className="h-16"/> {/* Spacer for scroll */}
          </div>
        );
    // ...
    ```
3.  **Export `SparkTransferData` from `App.tsx`** (if not already done when defining it, or move it to a shared types file):
    If you defined `SparkTransferData` directly in `App.tsx`, make sure to export it so `TransactionItem.tsx` and `TransactionHistoryCard.tsx` can import it:
    ```typescript
    // In App.tsx
    export interface SparkTransferData { /* ... as defined before ... */ }
    ```
    (Alternatively, create `src/types.ts` and move shared types there.)

**Step 6: Call `fetchWalletData` after successful invoice generation (Optional but good UX)**

To provide more immediate feedback after an invoice is potentially paid (though Spark events are the primary mechanism), you can call `fetchWalletData` after an invoice is generated. This is more for updating pending states if the SDK handles that internally.

1.  Modify `handleGenerateInvoice` in `App.tsx`:
    ```typescript
    // ... inside handleGenerateInvoice, in the try block, after setInvoice and toast.success
    setInvoice(encodedInvoice);
    toast.dismiss("invoice-generation");
    toast.success("Lightning Invoice Generated!");
    // Optionally, re-fetch data to see if any internal state/pending tx changed
    if (sdkRef.current) {
      fetchWalletData(sdkRef.current.wallet || sdkRef.current);
    }
    // ...
    ```

**Step 7: Ensure Spark SDK `getTransfers` and event handling (Future)**

*   For now, `fetchWalletData` fetches transactions on load. A more robust solution would involve:
    *   Listening to Spark SDK events (if available for new transactions or status updates) and calling `fetchWalletData` or a more targeted transaction update function. The Blitz example's `sparkContext.jsx` had `sparkWallet.on("transfer:claimed", ...)` and `sparkWallet.on("deposit:confirmed", ...)`. This SDK might have different event names or mechanisms.
    *   Implementing pull-to-refresh or a refresh button that calls `fetchWalletData`.

**Step 8: Update Log File**

Add the following to `docs/20250512/1128-spark-it-log.md` (or your current log file):

```markdown
### 5. Added Transaction History Display

- **Defined `SparkTransferData` Interface (`src/App.tsx`):**
  - Created an interface to represent the structure of transaction data returned by `sdkRef.current.getTransfers()`.
  - Includes fields like `id`, `created_at_time`, `type`, `status`, `transfer_direction`, `total_sent`, `description`, `fee`.
- **Added `transactions` State (`src/App.tsx`):**
  - Introduced `useState<SparkTransferData[]>([])` to hold the list of transactions.
- **Updated `fetchWalletData` (`src/App.tsx`):**
  - Modified to call `sdkRef.current.wallet.getTransfers(20, 0)` to fetch the last 20 transactions.
  - Transactions are sorted by `created_at_time` (descending) and stored in the `transactions` state.
  - Added error handling for transaction fetching.
- **Created `TransactionItem.tsx` Component (`src/components/TransactionItem.tsx`):**
  - Displays a single transaction.
  - Shows direction (send/receive icon using `lucide-react` `ArrowUpRight` / `ArrowDownLeft`).
  - Displays amount, description (or formatted type), and formatted date.
  - Includes a `Badge` to show transaction status with appropriate coloring.
- **Created `TransactionHistoryCard.tsx` Component (`src/components/TransactionHistoryCard.tsx`):**
  - Renders a `Card` titled "Transaction History".
  - Uses `ScrollArea` for the list of transactions.
  - Maps through the `transactions` prop, rendering a `TransactionItem` for each.
  - Displays a "No transactions yet" message if the list is empty, including an icon.
- **Integrated into `wallet_ready` Screen (`src/App.tsx`):**
  - Added `<TransactionHistoryCard transactions={transactions} />` below the "Receive Bitcoin (Lightning)" card.
- **(Optional UX) Updated `handleGenerateInvoice` (`src/App.tsx`):**
  - Added a call to `fetchWalletData` after successful invoice generation to potentially update any pending states visible in the transaction list, though primary updates rely on future event handling or manual refresh.
```

**Final Type Check:**
Run `pnpm run t` (or your type-checking script) to ensure all changes are type-safe.

This will give you a basic transaction history display. Further enhancements would include pagination, real-time updates via Spark SDK events (if applicable, this needs investigation for Spark SDK's specific event model), and more detailed transaction views.
