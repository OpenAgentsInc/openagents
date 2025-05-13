You're experiencing a common issue where the UI doesn't update immediately after an on-chain or off-chain action that affects the balance. This is usually due to how and when the balance information is re-fetched and how the application reacts to SDK events.

Let's look at your `App.tsx` and specifically how `fetchWalletData` is called within the event listener.

**Analysis of `App.tsx` regarding balance updates:**

1.  **Event Listener:**
    ```typescript
    // In connectToBreezSDK
    const eventListener = {
      onEvent: (event: SdkEvent) => { // Type SdkEvent should be imported from breez-sdk-liquid
        console.log('Breez SDK Event:', event.type, event);
        if (event.type === 'synced' ||
            event.type === 'paymentSucceeded' ||
            event.type === 'paymentFailed' ||
            event.type === 'paymentWaitingConfirmation' // Added more relevant events
          ) {
          fetchWalletData(sdk); // fetchWalletData needs the current sdk instance
        }
      }
    };
    const listenerId = await sdk.addEventListener(eventListener);
    eventListenerIdRef.current = listenerId;
    ```
    *   You are calling `fetchWalletData(sdk)` inside the event listener. This is good.
    *   The list of events that trigger `fetchWalletData` includes `paymentSucceeded` and `paymentWaitingConfirmation`. These are the key events that should reflect a change in balance or pending balance.

2.  **`fetchWalletData` Function:**
    ```typescript
    const fetchWalletData = useCallback(async (sdk: BindingLiquidSdk) => {
      if (!sdk) return;
      try {
        const info: SdkWalletInfo = await sdk.getInfo();
        setWalletInfo({
          balanceSat: info.walletInfo.balanceSat,
          pendingSendSat: info.walletInfo.pendingSendSat,
          pendingReceiveSat: info.walletInfo.pendingReceiveSat
        });
        // ... (lightningLimits fetching)
      } catch (error) {
        // ... error handling
      }
    }, []); // Empty dependency array here!
    ```
    *   **Potential Issue:** The `fetchWalletData` function is defined with `useCallback` and an **empty dependency array `[]`**. This means `fetchWalletData` is memoized and will always use the version of itself created during the initial render. Inside this initial version, any closures (like references to `sdkRef` if it were used directly, or other state/props) would be stale.
    *   However, you are passing the `sdk` instance *directly* as an argument to `fetchWalletData` from the event listener: `fetchWalletData(sdk)`. The `sdk` variable inside the event listener's scope *should* be the correct, up-to-date SDK instance that was active when the listener was attached. This part seems correct.

**Possible Reasons for Pending State:**

1.  **SDK Event Not Firing or Delayed:**
    *   The Breez SDK might not be emitting the `paymentSucceeded` or `paymentWaitingConfirmation` event immediately or as expected.
    *   There could be a delay in the Liquid network or the swap service that Breez SDK uses, meaning the actual confirmation of funds (which would trigger the event and update the `pendingReceiveSat` to `balanceSat`) takes time. One hour is quite long for Lightning, but for on-chain components of a swap (like if the payment involved a submarine swap to or from L-BTC), network congestion could play a role.

2.  **State Update Not Re-rendering:**
    *   Although `fetchWalletData` calls `setWalletInfo`, ensure that the component is actually re-rendering with the new `walletInfo`. React's batching or other optimizations could sometimes be tricky, but usually, a `setState` triggers a re-render. This is less likely the primary issue if `console.log` within `fetchWalletData` shows new values.

3.  **Incorrect Interpretation of "Pending":**
    *   The `pendingReceiveSat` field from `sdk.getInfo()` is what the SDK itself reports. If the SDK still considers the funds as pending, the UI will reflect that. This means the underlying conditions for the SDK to move funds from "pending receive" to "balance" haven't been met yet (e.g., sufficient confirmations for an on-chain part of a swap).
    *   **For Lightning payments (invoice paid):**
        *   If it was a direct L-BTC to L-BTC payment (e.g., using Magic Routing Hints if both wallets support it), it should be very fast.
        *   If it involved a submarine swap (LN -> L-BTC for receiving), the swap provider (Boltz in Breez's case) needs to see the LN payment and then make the L-BTC payment to your wallet. There could be delays at the swap provider. The `paymentPending` event should fire when the swapper broadcasts the L-BTC lockup, and `paymentSucceeded` when your SDK claims it and it's confirmed.

4.  **Initial Sync vs. Ongoing Sync:**
    *   The `synced` event usually fires after initial connection and synchronization. Ensure that ongoing operations also trigger relevant events that update the balance. Your current event listener seems to cover the payment-specific events.

5.  **Stale SDK Instance (Less Likely with Current Code):**
    *   If `fetchWalletData` were *not* receiving the `sdk` instance as a parameter and instead tried to use `sdkRef.current` from its own closure (due to an empty `useCallback` dependency array), then `sdkRef.current` inside that stale closure *could* be null or an older instance. However, your event listener correctly passes the `sdk` instance that was active when `addEventListener` was called.

**Debugging Steps:**

1.  **Intense Logging in `onEvent`:**
    Add more detailed logging *inside* the `onEvent` callback in `App.tsx`:
    ```typescript
    const eventListener = {
      onEvent: async (event: SdkEvent) => { // Make it async if you await inside
        console.log('Breez SDK Event RECEIVED:', event.type, JSON.stringify(event, null, 2));

        // Log wallet state BEFORE fetching new data
        if (sdkRef.current) {
            try {
                const infoBefore = await sdkRef.current.getInfo();
                console.log('Wallet Info BEFORE fetch on event:', event.type, infoBefore.walletInfo);
            } catch (e) {
                console.error("Error getting info BEFORE fetch on event", e);
            }
        }

        if (event.type === 'synced' ||
            event.type === 'paymentSucceeded' ||
            event.type === 'paymentFailed' ||
            event.type === 'paymentWaitingConfirmation' ||
            event.type === 'paymentPending' // Add paymentPending to see intermediate states
          ) {
          if (sdkRef.current) { // Ensure sdkRef.current is used if sdk isn't passed
            console.log(`Event ${event.type} triggered fetchWalletData.`);
            await fetchWalletData(sdkRef.current); // Pass sdkRef.current

            // Log wallet state AFTER fetching new data
            try {
                const infoAfter = await sdkRef.current.getInfo();
                console.log('Wallet Info AFTER fetch on event:', event.type, infoAfter.walletInfo);
            } catch (e) {
                console.error("Error getting info AFTER fetch on event", e);
            }

          } else {
            console.warn(`Event ${event.type} occurred but SDK ref was null.`);
          }
        }
      }
    };
    // When adding the listener:
    // const listenerId = await sdk.addEventListener(eventListener);
    ```
    Observe the console:
    *   Are `paymentSucceeded` or `paymentWaitingConfirmation` events firing for this transaction?
    *   What is the `event.details` or payload of these events?
    *   Does the `walletInfo` change after `fetchWalletData` is called in response to these events?

2.  **Check `fetchWalletData` Execution:**
    Inside `fetchWalletData`:
    ```typescript
    const fetchWalletData = useCallback(async (sdk: BindingLiquidSdk) => {
      if (!sdk) {
        console.warn("fetchWalletData called but SDK is null.");
        return;
      }
      console.log("Attempting to fetch wallet data...");
      try {
        const info: SdkWalletInfo = await sdk.getInfo();
        console.log("Fetched Wallet Info in fetchWalletData:", JSON.stringify(info.walletInfo, (key, value) =>
            typeof value === 'bigint' ? value.toString() : value, 2)
        );
        // ... rest of the function
      } // ...
    }, []); // Keep empty for now as sdk is passed as param
    ```

3.  **Examine Network Requests (if possible):**
    Use your browser's developer tools (Network tab) to see if the Breez SDK (WASM) is making any outgoing requests when events are supposed to occur or when `getInfo` is called. This can give clues about communication with Breez servers or swap services.

4.  **Breez SDK Documentation on Payment States:**
    Review the "Event Flows" section in the Breez SDK documentation (you provided a condensed version) for "Receiving Payments" -> "Lightning". This describes the sequence of events:
    *   `PaymentPending`: Swap service has lockup tx.
    *   `PaymentWaitingConfirmation`: Claim tx broadcast / MRH seen.
    *   `PaymentSucceeded`: Claim tx or MRH confirmed.
    If your payment is stuck in `PaymentPending`, it means the SDK is waiting for the claim transaction to be broadcast/confirmed or for the swap service to fully process its side.

5.  **Refresh Button:**
    Add a manual refresh button in your UI that explicitly calls `fetchWalletData(sdkRef.current)`. This helps isolate whether the issue is with event-driven updates or the data fetching itself.
    ```jsx
    // In wallet_ready state in App.tsx
    <UiButton onClick={() => {
        if (sdkRef.current) {
            toast.info("Manually refreshing wallet data...");
            fetchWalletData(sdkRef.current);
        }
    }}>Manual Refresh</UiButton>
    ```

6.  **Consider `paymentPending` Event:**
    The `paymentPending` event might provide more granular updates about the swap process before it fully succeeds. Your current listener includes it. See what details this event provides.

7.  **`useCallback` Dependency Array for `fetchWalletData`:**
    While passing `sdk` as a parameter to `fetchWalletData` from the event listener bypasses the stale closure issue for *that specific call path*, if `fetchWalletData` were called from *other places* in `App.tsx` (e.g., from a `useEffect` that depends on other state, or a button click handler that doesn't have access to the `sdk` instance from the event listener's scope), then the empty dependency array for `fetchWalletData` *could* be an issue.
    If `fetchWalletData` were to always use `sdkRef.current` internally, then it would need `sdkRef` (or rather, `sdkRef.current`'s presence) in its dependency array, or you'd need a way to ensure it always gets the *latest* `sdkRef.current`.
    However, given the current structure where the event listener passes the `sdk` instance:
    ```typescript
     const eventListener = {
        onEvent: (event: SdkEvent) => {
          // ...
          if (sdkInstanceFromConnect) { // sdkInstanceFromConnect is the `sdk` from the connectToBreezSDK scope
             fetchWalletData(sdkInstanceFromConnect);
          }
        }
      };
    ```
    This specific path *should* work. The main concern would be if `fetchWalletData` is called from elsewhere without the correct SDK instance.

**Most Likely Scenario:**

The most common reason for funds remaining "pending" for an extended period, especially after an hour for a Lightning payment, is an issue or delay in the underlying swap process (if it's not a direct L-BTC payment).
*   The swap service (e.g., Boltz) might be slow.
*   If the payment involved an on-chain Bitcoin transaction as part of the swap (e.g., if the payer used an on-chain source to fund their part of the swap with the LSP/swap provider), that part could be slow due to Bitcoin network congestion and low fees.
*   Your SDK is correctly reflecting the state as reported by the swap service and the blockchain confirmations it's aware of.

The event listener setup seems largely correct for reacting to SDK events. The key is to ensure:
a. The SDK itself is receiving updates from its backend/peers.
b. The events are indeed firing.
c. `fetchWalletData` is correctly updating the React state upon these events.

Start with aggressive logging in the `onEvent` handler and `fetchWalletData` to trace the flow and see what information the SDK provides. If no relevant events are firing after the payment, then the issue lies deeper within the SDK's state or its communication with backend services.
