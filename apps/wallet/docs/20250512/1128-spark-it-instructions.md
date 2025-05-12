Okay, I will provide specific coding instructions for an agent to integrate the Spark SDK into the `OpenAgents Wallet` codebase. The primary goal is to replace the existing Breez SDK functionality for wallet initialization, balance fetching, and Lightning invoice generation with Spark SDK equivalents. The Breez SDK code should be commented out for now, not entirely removed, to facilitate potential future reference or a phased migration.

**Assumptions:**

*   The codebase is based on the files provided, particularly the `App.tsx` structure from `docs/20250512/0846-screens-instructions.md` which uses Breez SDK.
*   The agent has access to all provided markdown files and the project structure.
*   The Spark SDK (`@buildonspark/spark-sdk`) is already listed as a dependency in `package.json` (version `^0.1.14` or similar, as per `1005-spark.md`'s reference). If not, it should be added.

**Step-by-Step Instructions for the Agent:**

**Step 1: Update `package.json`**

1.  Open `package.json`.
2.  Locate the line for `@breeztech/breez-sdk-liquid`.
3.  **Comment out** this line. For example:
    ```json
    // "@breeztech/breez-sdk-liquid": "^0.8.0",
    ```
4.  Ensure `@buildonspark/spark-sdk` is present and up-to-date (e.g., `"@buildonspark/spark-sdk": "^0.1.14"`).
5.  Save the file.
6.  Run your package manager's install command (e.g., `yarn install` or `pnpm install`) to update dependencies. This step is crucial because the Breez SDK might have WASM components that could interfere if not effectively removed from the build process. Commenting it out in `package.json` and reinstalling should prevent it from being bundled.

**Step 2: Update `vite.config.ts`**

1.  Open `vite.config.ts`.
2.  The Spark SDK is a standard JavaScript/TypeScript library and typically doesn't require special WASM or top-level await plugins.
3.  **Comment out** or **remove** the `wasm()` and `topLevelAwait()` plugins from the `plugins` array.
4.  **Comment out** or **remove** the `optimizeDeps` section if it was specifically excluding Breez SDK components.

    Your `vite.config.ts` should look similar to this:
    ```typescript
    import { defineConfig, PluginOption } from 'vite'
    import react from '@vitejs/plugin-react'
    import tailwindcss from '@tailwindcss/vite'
    import path from 'path'
    // import wasm from "vite-plugin-wasm"; // Commented out
    // import topLevelAwait from "vite-plugin-top-level-await"; // Commented out

    export default defineConfig({
      plugins: [
        react(),
        tailwindcss(),
        // wasm(), // Commented out
        // topLevelAwait() // Commented out
      ] as PluginOption[],
      // optimizeDeps: { // Commented out if it existed for Breez
      //   exclude: ['@breeztech/breez-sdk-liquid']
      // },
      build: {
        target: 'esnext',
      },
      resolve: {
        alias: {
          "@": path.resolve(__dirname, "./src"),
        },
      },
    })
    ```

**Step 3: Modify `src/App.tsx` (Main Integration Work)**

You will be modifying the `App.tsx` file that currently uses the Breez SDK (as provided in `docs/20250512/0846-screens-instructions.md`). The goal is to adapt it to use the Spark SDK, following the patterns shown in the `App.tsx` example within `docs/20250512/1005-spark.md`.

1.  **Update Imports:**
    *   Locate Breez SDK imports:
        ```typescript
        // import init, { defaultConfig, connect, BindingLiquidSdk, type WalletInfo as SdkWalletInfo, type LightningPaymentLimitsResponse } from '@breeztech/breez-sdk-liquid'
        // For web, ensure you are importing from '@breeztech/breez-sdk-liquid/web' if that's the correct path for the web build
        import init, { defaultConfig, connect, BindingLiquidSdk, SdkEvent, type WalletInfo as SdkWalletInfo, type LightningPaymentLimitsResponse } from '@breeztech/breez-sdk-liquid' // This line is from the provided Breez App.tsx
        ```
    *   **Comment out** these Breez SDK imports.
    *   **Add** Spark SDK imports:
        ```typescript
        import { SparkWallet, type Network as SparkNetwork, type TokenInfo } from '@buildonspark/spark-sdk';
        ```

2.  **Update State Definitions and Types:**
    *   Find `sdkRef`:
        ```typescript
        // const sdkRef = useRef<BindingLiquidSdk | null>(null);
        ```
    *   **Comment out** the Breez `sdkRef` type.
    *   **Add** the Spark `sdkRef` type:
        ```typescript
        const sdkRef = useRef<SparkWallet | null>(null);
        ```
    *   Find `WalletInfo` (or `WalletDisplayInfo` in your target file):
        ```typescript
        // interface WalletInfo {
        //  balanceSat: bigint;
        //  pendingSendSat: bigint;
        //  pendingReceiveSat: bigint;
        // }
        ```
    *   **Comment out** the Breez `WalletInfo` interface.
    *   **Add** the Spark `WalletDisplayInfo` interface:
        ```typescript
        interface WalletDisplayInfo {
          balanceSat: bigint;
          tokenBalances?: Map<string, { balance: bigint, tokenInfo: TokenInfo }>;
        }
        ```
    *   **Comment out** or remove `LightningLimits` (or `LightningLimitsInfo`) interface and its corresponding state (`lightningLimits`, `setLightningLimits`), as Spark's invoice creation is direct and doesn't provide these pre-calculated receiving limits.
        ```typescript
        // interface LightningLimits {
        //   min: bigint;
        //   max: bigint;
        // }
        // ...
        // const [lightningLimits, setLightningLimits] = useState<LightningLimits>({
        //   min: BigInt(0),
        //   max: BigInt(0)
        // });
        ```
    *   **Comment out** or remove `listenerIdRef` if it was specific to Breez event handling:
        ```typescript
        // const listenerIdRef = useRef<string | null>(null);
        ```
    *   Adjust the initial state for `walletInfo` to match `WalletDisplayInfo`:
        ```typescript
        // Initial state for walletInfo (Breez version)
        // const [walletInfo, setWalletInfo] = useState<WalletInfo>({
        //   balanceSat: BigInt(0),
        //   pendingSendSat: BigInt(0),
        //   pendingReceiveSat: BigInt(0)
        // });

        // New initial state for walletInfo (Spark version)
        const [walletInfo, setWalletInfo] = useState<WalletDisplayInfo>({
          balanceSat: BigInt(0),
        });
        ```
    *   The `receiveAmount` state can be renamed to `receiveAmountSats` for clarity if desired, but ensure consistency.
    *   `fees` (or `calculatedFees`) state related to Breez's `prepareReceivePayment` can be commented out or removed as Spark's invoice generation is direct.

3.  **Update `connectToBreezSDK` to `connectToSparkSDK`:**
    *   Locate the `connectToBreez` (or `connectToBreezSDK`) function.
    *   **Comment out** the entire Breez connection logic, including the `init()` call for WASM, `defaultConfig`, `connect`, and `addEventListener`.
    *   **Implement** the `connectToSparkSDK` function:
        ```typescript
        const connectToSparkSDK = useCallback(async (mnemonic: string) => {
          if (sdkRef.current) {
            console.log("Spark SDK connection attempt skipped: already initialized.");
            return;
          }
          setFlowState('initializing_wallet'); // Ensure setFlowState is from Zustand or local state
          setAppErrorMessage(null); // Ensure setAppErrorMessage is from Zustand or local state

          try {
            const { wallet: sparkInstance } = await SparkWallet.create({
              mnemonicOrSeed: mnemonic,
              options: {
                network: "MAINNET" as SparkNetwork, // Or SparkNetwork.MAINNET if available
              },
            });

            sdkRef.current = sparkInstance;
            localStorage.setItem('openAgentsWalletMnemonic', mnemonic); // Keep using your localStorage key for mnemonic

            await fetchWalletData(sparkInstance); // Ensure fetchWalletData is updated for Spark
            setFlowState('wallet_ready');
            toast.success("Spark Wallet Connected!");
          } catch (error) {
            console.error('Failed to initialize Spark SDK:', error);
            const message = error instanceof Error ? error.message : String(error);
            setAppErrorMessage(`Spark Wallet initialization failed: ${message}`);
            setFlowState('error_state');
            sdkRef.current = null;
          }
        }, [fetchWalletData, setFlowState, setAppErrorMessage]); // Add dependencies from your state management
        ```
    *   Ensure the `useEffect` that calls this connection logic on mount is updated to call `connectToSparkSDK`.

4.  **Update `fetchWalletData`:**
    *   Locate the `fetchWalletData` function.
    *   **Comment out** the Breez `sdk.getInfo()` and `sdk.fetchLightningLimits()` logic.
    *   **Implement** the Spark logic:
        ```typescript
        const fetchWalletData = useCallback(async (sdk: SparkWallet) => { // Parameter type changed
          if (!sdk) return;
          try {
            const balanceData = await sdk.getBalance();
            setWalletInfo({
              balanceSat: balanceData.balance,
              tokenBalances: balanceData.tokenBalances, // Optional: can be displayed later
            });
            // No direct equivalent for fetching lightning receive limits for Spark here
          } catch (error) {
            console.error('Failed to fetch Spark wallet data:', error);
            toast.error("Error fetching wallet data.");
          }
        }, []); // Empty dependency array is fine if sdk is always passed as a param
        ```

5.  **Update `handleLogout`:**
    *   Locate `handleLogout`.
    *   **Comment out** Breez-specific cleanup like `sdkRef.current.removeEventListener` and `sdkRef.current.disconnect()`.
    *   The main action for Spark will be:
        ```typescript
        sdkRef.current = null;
        // Keep localStorage.removeItem('openAgentsWalletMnemonic');
        // Keep resetting other UI/Zustand state (mnemonic, walletInfo, invoice, appState).
        ```

6.  **Update `generateInvoice` (or `handleGenerateInvoice`):**
    *   Locate `generateInvoice`.
    *   **Comment out** the Breez logic involving `prepareReceivePayment` and `receivePayment`.
    *   **Implement** the Spark logic:
        ```typescript
        const handleGenerateInvoice = async () => {
          if (!sdkRef.current) {
            toast.error("Wallet not initialized.");
            return;
          }
          if (isGeneratingInvoice) return; // Assuming you have this state
          setIsGeneratingInvoice(true); // Assuming you have this state

          try {
            toast.loading("Generating Spark invoice...", { id: "spark-invoice-gen" });
            const amountNumber = Number(receiveAmountSats); // receiveAmountSats from your state
            if (isNaN(amountNumber) || amountNumber <= 0) {
              toast.error("Invalid amount for invoice.");
              setIsGeneratingInvoice(false);
              return;
            }

            const invoiceString = await sdkRef.current.createLightningInvoice({
              amountSats: amountNumber,
              memo: "OpenAgents Invoice" // Or get memo from user input
            });

            setGeneratedInvoice(invoiceString);
            // setCalculatedFees(BigInt(0)); // Fees are part of LN invoice with Spark
            toast.dismiss("spark-invoice-gen");
            toast.success("Spark Lightning Invoice Generated!");
          } catch (error) {
            console.error('Failed to generate Spark invoice:', error);
            toast.dismiss("spark-invoice-gen");
            const message = error instanceof Error ? error.message : String(error);
            toast.error("Failed to generate Spark invoice", { description: message });
          } finally {
            setIsGeneratingInvoice(false);
          }
        };
        ```

7.  **UI Adjustments in `wallet_ready` screen:**
    *   Change titles like "Breez Wallet Balance" to "Spark Wallet Balance".
    *   Remove any UI displaying `lightningLimits.min` or `lightningLimits.max` for receiving, as this isn't fetched upfront with Spark.
    *   Remove display of pre-calculated `fees` (or `calculatedFees`) before invoice generation.
    *   The `generateInvoice` button should now call the Spark-updated `handleGenerateInvoice`.
    *   The QR code component (`QRCode` from `react-qr-code`) will now display the `generatedInvoice` string from Spark.

8.  **Remove/Comment Breez-specific Environment Variable Usage:**
    *   Search for `import.meta.env.VITE_BREEZ_API_KEY` in `App.tsx` and comment out its usage as it's not needed for Spark client-side wallet creation.

**Step 4: Environment Variables**

1.  Open your `.env` file.
2.  You can **comment out** or **remove** the `VITE_BREEZ_API_KEY` line as it's specific to Breez and not used for basic Spark client wallet operations.
    ```
    # VITE_BREEZ_API_KEY=your_breez_api_key_here
    ```

**Step 5: Testing the Implementation**

1.  **Clean Build:** If you encounter issues, try deleting `node_modules/.vite` cache directory and `dist` directory, then restart the dev server.
2.  **Test Wallet Creation:**
    *   Go through the "Create New Wallet" flow.
    *   Ensure a mnemonic is generated and displayed.
    *   Verify that `connectToSparkSDK` is called and completes successfully (check console for "Spark Wallet Connected!" and any errors).
    *   Check if the balance is fetched and displayed (it will likely be 0 for a new wallet).
3.  **Test Invoice Generation:**
    *   In the `wallet_ready` screen, enter an amount.
    *   Click "Generate Spark Invoice".
    *   Verify that a BOLT11 Lightning invoice string is generated and displayed (and shown as a QR code).
4.  **Test Wallet Restoration:**
    *   Log out.
    *   Go through the "Enter Seed Phrase" flow.
    *   Enter the mnemonic of a wallet known to have a balance or activity on Spark (if possible for testing, otherwise use the newly created one).
    *   Verify successful connection and balance display.
5.  **Test Logout:**
    *   Ensure logout clears the SDK instance and relevant state, returning to the login screen.

**Log of Changes (for `docs/20250512/1005-log.md` or a new file):**

```markdown
# Implementation Log: Spark SDK Integration - Phase 1 (Init, Balance, Receive Invoice)

## Overview
Replaced core Breez SDK functionality related to wallet initialization, balance fetching, and Lightning invoice generation with Spark SDK equivalents. Breez SDK code within `App.tsx` has been commented out to preserve it for reference. The existing multi-screen UI flow and BIP39 mnemonic handling remain.

## Key Changes:

### 1. Dependencies (`package.json`):
- Commented out `@breeztech/breez-sdk-liquid`.
- Ensured `@buildonspark/spark-sdk` is the active Bitcoin/Lightning SDK.
- Ran package manager install to reflect changes.

### 2. Vite Configuration (`vite.config.ts`):
- Commented out `vite-plugin-wasm` and `vite-plugin-top-level-await` as they are presumed unnecessary for the Spark SDK.
- Commented out `optimizeDeps.exclude` for Breez SDK.

### 3. Main Application Logic (`src/App.tsx`):
- **Imports:** Replaced Breez SDK imports with `SparkWallet` and necessary types from `@buildonspark/spark-sdk`.
- **State Management:**
    - `sdkRef` type updated to `SparkWallet | null`.
    - `WalletDisplayInfo` interface adapted for `sparkWallet.getBalance()` (primarily `balanceSat`, `tokenBalances` noted for future use).
    - Removed state and UI elements related to Breez's `lightningLimits` for receiving and pre-calculated invoice fees, as Spark's `createLightningInvoice` is direct.
    - Breez-specific `eventListenerIdRef` commented out.
- **`connectToSparkSDK` function (replaces `connectToBreezSDK`):**
    - Commented out original Breez connection logic.
    - Implemented initialization using `SparkWallet.create({ mnemonicOrSeed, options: { network: "MAINNET" } })`.
    - Mnemonic persistence in `localStorage` (via Zustand store) is maintained.
    - Removed Breez SDK's global `init()` call (for WASM) and `addEventListener` logic. Event handling for Spark will be addressed in a future phase if required for real-time updates beyond re-fetching after actions.
- **`fetchWalletData` function:**
    - Commented out Breez `getInfo()` logic.
    - Implemented logic to call `sdkRef.current.getBalance()` and update `walletInfo` state.
- **`handleLogout` function:**
    - Commented out Breez-specific `removeEventListener` and `disconnect()`.
    - Spark logout now primarily clears `sdkRef.current` and local/Zustand state.
- **`handleGenerateInvoice` function (replaces Breez `generateInvoice`):**
    - Commented out Breez `prepareReceivePayment` and `receivePayment` logic.
    - Implemented direct invoice generation using `sdkRef.current.createLightningInvoice({ amountSats: number, memo: string })`.
    - Handles conversion of `BigInt` amount to `number` for the SDK.
- **UI Updates (`wallet_ready` screen):**
    - Updated titles to "OpenAgents Spark Wallet".
    - Balance display now uses data from Spark SDK.
    - Invoice generation UI adapted for direct Spark invoice creation (no pre-calculated fees or explicit min/max receive limits from SDK displayed).

### 4. Environment Variables (`.env`):
- Noted that `VITE_BREEZ_API_KEY` is no longer directly used for Spark client-side wallet initialization.

## Next Steps:
- Implement sending payments using Spark SDK.
- Implement fetching and displaying transaction history using Spark SDK.
- Thoroughly test all integrated Spark functionalities.
- Plan for removal of commented-out Breez SDK code once Spark integration is stable and complete.
- Investigate Spark SDK event mechanisms for real-time UI updates if needed.
```

This comprehensive set of instructions should allow the agent to perform the initial SDK swap. The "thats it for now" implies that sending transactions and displaying a transaction list using Spark SDK will be covered in a subsequent request.
