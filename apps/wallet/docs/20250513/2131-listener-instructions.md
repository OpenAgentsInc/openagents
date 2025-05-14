Okay, I've reviewed the Spark SDK documentation and the Blitz Wallet example. The key to immediate balance updates for incoming transactions is to use the event listeners provided by the `SparkWallet` EventEmitter.

Here are the instructions for the coding agent to implement real-time balance and transaction updates using Spark SDK events:

**Objective:**

Integrate Spark SDK event listeners (`transfer:claimed`, `deposit:confirmed`, and optionally stream status events) into `App.tsx`. These listeners will trigger immediate updates to the wallet balance and a refresh of the transaction history upon receiving relevant events, eliminating the need for a manual page refresh for incoming funds.

**File to Modify:** `src/App.tsx`

---

**I. Define Event Handler Callbacks**

1.  **Locate the `fetchWalletData` function.** Ensure it's correctly defined using `useCallback`.
2.  **Add `useCallback` for event handlers:**
    Below `fetchWalletData` and before the `connectToSparkSDK` function, define the following memoized callback functions. These will handle the logic when Spark events are received.

    ```typescript
    // Inside App component, after fetchWalletData

    const handleTransferClaimed = useCallback((transferId: string, updatedBalance: number) => {
      console.log(`Spark Event - transfer:claimed: ID=${transferId}, New Balance=${updatedBalance}`);
      toast.info(`Incoming payment confirmed! New balance: ₿ ${updatedBalance.toLocaleString()}`);
      // Directly update balance for immediate UI feedback
      setWalletInfo(prev => ({ ...prev, balanceSat: BigInt(updatedBalance) }));
      // Then, refresh all wallet data (including transaction list)
      if (sdkRef.current) {
        // sdkRef.current is the SparkWallet instance here
        fetchWalletData(sdkRef.current);
      }
    }, [fetchWalletData]); // Depends on fetchWalletData

    const handleDepositConfirmed = useCallback((depositId: string, updatedBalance: number) => {
      console.log(`Spark Event - deposit:confirmed: ID=${depositId}, New Balance=${updatedBalance}`);
      toast.info(`Deposit confirmed! New balance: ₿ ${updatedBalance.toLocaleString()}`);
      // Directly update balance
      setWalletInfo(prev => ({ ...prev, balanceSat: BigInt(updatedBalance) }));
      // Then, refresh all wallet data
      if (sdkRef.current) {
        fetchWalletData(sdkRef.current);
      }
    }, [fetchWalletData]); // Depends on fetchWalletData

    // Optional: Handlers for stream status events
    const handleStreamConnected = useCallback(() => {
        console.log("Spark stream connected.");
        // Dismiss any reconnecting toasts
        toast.dismiss("spark-reconnect");
        toast.success("Real-time updates connected.");
    }, []);

    const handleStreamDisconnected = useCallback((reason: string) => {
        console.warn("Spark stream disconnected:", reason);
        toast.error("Real-time updates disconnected.", { description: `Reason: ${reason}. Will attempt to reconnect.` });
    }, []);

    const handleStreamReconnecting = useCallback((attempt: number, maxAttempts: number, delayMs: number, error: string) => {
        console.log(`Spark stream reconnecting: attempt ${attempt}/${maxAttempts}, delay ${delayMs}ms, error: ${error}`);
        if (attempt === 1) {
          toast.loading(`Connection lost. Reconnecting (attempt ${attempt})...`, { id: "spark-reconnect", duration: Infinity });
        } else {
          toast.loading(`Reconnecting (attempt ${attempt})...`, { id: "spark-reconnect", duration: Infinity });
        }
    }, []);
    ```

**II. Set Up and Clean Up Event Listeners using `useEffect`**

1.  **Add a `useEffect` hook:**
    This hook will manage the lifecycle of the event listeners. It should run when the wallet becomes ready (`appState === 'wallet_ready'`) and `sdkRef.current` is populated. It will also clean up listeners when the state changes or the component unmounts.

    ```typescript
    // Inside App component, typically after state and callback definitions

    useEffect(() => {
      // Check if wallet is ready and SDK instance exists
      if (appState === 'wallet_ready' && sdkRef.current) {
        const sdkInstance = sdkRef.current; // sdkRef.current is the SparkWallet instance

        // Ensure sdkInstance has event methods (it should as it extends EventEmitter)
        if (sdkInstance && typeof sdkInstance.on === 'function' && typeof sdkInstance.off === 'function') {
          console.log("Attaching Spark event listeners...");

          // Attach listeners for balance/transaction updates
          sdkInstance.on('transfer:claimed', handleTransferClaimed);
          sdkInstance.on('deposit:confirmed', handleDepositConfirmed);

          // Optional: Attach listeners for stream status for better UX
          sdkInstance.on('stream:connected', handleStreamConnected);
          sdkInstance.on('stream:disconnected', handleStreamDisconnected);
          sdkInstance.on('stream:reconnecting', handleStreamReconnecting);

          // Cleanup function: This will be called when appState changes from 'wallet_ready',
          // or when the component unmounts.
          return () => {
            console.log("Detaching Spark event listeners...");
            sdkInstance.off('transfer:claimed', handleTransferClaimed);
            sdkInstance.off('deposit:confirmed', handleDepositConfirmed);

            // Optional: Detach stream status listeners
            sdkInstance.off('stream:connected', handleStreamConnected);
            sdkInstance.off('stream:disconnected', handleStreamDisconnected);
            sdkInstance.off('stream:reconnecting', handleStreamReconnecting);
          };
        }
      }
    }, [
      appState, // Re-run when appState changes
      handleTransferClaimed, // Stable due to useCallback
      handleDepositConfirmed, // Stable due to useCallback
      handleStreamConnected,  // Stable due to useCallback
      handleStreamDisconnected, // Stable due to useCallback
      handleStreamReconnecting  // Stable due to useCallback
      // sdkRef.current is not directly in dependency array as it's a ref.
      // The effect runs based on appState, and sdkRef.current is checked inside.
    ]);
    ```

**III. No Changes Needed for Sending Payments**

*   For payments *sent* by the user (both Spark-to-Spark via `handleSendSparkPayment` and Lightning invoice payments via `confirmAndPayInvoice`), your existing logic already calls `fetchWalletData(sdkRef.current)` after a successful payment initiation. This is sufficient for updating the balance and transaction list after a send operation initiated by the current user. The events are primarily for incoming funds or status changes that occur asynchronously or are initiated externally.

**IV. Testing**

1.  **Receive Spark Payment:**
    *   If you have two instances of the wallet or another Spark-compatible wallet, send a Spark payment to your test wallet's Spark address.
    *   Observe the console for `transfer:claimed` event logs.
    *   Verify that a toast notification appears.
    *   Verify that the balance in the UI updates immediately without a manual refresh.
    *   Verify that the new incoming transaction appears in the history.

2.  **Receive Lightning Payment:**
    *   Generate a Lightning invoice in your test wallet.
    *   Pay this invoice from another Lightning wallet.
    *   Observe the console. If Spark's architecture treats this as a "transfer claimed" internally after the Lightning part, you should see the `transfer:claimed` event.
    *   Verify toast, immediate balance update, and transaction history update.
    *   *(If `transfer:claimed` is not triggered for incoming LN payments, this specific scenario won't have an *event-driven* immediate update, and would rely on the next `fetchWalletData` call, e.g., after a user action or periodic refresh if implemented. However, the Blitz example suggests `transfer:claimed` is used more broadly).*

3.  **On-Chain Deposit (if testable):**
    *   If you can simulate or perform an L1 Bitcoin deposit to one of the wallet's deposit addresses and then claim it (or if Spark auto-claims), test the `deposit:confirmed` event.
    *   Verify toast, immediate balance update, and transaction history update.

4.  **Stream Connection Status (Optional):**
    *   Observe toasts and console logs for `stream:connected`, `stream:disconnected`, and `stream:reconnecting` if you implemented these listeners. You might be able to test disconnection by temporarily cutting off internet access.

5.  **Verify Send Operations:**
    *   Re-test sending Spark payments and Lightning invoices.
    *   Confirm that the balance and transaction history still update correctly after these *outgoing* operations (due to the existing `fetchWalletData` calls in their handlers).

**Explanation of Changes:**

*   **Event Handlers (`handleTransferClaimed`, `handleDepositConfirmed`):** These functions are triggered when the Spark SDK emits the corresponding events. They immediately update the `balanceSat` in `walletInfo` state with the `updatedBalance` provided by the event. Then, they call `fetchWalletData` to get a complete, fresh state from the SDK, ensuring the transaction list and any other wallet details are also current.
*   **`useEffect` for Listener Management:** This hook ensures that event listeners are correctly attached when the wallet is ready and `sdkRef.current` (the SparkWallet instance) is available. The cleanup function within the `useEffect` is crucial: it detaches the listeners when `appState` changes away from `wallet_ready` (e.g., on logout) or when the `App` component unmounts, preventing memory leaks and errors.
*   **`useCallback`:** Wrapping the event handlers in `useCallback` memoizes them, preventing them from being recreated on every render unless their dependencies change. This is important for the `useEffect` dependency array to work correctly and avoid unnecessary re-attachment of listeners.

By implementing these event listeners, the wallet will provide a much more dynamic and responsive user experience for incoming funds and deposit confirmations.
