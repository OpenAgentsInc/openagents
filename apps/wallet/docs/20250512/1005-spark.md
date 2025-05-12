Okay, I will provide instructions for a coding agent to replace the Breez SDK integration with the Spark SDK, focusing on the wallet initialization, balance fetching, and invoice generation aspects, while retaining the existing BIP39 mnemonic flow.

**I. Update Dependencies**

1.  **Remove Breez SDK and Add Spark SDK:**
    Open your `package.json` file.
    *   Remove the line for `@breeztech/breez-sdk-liquid`.
    *   Add the Spark SDK dependency:
        ```json
        "@buildonspark/spark-sdk": "^<latest_version>",
        ```
        (Replace `<latest_version>` with the actual latest stable version of the Spark SDK from npm).
    *   After saving `package.json`, run `pnpm install` (or `yarn install` / `npm install` depending on your project's package manager) to update your `node_modules` and lock file.

**II. Modify `src/App.tsx` for Spark SDK Integration**

The `App.tsx` component will require significant changes to adapt to the Spark SDK's API and initialization process.

1.  **Update Imports:**
    *   Remove imports from `@breeztech/breez-sdk-liquid`.
    *   Add imports from `@buildonspark/spark-sdk`.

2.  **Update State and Types:**
    *   The `sdkRef` will now hold a `SparkWallet` instance.
    *   `WalletDisplayInfo` might need adjustment based on what `sparkWallet.getBalance()` returns (it returns `balance: bigint` and `tokenBalances`). For simplicity, we'll focus on the main `balance`.
    *   `LightningLimitsInfo` might not be directly available from Spark SDK in the same way. Invoice generation in Spark takes `amountSats` and `memo`. We'll remove the limits display for now unless a direct equivalent is found or needed.
    *   Remove `eventListenerIdRef` as Spark SDK event handling might differ or might not be immediately used in this phase. (The provided Spark docs don't detail an `addEventListener` equivalent directly on the `SparkWallet` instance in the same way as Breez).

3.  **Update `connectToBreezSDK` to `connectToSparkSDK`:**
    *   This function will now use `SparkWallet.create()` for initialization.
    *   The `SparkWallet.create()` method takes `mnemonicOrSeed`.
    *   The `options` object for Spark will need to be configured, especially `network`. The example uses "REGTEST", but the instructions ask for "MAINNET".
    *   The `SparkWallet.create()` returns `{ wallet, mnemonic }`. We'll use the returned `wallet`.
    *   Error handling should be adapted for Spark's potential errors.
    *   Remove Breez-specific event listener setup. If Spark has a different event system, it would be integrated here later if needed.

4.  **Update `fetchWalletData`:**
    *   This function will now call `sdkRef.current.getBalance()`.
    *   The balance is returned as `balance: bigint`.
    *   Remove fetching of lightning limits as its Spark equivalent is not immediately obvious from the provided docs for *receiving* limits.

5.  **Update `handleLogout`:**
    *   The `disconnect` method on `sdkRef.current` might not exist for SparkWallet (the docs don't show one). If not, logout will primarily involve clearing local state and `localStorage`. Check Spark SDK for a specific cleanup/disconnect method. If none, `sdkRef.current = null;` will be the main action.

6.  **Update `handleGenerateInvoice`:**
    *   This will now call `sdkRef.current.createLightningInvoice({ amountSats: number, memo: string })`.
    *   `amountSats` should be a `number` for Spark, so convert the `BigInt` `receiveAmount`.
    *   The `memo` field can be added.
    *   The returned invoice is a string.
    *   Remove the `prepareReceivePayment` step as Spark's `createLightningInvoice` is direct. `calculatedFees` might not be available before invoice creation with Spark in the same way, so we'll remove its display for this step.

7.  **Remove Breez Specific Logic:**
    *   Any remaining Breez-specific calls or types should be removed or replaced.
    *   The `init()` call from Breez (for WASM) is not needed for Spark SDK if it's a standard JS/TS library. The Spark docs don't mention a similar global init.

Here's the modified `src/App.tsx`:

```typescript
import { useEffect, useState, useRef, useCallback } from 'react';
// Spark SDK Imports
import { SparkWallet, type Network as SparkNetwork, type TokenInfo } from '@buildonspark/spark-sdk';
import * as bip39 from '@scure/bip39';
import { wordlist } from '@scure/bip39/wordlists/english';

// Shadcn UI - Main App components
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from '@/components/ui/card';
import { Input as UiInput } from '@/components/ui/input';
import { Button as UiButton } from '@/components/ui/button';
// import { Badge } from '@/components/ui/badge'; // Badge might not be used if tokenBalances aren't displayed initially
import { ScrollArea } from '@/components/ui/scroll-area';
import { Toaster } from '@/components/ui/sonner';
import { toast } from 'sonner';
import { ModeToggle } from '@/components/mode-toggle';
import { Loader2 } from 'lucide-react';

// Screen Components
import LoginScreen from './components/LoginScreen';
import CreateWalletDisclaimerScreen from './components/CreateWalletDisclaimerScreen';
import ShowMnemonicScreen from './components/ShowMnemonicScreen';
import EnterSeedScreen from './components/EnterSeedScreen';

type WalletFlowState = 'login' | 'creating_disclaimer' | 'showing_mnemonic' | 'entering_seed' | 'initializing_wallet' | 'wallet_ready' | 'error_state';

interface WalletDisplayInfo {
  balanceSat: bigint;
  // Spark's getBalance also returns tokenBalances, which can be added later
  tokenBalances?: Map<string, { balance: bigint, tokenInfo: TokenInfo }>;
}

// Lightning limits are handled differently in Spark; invoice creation is direct.
// interface LightningLimitsInfo {
//   min: bigint;
//   max: bigint;
// }

function App() {
  const [flowState, setFlowState] = useState<WalletFlowState>('login');
  const [currentMnemonic, setCurrentMnemonic] = useState<string | null>(null);
  const [appErrorMessage, setAppErrorMessage] = useState<string | null>(null);

  const [walletInfo, setWalletInfo] = useState<WalletDisplayInfo>({
    balanceSat: BigInt(0),
  });
  // const [lightningLimits, setLightningLimits] = useState<LightningLimitsInfo>({ // Spark doesn't provide these upfront for receiving
  //   min: BigInt(0),
  //   max: BigInt(0)
  // });
  const [receiveAmountSats, setReceiveAmountSats] = useState(BigInt(10000)); // Default 10k sats
  const [generatedInvoice, setGeneratedInvoice] = useState('');
  // Fees for invoice generation are not typically pre-calculated with Spark in the same way as Breez prepare step
  // const [calculatedFees, setCalculatedFees] = useState(BigInt(0));

  const sdkRef = useRef<SparkWallet | null>(null);
  // Spark SDK event handling is different, no direct addEventListener on wallet instance from provided docs.
  // const eventListenerIdRef = useRef<string | null>(null);


  const fetchWalletData = useCallback(async (sdk: SparkWallet) => {
    if (!sdk) return;
    try {
      const balanceData = await sdk.getBalance();
      setWalletInfo({
        balanceSat: balanceData.balance,
        tokenBalances: balanceData.tokenBalances,
      });
      // Lightning limits for receiving are not fetched this way in Spark
    } catch (error) {
      console.error('Failed to fetch Spark wallet data:', error);
      toast.error("Error fetching wallet data.");
    }
  }, []);

  const connectToSparkSDK = useCallback(async (mnemonic: string) => {
    if (sdkRef.current) {
      console.log("Spark SDK connection attempt skipped: already initialized.");
      return;
    }
    setFlowState('initializing_wallet');
    setAppErrorMessage(null);

    try {
      // Spark SDK doesn't require a global init() like Breez WASM
      const { wallet: sparkInstance, mnemonic: generatedMnemonic } = await SparkWallet.create({
        mnemonicOrSeed: mnemonic, // Pass the provided or generated mnemonic
        options: {
          // Per instructions, use MAINNET. Ensure SparkNetwork enum is correctly mapped or used.
          // The example showed "REGTEST" as string, docs show enum.
          // Let's assume string is accepted for now, or use SparkNetwork.MAINNET if available.
          network: "MAINNET",
        },
        // lrc20WalletApiConfig: {} // Optional, add if needed
      });

      sdkRef.current = sparkInstance;

      // If a new mnemonic was generated by Spark (e.g., if null was passed), update state.
      // In our flow, `mnemonic` is always provided from our bip39 generation or user input.
      if (generatedMnemonic && !currentMnemonic) {
        setCurrentMnemonic(generatedMnemonic);
        localStorage.setItem('openAgentsWalletMnemonic', generatedMnemonic);
      } else {
         localStorage.setItem('openAgentsWalletMnemonic', mnemonic);
      }

      // Spark event handling might be different (e.g., on specific operations or via a global listener if available)
      // For now, we don't have a direct replacement for Breez's addEventListener here.

      await fetchWalletData(sparkInstance);
      setFlowState('wallet_ready');
      toast.success("Spark Wallet Connected!");
    } catch (error) {
      console.error('Failed to initialize Spark SDK:', error);
      const message = error instanceof Error ? error.message : String(error);
      setAppErrorMessage(`Spark Wallet initialization failed: ${message}`);
      setFlowState('error_state');
      sdkRef.current = null;
    }
  }, [fetchWalletData, currentMnemonic]); // Added currentMnemonic to ensure it's up-to-date if Spark generates one

  useEffect(() => {
    const storedMnemonic = localStorage.getItem('openAgentsWalletMnemonic');
    if (storedMnemonic) {
      setCurrentMnemonic(storedMnemonic);
      connectToSparkSDK(storedMnemonic);
    } else {
      setFlowState('login');
    }
  }, [connectToSparkSDK]);

  const handleLogout = useCallback(async () => {
    // Spark SDK docs don't show a disconnect method.
    // We'll clear local state and storage.
    // if (sdkRef.current && eventListenerIdRef.current) { // If Spark had events
    //   // await sdkRef.current.removeEventListener(eventListenerIdRef.current);
    //   // eventListenerIdRef.current = null;
    // }
    sdkRef.current = null;
    localStorage.removeItem('openAgentsWalletMnemonic');
    setCurrentMnemonic(null);
    setWalletInfo({ balanceSat: BigInt(0) });
    setGeneratedInvoice('');
    // setCalculatedFees(BigInt(0));
    setFlowState('login');
    toast.info("Successfully logged out.");
  }, []);


  const handleCreateNewWallet = () => setFlowState('creating_disclaimer');
  const handleDisclaimerAccepted = () => {
    const newMnemonic = bip39.generateMnemonic(wordlist);
    setCurrentMnemonic(newMnemonic);
    setFlowState('showing_mnemonic');
  };
  const handleMnemonicSavedAndConfirmed = () => {
    if (currentMnemonic) {
      connectToSparkSDK(currentMnemonic);
    } else {
      setAppErrorMessage("Error: Mnemonic not available.");
      setFlowState('error_state');
    }
  };
  const handleEnterExistingSeed = () => setFlowState('entering_seed');
  const handleSeedPhraseSubmitted = (seed: string) => {
    if (!bip39.validateMnemonic(seed, wordlist)) {
      toast.error("Invalid Seed Phrase", { description: "Please check your phrase and try again." });
      return;
    }
    setCurrentMnemonic(seed);
    connectToSparkSDK(seed);
  };

  const handleGenerateInvoice = async () => {
    if (!sdkRef.current) {
      toast.error("Wallet not initialized.");
      return;
    }
    try {
      const amountNumber = Number(receiveAmountSats); // Spark expects number for amountSats
      if (isNaN(amountNumber) || amountNumber <= 0) {
        toast.error("Invalid amount for invoice.");
        return;
      }

      const invoiceString = await sdkRef.current.createLightningInvoice({
        amountSats: amountNumber,
        memo: "OpenAgents Invoice" // Example memo
      });

      setGeneratedInvoice(invoiceString); // Spark returns the BOLT11 string directly
      // setCalculatedFees(BigInt(0)); // Fees are part of the invoice or handled by sender with Spark
      toast.success("Spark Lightning Invoice Generated!");
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      toast.error("Failed to generate Spark invoice", { description: message });
      console.error('Failed to generate Spark invoice:', error);
    }
  };

  const formatSats = (sats: bigint) => `₿ ${sats.toLocaleString('en-US')}`;

  const renderCurrentScreen = () => {
    switch (flowState) {
      case 'login':
        return <LoginScreen onCreateWallet={handleCreateNewWallet} onEnterSeed={handleEnterExistingSeed} />;
      case 'creating_disclaimer':
        return <CreateWalletDisclaimerScreen onNext={handleDisclaimerAccepted} />;
      case 'showing_mnemonic':
        return currentMnemonic ? <ShowMnemonicScreen mnemonic={currentMnemonic} onNext={handleMnemonicSavedAndConfirmed} /> :
          <div className="flex items-center justify-center min-h-screen"><Loader2 className="h-8 w-8 animate-spin" /> <p className="ml-2">Generating...</p></div>;
      case 'entering_seed':
        return <EnterSeedScreen onSeedEntered={handleSeedPhraseSubmitted} />;
      case 'initializing_wallet':
        return (
          <div className="flex flex-col items-center justify-center min-h-screen">
            <Loader2 className="h-12 w-12 animate-spin text-primary mb-4" />
            <p className="text-lg text-muted-foreground">Initializing Your Spark Wallet...</p>
            <p className="text-sm text-muted-foreground">This may take a moment.</p>
          </div>
        );
      case 'wallet_ready':
        return (
          <div className="container mx-auto p-4 max-w-3xl py-6">
            <div className="flex justify-between items-center mb-6">
              <h1 className="text-xl font-medium">OpenAgents Spark Wallet</h1>
              <div className="flex items-center gap-2">
                <ModeToggle />
                <UiButton variant="outline" size="sm" onClick={handleLogout}>Logout</UiButton>
              </div>
            </div>
            <Card className="mb-6">
              <CardHeader>
                <CardTitle>Spark Wallet Balance</CardTitle>
                <CardDescription>
                  Overview of your current wallet balance.
                  <span className="inline-block ml-1 text-xs text-muted-foreground">
                    (Main balance in satoshis)
                  </span>
                </CardDescription>
              </CardHeader>
              <CardContent>
                <div>
                  <h3 className="text-sm font-medium mb-1">Available Balance</h3>
                  <p className="text-2xl font-bold">{formatSats(walletInfo.balanceSat)}</p>
                </div>
                {/* Placeholder for token balances if you decide to display them */}
                {/* {walletInfo.tokenBalances && walletInfo.tokenBalances.size > 0 && (
                  <div className="mt-4">
                    <h4 className="text-md font-medium mb-2">Token Balances:</h4>
                    {Array.from(walletInfo.tokenBalances.entries()).map(([tokenId, tokenData]) => (
                      <div key={tokenId} className="text-sm">
                        {tokenData.tokenInfo?.name || tokenId.substring(0,8)}: {tokenData.balance.toString()}
                      </div>
                    ))}
                  </div>
                )} */}
              </CardContent>
            </Card>

            <Card>
              <CardHeader>
                <CardTitle>Receive Payment (Lightning)</CardTitle>
                <CardDescription>Generate a Lightning invoice to receive funds via Spark.</CardDescription>
              </CardHeader>
              <CardContent className="flex flex-col gap-4">
                <div className="space-y-2">
                  <Label htmlFor="receive-amount-spark">Amount (sats)</Label>
                  <UiInput
                    id="receive-amount-spark"
                    type="number"
                    value={receiveAmountSats.toString()}
                    onChange={(e) => setReceiveAmountSats(BigInt(e.target.value || "0"))}
                    // Spark doesn't provide min/max for invoice creation in the same way.
                    // Add reasonable client-side validation if needed.
                    min="1" // Example: basic client-side min
                  />
                  {/* <p className="text-sm text-muted-foreground">
                    No explicit min/max from SDK for invoice creation.
                  </p> */}
                </div>
                <UiButton
                  onClick={handleGenerateInvoice}
                  disabled={!sdkRef.current || receiveAmountSats <= 0}
                  className="w-full"
                >
                  Generate Spark Invoice
                </UiButton>
                {/* Fees are part of the invoice with Spark, not shown separately before creation */}
                {/* {calculatedFees > 0 && (
                  <p className="text-sm text-muted-foreground">Estimated Network Fees: {calculatedFees.toString()} sats</p>
                )} */}
                {generatedInvoice && (
                  <div className="mt-4 space-y-2">
                    <div className="flex justify-between items-center">
                      <h3 className="text-sm font-medium">Spark Lightning Invoice</h3>
                      <UiButton
                        variant="outline"
                        size="sm"
                        onClick={() => {
                          navigator.clipboard.writeText(generatedInvoice);
                          toast.success("Spark Invoice Copied!");
                        }}
                      >
                        Copy
                      </UiButton>
                    </div>
                    <ScrollArea className="h-24 w-full rounded-md border p-2">
                      <div className="p-2 font-mono text-sm break-all">
                        {generatedInvoice}
                      </div>
                    </ScrollArea>
                  </div>
                )}
              </CardContent>
            </Card>
            <div className="h-16"/> {/* Spacer for scroll */}
          </div>
        );
      case 'error_state':
        return (
          <div className="flex flex-col items-center justify-center min-h-screen p-4 text-center">
            <h2 className="text-2xl font-semibold text-destructive mb-4">Wallet Error</h2>
            <p className="text-muted-foreground mb-6">{appErrorMessage || "An unexpected error occurred."}</p>
            <UiButton onClick={handleLogout}>Return to Login</UiButton>
          </div>
        );
      default:
        return <div className="flex items-center justify-center min-h-screen"><Loader2 className="h-8 w-8 animate-spin" /></div>;
    }
  };

  return (
    <div className="fixed inset-0 overflow-hidden bg-background flex flex-col">
      <Toaster richColors />
      <main className="flex-1 overflow-hidden">
        <ScrollArea className="h-full">
          {renderCurrentScreen()}
        </ScrollArea>
      </main>
    </div>
  );
}

export default App;
```

**III. Update Vite Configuration (if necessary)**

The Spark SDK is a standard JavaScript/TypeScript library and typically doesn't require special WASM or top-level await plugins in Vite like the Breez WASM SDK might. You can simplify `vite.config.ts`:

```typescript
import { defineConfig, PluginOption } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import path from 'path'
// import wasm from "vite-plugin-wasm"; // Likely not needed for Spark SDK
// import topLevelAwait from "vite-plugin-top-level-await"; // Likely not needed

export default defineConfig({
  plugins: [
    react(),
    tailwindcss(),
    // wasm(), // Remove or comment out
    // topLevelAwait() // Remove or comment out
  ] as PluginOption[],
  // optimizeDeps: { // Typically not needed for standard JS SDKs
  //   exclude: ['@buildonspark/spark-sdk']
  // },
  build: {
    target: 'esnext', // Keep this for modern JS features
  },
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
})
```
**Note:** If the Spark SDK *does* internally use WASM or top-level await in a way that Vite's default handling doesn't cover, you might need to re-add those plugins. Start without them and add if build/runtime errors occur.

**IV. Environment Variables**
The Spark SDK does not seem to require an API key for client-side initialization according to the provided `SparkWallet.create` documentation. The `VITE_BREEZ_API_KEY` is specific to Breez and can be removed from `.env` and `App.tsx` if no other part of your application uses it. If Spark requires an API key for other operations (e.g., interacting with an LRC20 API), you'd add that configuration to `SparkWallet.create({ lrc20WalletApiConfig: ... })`. For this task, we're focusing on the client-side wallet.

**V. Testing**

1.  **Clean Installation:** Delete `node_modules` and your lock file (`pnpm-lock.yaml`, `yarn.lock`, or `package-lock.json`) and run `pnpm install` (or your package manager's install command) again to ensure a clean state with the new SDK.
2.  Run `npm run dev`.
3.  **Create Wallet Flow:**
    *   Test creating a new wallet. The mnemonic generation should work.
    *   Wallet initialization should now use `SparkWallet.create()`. Check the console for "Spark Wallet Connected!" toast and any errors.
    *   The balance display should fetch from `sparkWallet.getBalance()`.
    *   Generating an invoice should call `sparkWallet.createLightningInvoice()`.
4.  **Logout and Restore Flow:**
    *   Test logging out.
    *   Test restoring a wallet using a previously generated seed phrase.
    *   Verify balance and invoice generation work after restoration.

**VI. Log Work**

Create/update `docs/20250512/0846-screens-log.md` (or a new log file like `0847-spark-sdk-integration-log.md`) with details of these changes:

```markdown
# Implementation Log: Spark SDK Integration

## Overview
This log documents the replacement of the Breez SDK with the Spark SDK for wallet initialization, balance fetching, and Lightning invoice generation. The existing multi-screen UI flow and BIP39 mnemonic handling are preserved.

## Key Changes

### Dependencies
- Removed `@breeztech/breez-sdk-liquid`.
- Added `@buildonspark/spark-sdk`.
- Updated `package.json` and ran package manager install.

### `src/App.tsx`
- **Imports:** Replaced Breez SDK imports with `SparkWallet` and related types from `@buildonspark/spark-sdk`.
- **State Management:**
    - `sdkRef` now typed as `SparkWallet | null`.
    - `WalletDisplayInfo` updated to reflect `SparkWallet.getBalance()` (includes `balance: bigint`, `tokenBalances` is available but not displayed in this iteration).
    - Removed Breez-specific state like `lightningLimits` for receiving, as Spark's invoice creation is direct.
    - Removed `eventListenerIdRef` as Spark's event model is different (not directly attaching to wallet instance in this phase).
- **`connectToSparkSDK` function (formerly `connectToBreezSDK`):**
    - Initializes Spark wallet using `SparkWallet.create({ mnemonicOrSeed, options: { network: "MAINNET" } })`.
    - Handles mnemonic persistence in `localStorage`.
    - Adapted error handling for Spark SDK initialization.
    - Removed Breez-specific global `init()` and event listener setup.
- **`fetchWalletData` function:**
    - Now calls `sdkRef.current.getBalance()` to retrieve balance.
    - Updates `walletInfo` state with the `balance` (and potentially `tokenBalances` in the future).
- **`handleLogout` function:**
    - Updated to reflect that Spark SDK docs don't show a `disconnect()` method. Logout primarily clears local state and `localStorage`, setting `sdkRef.current` to `null`.
- **`handleGenerateInvoice` function:**
    - Now calls `sdkRef.current.createLightningInvoice({ amountSats: number, memo: string })`.
    - Converts `receiveAmountSats` (BigInt) to a `number` for the Spark SDK.
    - Sets a default memo.
    - Directly sets the returned invoice string to state.
    - Removed Breez-specific `prepareReceivePayment` and fee pre-calculation steps for invoice generation.
- **UI Updates:**
    - Text in the main wallet view updated to "OpenAgents Spark Wallet".
    - Balance display reflects data from `sparkWallet.getBalance()`.
    - Invoice generation section updated for Spark's direct invoice creation. Removed display of pre-calculated fees for invoice generation.
    - Removed UI elements related to Breez-specific receive limits.

### Vite Configuration (`vite.config.ts`)
- Commented out/removed `vite-plugin-wasm` and `vite-plugin-top-level-await` as they are likely not needed for the standard JS Spark SDK.
- Commented out `optimizeDeps.exclude` for the Spark SDK.

### Environment Variables
- Noted that `VITE_BREEZ_API_KEY` is Breez-specific and likely not needed for Spark client-side wallet initialization unless specific Spark features (like LRC20 API interaction) require separate configuration.

## Testing Considerations
- Verified create new wallet flow with Spark.
- Verified restore wallet with seed phrase flow with Spark.
- Verified balance display.
- Verified Lightning invoice generation using Spark SDK.
- Ensured logout clears relevant state.

## Known Differences/Limitations based on provided Spark Docs:
- **Event Handling:** The provided Spark SDK documentation for `SparkWallet` doesn't show a direct equivalent to Breez SDK's `addEventListener` on the wallet instance for general events like 'synced'. Real-time updates might rely on re-fetching data after operations or if Spark provides a different event mechanism (e.g., for specific transactions). This implementation currently refreshes data after known operations.
- **Receiving Limits:** Spark SDK's `createLightningInvoice` doesn't seem to involve fetching min/max receivable limits beforehand in the same way Breez's `prepareReceivePayment` did. Client-side validation for amount might be simpler (e.g., amount > 0).
- **Fee Pre-calculation for Invoice:** Spark's `createLightningInvoice` is direct; fees are embedded in the invoice or handled by the sender, so there's no "prepare" step to show fees to the receiver beforehand.
- **Disconnect Method:** No explicit `disconnect()` method found for `SparkWallet` in the provided docs. Logout relies on clearing local state.

This migration focuses on the core requested functionalities: wallet creation/restoration, balance, and invoice generation using the Spark SDK.
```

This plan outlines the necessary code changes. The agent should be able to follow these steps to perform the SDK replacement. Remember that the Spark SDK's event handling and some other nuanced behaviors might differ from Breez, and further adjustments might be needed based on more detailed Spark documentation or runtime behavior.
