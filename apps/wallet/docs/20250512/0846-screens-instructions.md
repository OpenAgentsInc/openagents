Okay, I will provide instructions to modify your codebase to implement the new login and wallet creation flow.

**I. Project Setup & Shadcn/ui Component Installation**

1.  **Install necessary shadcn/ui components:**

EDIT: THIS IS ALREADY DONE. COMPONENTS ARE IN apps/wallet/src/components/ui

    Open your terminal in the project root and run the following commands:
    ```bash
    npx shadcn-ui@latest add button
    npx shadcn-ui@latest add card
    npx shadcn-ui@latest add input
    npx shadcn-ui@latest add label
    npx shadcn-ui@latest add alert
    npx shadcn-ui@latest add textarea
    # Tooltip can be useful for the copy button
    npx shadcn-ui@latest add tooltip
    ```
    When prompted, confirm overwriting existing files if necessary (e.g., if `button` was already implicitly added). Ensure your `components.json` is correctly configured for these additions.

**II. Create New Component Files**

Create the following new files in your `src/components` directory (or a new `src/screens` or `src/features/auth` directory if you prefer more organization):

1.  `LoginScreen.tsx`
2.  `CreateWalletDisclaimerScreen.tsx`
3.  `ShowMnemonicScreen.tsx`
4.  `EnterSeedScreen.tsx`

**III. Modify `src/App.tsx`**

Replace the content of `src/App.tsx` with the following. This introduces state management for different screens and modifies the wallet initialization logic.

```typescript
import { useEffect, useState, useRef, useCallback } from 'react'
import init, { defaultConfig, connect, BindingLiquidSdk, type WalletInfo as SdkWalletInfo, type LightningPaymentLimitsResponse } from '@breeztech/breez-sdk-liquid'
import * as bip39 from '@scure/bip39';
import { wordlist } from '@scure/bip39/wordlists/english';

// Shadcn UI - Main App components (already present, but for clarity)
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from '@/components/ui/card'
import { Input as UiInput } from '@/components/ui/input' // Renamed to avoid conflict
import { Button as UiButton } from '@/components/ui/button' // Renamed to avoid conflict
import { Badge } from '@/components/ui/badge'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Toaster } from '@/components/ui/sonner'
import { toast } from 'sonner'
import { ModeToggle } from '@/components/mode-toggle'

// New Screen Components (to be created)
import LoginScreen from './components/LoginScreen'; // Adjust path if needed
import CreateWalletDisclaimerScreen from './components/CreateWalletDisclaimerScreen'; // Adjust path
import ShowMnemonicScreen from './components/ShowMnemonicScreen'; // Adjust path
import EnterSeedScreen from './components/EnterSeedScreen'; // Adjust path

type WalletState = 'login' | 'creating_disclaimer' | 'showing_mnemonic' | 'entering_seed' | 'initializing_wallet' | 'wallet_ready' | 'error';

interface WalletInfo {
  balanceSat: bigint;
  pendingSendSat: bigint;
  pendingReceiveSat: bigint;
}

interface LightningLimits {
  min: bigint;
  max: bigint;
}

function App() {
  const [appState, setAppState] = useState<WalletState>('login');
  const [currentMnemonic, setCurrentMnemonic] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  // Wallet specific state (moved from original App for clarity)
  const [walletInfo, setWalletInfo] = useState<WalletInfo>({
    balanceSat: BigInt(0),
    pendingSendSat: BigInt(0),
    pendingReceiveSat: BigInt(0)
  });
  const [lightningLimits, setLightningLimits] = useState<LightningLimits>({
    min: BigInt(0),
    max: BigInt(0)
  });
  const [receiveAmount, setReceiveAmount] = useState(BigInt(10000)); // Default 10k sats
  const [invoice, setInvoice] = useState('');
  const [fees, setFees] = useState(BigInt(0));
  const sdkRef = useRef<BindingLiquidSdk | null>(null);
  const listenerIdRef = useRef<string | null>(null);

  const connectToBreez = useCallback(async (mnemonic: string) => {
    if (sdkRef.current) {
      console.log("SDK already connected or connecting.");
      return;
    }
    setAppState('initializing_wallet');
    setErrorMessage(null);

    try {
      await init(); // Ensure WASM is initialized

      const config = defaultConfig('mainnet', import.meta.env.VITE_BREEZ_API_KEY);
      // You might want to persist the workingDir if needed, or let it be in-memory for web
      // config.workingDir = `breez_sdk_liquid_${mnemonic.slice(0,10)}`; // Example

      const sdk = await connect({ mnemonic, config });
      sdkRef.current = sdk;

      // Setup event listener
       const eventListener = {
        onEvent: (event: any) => { // `any` for simplicity, use SdkEvent if type is available
          console.log('Breez SDK Event:', event.type, event);
          if (event.type === 'synced' || event.type === 'paymentSucceeded' || event.type === 'paymentFailed' || event.type === 'paymentPending' || event.type === 'paymentWaitingConfirmation' || event.type === 'paymentRefunded' || event.type === 'paymentRefundPending' || event.type === 'paymentWaitingFeeAcceptance') {
            fetchWalletData(sdk); // Refresh data on relevant events
          }
        }
      };
      const listenerId = await sdk.addEventListener(eventListener);
      listenerIdRef.current = listenerId;


      await fetchWalletData(sdk);
      setAppState('wallet_ready');
      localStorage.setItem('userMnemonic', mnemonic); // Persist mnemonic
    } catch (error) {
      console.error('Failed to initialize Breez SDK:', error);
      setErrorMessage(`Failed to initialize wallet: ${error instanceof Error ? error.message : String(error)}`);
      setAppState('error');
      sdkRef.current = null; // Reset SDK ref on error
    }
  }, []);

  const fetchWalletData = async (sdk: BindingLiquidSdk) => {
    try {
      const info = await sdk.getInfo();
      setWalletInfo({
        balanceSat: info.walletInfo.balanceSat,
        pendingSendSat: info.walletInfo.pendingSendSat,
        pendingReceiveSat: info.walletInfo.pendingReceiveSat
      });

      const limits: LightningPaymentLimitsResponse = await sdk.fetchLightningLimits();
      setLightningLimits({
        min: limits.receive.minSat,
        max: limits.receive.maxSat
      });
    } catch (error) {
      console.error('Failed to fetch wallet data:', error);
      toast.error("Failed to fetch wallet data.");
    }
  };

  useEffect(() => {
    // Attempt to auto-login if mnemonic exists
    const storedMnemonic = localStorage.getItem('userMnemonic');
    if (storedMnemonic) {
      setCurrentMnemonic(storedMnemonic);
      connectToBreez(storedMnemonic);
    }
  }, [connectToBreez]);


  const handleCreateWallet = () => {
    setAppState('creating_disclaimer');
  };

  const handleDisclaimerNext = () => {
    const newMnemonic = bip39.generateMnemonic(wordlist);
    setCurrentMnemonic(newMnemonic);
    setAppState('showing_mnemonic');
  };

  const handleMnemonicConfirmed = (mnemonic: string) => {
    connectToBreez(mnemonic);
  };

  const handleEnterSeed = () => {
    setAppState('entering_seed');
  };

  const handleSeedEntered = (mnemonic: string) => {
    // Basic validation
    if (!bip39.validateMnemonic(mnemonic, wordlist)) {
      toast.error("Invalid seed phrase. Please check and try again.");
      setErrorMessage("Invalid seed phrase.");
      return;
    }
    setCurrentMnemonic(mnemonic);
    connectToBreez(mnemonic);
  };

  const handleLogout = async () => {
    if (sdkRef.current && listenerIdRef.current) {
      try {
        await sdkRef.current.removeEventListener(listenerIdRef.current);
      } catch (e) {
        console.error("Error removing listener on logout:", e);
      }
    }
    if (sdkRef.current) {
      try {
        await sdkRef.current.disconnect();
      } catch(e) {
        console.error("Error disconnecting SDK on logout:", e);
      }
    }
    sdkRef.current = null;
    listenerIdRef.current = null;
    localStorage.removeItem('userMnemonic');
    setCurrentMnemonic(null);
    setWalletInfo({ balanceSat: BigInt(0), pendingSendSat: BigInt(0), pendingReceiveSat: BigInt(0) });
    setInvoice('');
    setFees(BigInt(0));
    setAppState('login');
    toast.info("Logged out successfully.");
  };


  const generateInvoice = async () => {
    if (!sdkRef.current) return;

    try {
      const optionalAmount = {
        type: 'bitcoin',
        payerAmountSat: receiveAmount
      } // No need to cast with ReceiveAmount as type

      const prepareResponse = await sdkRef.current.prepareReceivePayment({
        paymentMethod: 'lightning',
        amount: optionalAmount
      });

      setFees(prepareResponse.feesSat);

      const receiveResponse = await sdkRef.current.receivePayment({
        prepareResponse
      });

      if (typeof receiveResponse === 'object' && receiveResponse !== null && receiveResponse.destination) {
        setInvoice(receiveResponse.destination);
        toast.success("Invoice Generated!");
      }
    } catch (error) {
      console.error('Failed to generate invoice:', error);
      toast.error(`Invoice generation failed: ${error instanceof Error ? error.message : String(error)}`);
    }
  };

  // Helper function to format satoshis using the ₿ symbol standard
  const formatSatsWithBitcoinSymbol = (sats: bigint) => {
    return `₿ ${sats.toLocaleString('en-US')}`;
  };

  // Render different screens based on appState
  const renderContent = () => {
    switch (appState) {
      case 'login':
        return <LoginScreen onCreateWallet={handleCreateWallet} onEnterSeed={handleEnterSeed} />;
      case 'creating_disclaimer':
        return <CreateWalletDisclaimerScreen onNext={handleDisclaimerNext} />;
      case 'showing_mnemonic':
        if (!currentMnemonic) return <p>Error: Mnemonic not generated.</p>;
        return <ShowMnemonicScreen mnemonic={currentMnemonic} onNext={() => handleMnemonicConfirmed(currentMnemonic)} />;
      case 'entering_seed':
        return <EnterSeedScreen onSeedEntered={handleSeedEntered} />;
      case 'initializing_wallet':
        return (
          <div className="flex flex-col items-center justify-center h-screen">
            <p className="text-lg">Initializing Wallet...</p>
            {/* You can add a spinner here */}
          </div>
        );
      case 'wallet_ready':
        return (
          // Existing Wallet UI from original App.tsx, adapted
          <div className="container mx-auto p-4 max-w-3xl py-6">
            <div className="flex justify-between items-center mb-6">
              <h1 className="text-xl font-medium">OpenAgents Wallet</h1>
              <div className="flex items-center gap-2">
                <ModeToggle />
                <UiButton variant="outline" size="sm" onClick={handleLogout}>Logout</UiButton>
              </div>
            </div>
            <Card className="mb-6">
              <CardHeader>
                <CardTitle>Wallet Balance</CardTitle>
                <CardDescription>
                  Overview of your current wallet balances.
                  <span className="inline-block ml-1 text-xs text-muted-foreground">
                    (Values shown in satoshis: ₿ 100,000,000 = 1 BTC)
                  </span>
                </CardDescription>
              </CardHeader>
              <CardContent className="grid grid-cols-1 md:grid-cols-3 gap-4">
                <div>
                  <h3 className="text-sm font-medium mb-1">Available Balance</h3>
                  <p className="text-xl font-bold">{formatSatsWithBitcoinSymbol(walletInfo.balanceSat)}</p>
                </div>
                <div>
                  <h3 className="text-sm font-medium mb-1">Pending Send</h3>
                  <p className="text-xl font-bold">{formatSatsWithBitcoinSymbol(walletInfo.pendingSendSat)}</p>
                </div>
                <div>
                  <h3 className="text-sm font-medium mb-1">Pending Receive</h3>
                  <p className="text-xl font-bold">{formatSatsWithBitcoinSymbol(walletInfo.pendingReceiveSat)}</p>
                </div>
              </CardContent>
            </Card>

            <Card>
              <CardHeader>
                <CardTitle>Receive Payment</CardTitle>
                <CardDescription>Generate a lightning invoice to receive funds</CardDescription>
              </CardHeader>
              <CardContent className="flex flex-col gap-4">
                <div className="space-y-2">
                  <label className="text-sm font-medium">Amount (sats)</label>
                  <UiInput
                    type="number"
                    value={receiveAmount.toString()}
                    onChange={(e) => setReceiveAmount(BigInt(e.target.value || "0"))}
                    min={lightningLimits.min.toString()}
                    max={lightningLimits.max.toString()}
                  />
                  <p className="text-sm text-muted-foreground">
                    Min: {lightningLimits.min.toString()} sats, Max: {lightningLimits.max.toString()} sats
                  </p>
                </div>
                <UiButton
                  onClick={generateInvoice}
                  disabled={!sdkRef.current || receiveAmount < lightningLimits.min || receiveAmount > lightningLimits.max}
                  className="w-full"
                >
                  Generate Invoice
                </UiButton>
                {fees > 0 && (
                  <p className="text-sm text-muted-foreground">Network Fees: {fees.toString()} sats</p>
                )}
                {invoice && (
                  <div className="mt-4 space-y-2">
                    <div className="flex justify-between items-center">
                      <h3 className="text-sm font-medium">Lightning Invoice</h3>
                      <UiButton
                        variant="outline"
                        size="sm"
                        onClick={() => {
                          navigator.clipboard.writeText(invoice);
                          toast.success("Invoice Copied");
                        }}
                      >
                        Copy Invoice
                      </UiButton>
                    </div>
                    <ScrollArea className="h-24 w-full rounded-md border p-2">
                      <div className="p-2 font-mono text-sm break-all">
                        {invoice}
                      </div>
                    </ScrollArea>
                  </div>
                )}
              </CardContent>
            </Card>
          </div>
        );
      case 'error':
        return (
          <div className="flex flex-col items-center justify-center h-screen">
            <p className="text-red-500 text-lg mb-4">Error: {errorMessage}</p>
            <UiButton onClick={() => setAppState('login')}>Go to Login</UiButton>
          </div>
        );
      default:
        return <LoginScreen onCreateWallet={handleCreateWallet} onEnterSeed={handleEnterSeed} />;
    }
  };

  return (
    <div className="fixed inset-0 overflow-hidden bg-background flex flex-col">
      <Toaster />
      <main className="flex-1 overflow-hidden">
        <ScrollArea className="h-full">
          {renderContent()}
        </ScrollArea>
      </main>
    </div>
  );
}

export default App;
```

**IV. Implement `LoginScreen.tsx`**

Create `src/components/LoginScreen.tsx` (or your chosen path) with the following content:

```typescript
import { Button } from "@/components/ui/button";
import { Github } from "lucide-react";

interface LoginScreenProps {
  onCreateWallet: () => void;
  onEnterSeed: () => void;
}

const LoginScreen: React.FC<LoginScreenProps> = ({ onCreateWallet, onEnterSeed }) => {
  return (
    <div className="flex flex-col items-center justify-center min-h-screen bg-background text-foreground p-4">
      <div className="flex flex-col items-center text-center">
        <h1 className="text-5xl font-bold mb-8">OpenAgents</h1>
        <div className="flex flex-col space-y-4 w-full max-w-xs">
          <Button onClick={onCreateWallet} className="w-full" size="lg">
            Create New Wallet
          </Button>
          <Button onClick={onEnterSeed} variant="outline" className="w-full" size="lg">
            Enter Seed Phrase
          </Button>
        </div>
      </div>
      <footer className="absolute bottom-6 text-center text-xs text-muted-foreground">
        An {" "}
        <a
          href="https://github.com/OpenAgentsInc/openagents/tree/main/apps/wallet"
          target="_blank"
          rel="noopener noreferrer"
          className="underline hover:text-primary inline-flex items-center"
        >
          open source
        </a>
        self-custodial bitcoin wallet
      </footer>
    </div>
  );
};

export default LoginScreen;
```

**V. Implement `CreateWalletDisclaimerScreen.tsx`**

Create `src/components/CreateWalletDisclaimerScreen.tsx` with:

```typescript
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from "@/components/ui/card";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Terminal } from "lucide-react";


interface CreateWalletDisclaimerScreenProps {
  onNext: () => void;
}

const CreateWalletDisclaimerScreen: React.FC<CreateWalletDisclaimerScreenProps> = ({ onNext }) => {
  return (
    <div className="flex items-center justify-center min-h-screen bg-background p-4">
      <Card className="w-full max-w-md">
        <CardHeader>
          <CardTitle>Important Notice</CardTitle>
          <CardDescription>Please read carefully before proceeding.</CardDescription>
        </CardHeader>
        <CardContent>
          <Alert variant="destructive">
            <Terminal className="h-4 w-4" />
            <AlertTitle>Self-Custody Wallet</AlertTitle>
            <AlertDescription>
              OpenAgents wallet is self-custodial. OpenAgents cannot access your funds or help recover them if lost. You are solely responsible for securing your seed phrase.
            </AlertDescription>
          </Alert>
        </CardContent>
        <CardFooter>
          <Button onClick={onNext} className="w-full">
            I Understand, Continue
          </Button>
        </CardFooter>
      </Card>
    </div>
  );
};

export default CreateWalletDisclaimerScreen;
```

**VI. Implement `ShowMnemonicScreen.tsx`**

Create `src/components/ShowMnemonicScreen.tsx` with:

```typescript
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from "@/components/ui/card";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Copy, ShieldAlert } from "lucide-react";
import { toast } from "sonner";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip"


interface ShowMnemonicScreenProps {
  mnemonic: string;
  onNext: () => void;
}

const ShowMnemonicScreen: React.FC<ShowMnemonicScreenProps> = ({ mnemonic, onNext }) => {
  const words = mnemonic.split(' ');

  const handleCopyMnemonic = () => {
    navigator.clipboard.writeText(mnemonic);
    toast.success("Seed Phrase Copied!", {
      description: "Your 12-word seed phrase has been copied to the clipboard.",
    });
  };

  return (
    <div className="flex items-center justify-center min-h-screen bg-background p-4">
      <Card className="w-full max-w-lg">
        <CardHeader>
          <CardTitle>Your Secret Recovery Phrase</CardTitle>
          <CardDescription>
            Write down these 12 words in order and keep them somewhere safe.
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-6">
          <Alert variant="destructive">
            <ShieldAlert className="h-4 w-4" />
            <AlertTitle>Critical Warning!</AlertTitle>
            <AlertDescription>
              This is your password to your money. If you lose it, you will lose your money! Never share this phrase with anyone.
            </AlertDescription>
          </Alert>

          <div className="grid grid-cols-2 md:grid-cols-3 gap-3 p-4 border rounded-md bg-muted/50">
            {words.map((word, index) => (
              <div key={index} className="flex items-center p-2 bg-background border rounded-md">
                <span className="text-xs text-muted-foreground mr-2 select-none">{index + 1}.</span>
                <span className="font-medium">{word}</span>
              </div>
            ))}
          </div>

          <TooltipProvider>
            <Tooltip delayDuration={100}>
              <TooltipTrigger asChild>
                <Button onClick={handleCopyMnemonic} variant="outline" className="w-full">
                  <Copy className="mr-2 h-4 w-4" /> Copy Seed Phrase
                </Button>
              </TooltipTrigger>
              <TooltipContent>
                <p>Copy all 12 words to clipboard</p>
              </TooltipContent>
            </Tooltip>
          </TooltipProvider>

        </CardContent>
        <CardFooter>
          <Button onClick={onNext} className="w-full">
            I Have Saved My Seed Phrase, Next
          </Button>
        </CardFooter>
      </Card>
    </div>
  );
};

export default ShowMnemonicScreen;
```

**VII. Implement `EnterSeedScreen.tsx`**

Create `src/components/EnterSeedScreen.tsx` with:

```typescript
import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { toast } from "sonner";

interface EnterSeedScreenProps {
  onSeedEntered: (seed: string) => void;
}

const EnterSeedScreen: React.FC<EnterSeedScreenProps> = ({ onSeedEntered }) => {
  const [seedPhrase, setSeedPhrase] = useState('');

  const handleSubmit = () => {
    const trimmedSeed = seedPhrase.trim().toLowerCase();
    const words = trimmedSeed.split(/\s+/); // Split by any whitespace

    if (words.length !== 12 && words.length !== 24) { // Common lengths
      toast.error("Invalid Seed Phrase", {
        description: "Seed phrases usually have 12 or 24 words. Please check your input.",
      });
      return;
    }
    onSeedEntered(trimmedSeed);
  };

  return (
    <div className="flex items-center justify-center min-h-screen bg-background p-4">
      <Card className="w-full max-w-md">
        <CardHeader>
          <CardTitle>Enter Your Seed Phrase</CardTitle>
          <CardDescription>
            Enter your 12 or 24 word recovery phrase to restore your wallet. Separate words with spaces.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <div className="grid w-full gap-1.5">
            <Label htmlFor="seed-phrase">Seed Phrase</Label>
            <Textarea
              id="seed-phrase"
              placeholder="Enter your seed phrase here..."
              value={seedPhrase}
              onChange={(e) => setSeedPhrase(e.target.value)}
              rows={4}
              className="resize-none"
            />
          </div>
        </CardContent>
        <CardFooter>
          <Button onClick={handleSubmit} className="w-full">
            Restore Wallet
          </Button>
        </CardFooter>
      </Card>
    </div>
  );
};

export default EnterSeedScreen;
```

**VIII. Update `src/index.css` (if needed)**

Ensure your `src/index.css` has the base Tailwind directives. It should already be configured from your `components.json`.

**IX. Environment Variables**

Ensure you have `VITE_BREEZ_API_KEY` set in your `.env` file at the project root:
`.env`
```
VITE_BREEZ_API_KEY=your_actual_breez_api_key_here
```

**X. Test the Flow**

1.  Run `npm run dev`.
2.  You should see the "OpenAgents" login screen.
3.  Test the "Create New Wallet" flow:
    *   Click "Create New Wallet".
    *   See the disclaimer. Click "I Understand, Continue".
    *   See the 12-word mnemonic and the warning.
    *   Copy the mnemonic. Click "I Have Saved My Seed Phrase, Next".
    *   The wallet should initialize and show the main wallet interface.
4.  Logout (using the new logout button).
5.  Test the "Enter Seed Phrase" flow:
    *   Click "Enter Seed Phrase".
    *   Enter the mnemonic you copied (or any valid 12-word mnemonic).
    *   Click "Restore Wallet".
    *   The wallet should initialize and show the main wallet interface.

This set of instructions should guide you through implementing the requested features. Remember to replace placeholder API keys and handle mnemonics securely in a production environment.Okay, I've analyzed your request and the provided codebase. Here's a plan to implement the new login and wallet creation flow using React, TypeScript, Vite, and shadcn/ui components.

**I. Project Setup & Shadcn/ui Component Installation**

1.  **Ensure shadcn/ui is initialized:**
    Your `components.json` indicates shadcn/ui is set up. If you haven't already, make sure you've run the init command:
    ```bash
    npx shadcn-ui@latest init
    ```

2.  **Install necessary shadcn/ui components:**
    Open your terminal in the project root and run the following commands to add the components we'll need:
    ```bash
    npx shadcn-ui@latest add button
    npx shadcn-ui@latest add card
    npx shadcn-ui@latest add input
    npx shadcn-ui@latest add label
    npx shadcn-ui@latest add alert
    npx shadcn-ui@latest add textarea
    npx shadcn-ui@latest add tooltip
    ```
    Confirm any prompts, especially if it asks to overwrite existing base styles or utils (usually fine if you're starting fresh with these components).

**II. Create New Component Files**

In your `src/components` directory, create the following new component files. For better organization, you might consider a subfolder like `src/components/auth/` or `src/screens/`.

1.  `LoginScreen.tsx`
2.  `CreateWalletDisclaimerScreen.tsx`
3.  `ShowMnemonicScreen.tsx`
4.  `EnterSeedScreen.tsx`

**III. Modify `src/App.tsx`**

This file will now act as a router, managing which screen is displayed based on the application's state. It will also hold the core wallet initialization logic.

Replace the content of `src/App.tsx` with the following:

```typescript
import { useEffect, useState, useRef, useCallback } from 'react'
import init, { defaultConfig, connect, BindingLiquidSdk, SdkEvent, type WalletInfo as SdkWalletInfo, type LightningPaymentLimitsResponse } from '@breeztech/breez-sdk-liquid'
import * as bip39 from '@scure/bip39';
import { wordlist } from '@scure/bip39/wordlists/english';

// Shadcn UI - Main App components
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from '@/components/ui/card'
import { Input as UiInput } from '@/components/ui/input'
import { Button as UiButton } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Toaster } from '@/components/ui/sonner'
import { toast } from 'sonner'
import { ModeToggle } from '@/components/mode-toggle'
import { Loader2 } from 'lucide-react';


// New Screen Components
import LoginScreen from './components/LoginScreen';
import CreateWalletDisclaimerScreen from './components/CreateWalletDisclaimerScreen';
import ShowMnemonicScreen from './components/ShowMnemonicScreen';
import EnterSeedScreen from './components/EnterSeedScreen';

type WalletFlowState = 'login' | 'creating_disclaimer' | 'showing_mnemonic' | 'entering_seed' | 'initializing_wallet' | 'wallet_ready' | 'error_state';

interface WalletDisplayInfo {
  balanceSat: bigint;
  pendingSendSat: bigint;
  pendingReceiveSat: bigint;
}

interface LightningLimitsInfo {
  min: bigint;
  max: bigint;
}

function App() {
  const [flowState, setFlowState] = useState<WalletFlowState>('login');
  const [currentMnemonic, setCurrentMnemonic] = useState<string | null>(null);
  const [appErrorMessage, setAppErrorMessage] = useState<string | null>(null);

  const [walletInfo, setWalletInfo] = useState<WalletDisplayInfo>({
    balanceSat: BigInt(0),
    pendingSendSat: BigInt(0),
    pendingReceiveSat: BigInt(0)
  });
  const [lightningLimits, setLightningLimits] = useState<LightningLimitsInfo>({
    min: BigInt(0),
    max: BigInt(0)
  });
  const [receiveAmount, setReceiveAmount] = useState(BigInt(10000));
  const [generatedInvoice, setGeneratedInvoice] = useState('');
  const [calculatedFees, setCalculatedFees] = useState(BigInt(0));

  const sdkRef = useRef<BindingLiquidSdk | null>(null);
  const eventListenerIdRef = useRef<string | null>(null);

  const fetchWalletData = useCallback(async (sdk: BindingLiquidSdk) => {
    if (!sdk) return;
    try {
      const info: SdkWalletInfo = await sdk.getInfo();
      setWalletInfo({
        balanceSat: info.walletInfo.balanceSat,
        pendingSendSat: info.walletInfo.pendingSendSat,
        pendingReceiveSat: info.walletInfo.pendingReceiveSat
      });

      const limits: LightningPaymentLimitsResponse = await sdk.fetchLightningLimits();
      setLightningLimits({
        min: limits.receive.minSat,
        max: limits.receive.maxSat
      });
    } catch (error) {
      console.error('Failed to fetch wallet data:', error);
      toast.error("Error fetching wallet data. Please try again.");
    }
  }, []);

  const connectToBreezSDK = useCallback(async (mnemonic: string) => {
    if (sdkRef.current) {
      console.log("SDK connection attempt skipped: already connected or connecting.");
      return;
    }
    setFlowState('initializing_wallet');
    setAppErrorMessage(null);

    try {
      await init();

      const config = defaultConfig('mainnet', import.meta.env.VITE_BREEZ_API_KEY);
      // For web, workingDir is usually not needed or can be a unique identifier if multiple wallets are managed.
      // Example: config.workingDir = `breez_sdk_liquid_web_wallet`;

      const sdk = await connect({ mnemonic, config });
      sdkRef.current = sdk;

      const eventListener = {
        onEvent: (event: SdkEvent) => {
          console.log('Breez SDK Event:', event.type, event);
          if (event.type === 'synced' ||
              event.type === 'paymentSucceeded' ||
              event.type === 'paymentFailed' ||
              event.type === 'paymentWaitingConfirmation' // Added more relevant events
            ) {
            fetchWalletData(sdk);
          }
        }
      };
      const listenerId = await sdk.addEventListener(eventListener);
      eventListenerIdRef.current = listenerId;

      await fetchWalletData(sdk);
      setFlowState('wallet_ready');
      localStorage.setItem('openAgentsWalletMnemonic', mnemonic);
      toast.success("Wallet Connected!");
    } catch (error) {
      console.error('Failed to initialize Breez SDK:', error);
      const message = error instanceof Error ? error.message : String(error);
      setAppErrorMessage(`Wallet initialization failed: ${message}`);
      setFlowState('error_state');
      sdkRef.current = null;
    }
  }, [fetchWalletData]);

  useEffect(() => {
    const storedMnemonic = localStorage.getItem('openAgentsWalletMnemonic');
    if (storedMnemonic) {
      setCurrentMnemonic(storedMnemonic);
      connectToBreezSDK(storedMnemonic);
    } else {
      setFlowState('login');
    }
  }, [connectToBreezSDK]);

  const handleLogout = useCallback(async () => {
    if (sdkRef.current) {
      if (eventListenerIdRef.current) {
        try {
          await sdkRef.current.removeEventListener(eventListenerIdRef.current);
          eventListenerIdRef.current = null;
        } catch (e) { console.error("Error removing listener:", e); }
      }
      try {
        await sdkRef.current.disconnect();
      } catch(e) { console.error("Error disconnecting SDK:", e); }
      sdkRef.current = null;
    }
    localStorage.removeItem('openAgentsWalletMnemonic');
    setCurrentMnemonic(null);
    setWalletInfo({ balanceSat: BigInt(0), pendingSendSat: BigInt(0), pendingReceiveSat: BigInt(0) });
    setGeneratedInvoice('');
    setCalculatedFees(BigInt(0));
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
      connectToBreezSDK(currentMnemonic);
    } else {
      setAppErrorMessage("Error: Mnemonic not available.");
      setFlowState('error_state');
    }
  };
  const handleEnterExistingSeed = () => setFlowState('entering_seed');
  const handleSeedPhraseSubmitted = (seed: string) => {
    if (!bip39.validateMnemonic(seed, wordlist)) {
      toast.error("Invalid Seed Phrase", { description: "Please check your 12-word phrase and try again." });
      return;
    }
    setCurrentMnemonic(seed);
    connectToBreezSDK(seed);
  };

  const handleGenerateInvoice = async () => {
    if (!sdkRef.current) return;
    try {
      const optionalAmount = {
        type: 'bitcoin',
        payerAmountSat: receiveAmount
      };
      const prepareResponse = await sdkRef.current.prepareReceivePayment({
        paymentMethod: 'lightning',
        amount: optionalAmount
      });
      setCalculatedFees(prepareResponse.feesSat);
      const receiveResponse = await sdkRef.current.receivePayment({ prepareResponse });
      if (receiveResponse && receiveResponse.destination) {
        setGeneratedInvoice(receiveResponse.destination);
        toast.success("Lightning Invoice Generated!");
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      toast.error("Failed to generate invoice", { description: message });
      console.error('Failed to generate invoice:', error);
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
        return currentMnemonic ? <ShowMnemonicScreen mnemonic={currentMnemonic} onNext={handleMnemonicSavedAndConfirmed} /> : <p>Generating mnemonic...</p>;
      case 'entering_seed':
        return <EnterSeedScreen onSeedEntered={handleSeedPhraseSubmitted} />;
      case 'initializing_wallet':
        return (
          <div className="flex flex-col items-center justify-center min-h-screen">
            <Loader2 className="h-12 w-12 animate-spin text-primary mb-4" />
            <p className="text-lg text-muted-foreground">Initializing Your Wallet...</p>
            <p className="text-sm text-muted-foreground">This may take a moment.</p>
          </div>
        );
      case 'wallet_ready':
        return (
          <div className="container mx-auto p-4 max-w-3xl py-6">
            <div className="flex justify-between items-center mb-6">
              <h1 className="text-xl font-medium">OpenAgents Wallet</h1>
              <div className="flex items-center gap-2">
                <ModeToggle />
                <UiButton variant="outline" size="sm" onClick={handleLogout}>Logout</UiButton>
              </div>
            </div>
            <Card className="mb-6">
              <CardHeader>
                <CardTitle>Wallet Balance</CardTitle>
                <CardDescription>
                  Overview of your current wallet balances.
                  <span className="inline-block ml-1 text-xs text-muted-foreground">
                    (Values in satoshis)
                  </span>
                </CardDescription>
              </CardHeader>
              <CardContent className="grid grid-cols-1 md:grid-cols-3 gap-4">
                <div>
                  <h3 className="text-sm font-medium mb-1">Available</h3>
                  <p className="text-xl font-bold">{formatSats(walletInfo.balanceSat)}</p>
                </div>
                <div>
                  <h3 className="text-sm font-medium mb-1">Pending Send</h3>
                  <p className="text-xl font-bold">{formatSats(walletInfo.pendingSendSat)}</p>
                </div>
                <div>
                  <h3 className="text-sm font-medium mb-1">Pending Receive</h3>
                  <p className="text-xl font-bold">{formatSats(walletInfo.pendingReceiveSat)}</p>
                </div>
              </CardContent>
            </Card>

            <Card>
              <CardHeader>
                <CardTitle>Receive Payment</CardTitle>
                <CardDescription>Generate a lightning invoice to receive funds</CardDescription>
              </CardHeader>
              <CardContent className="flex flex-col gap-4">
                <div className="space-y-2">
                  <Label htmlFor="receive-amount">Amount (sats)</Label>
                  <UiInput
                    id="receive-amount"
                    type="number"
                    value={receiveAmount.toString()}
                    onChange={(e) => setReceiveAmount(BigInt(e.target.value || "0"))}
                    min={lightningLimits.min.toString()}
                    max={lightningLimits.max.toString()}
                  />
                  <p className="text-sm text-muted-foreground">
                    Min: {lightningLimits.min.toString()} sats, Max: {lightningLimits.max.toString()} sats
                  </p>
                </div>
                <UiButton
                  onClick={handleGenerateInvoice}
                  disabled={!sdkRef.current || receiveAmount < lightningLimits.min || receiveAmount > lightningLimits.max}
                  className="w-full"
                >
                  Generate Invoice
                </UiButton>
                {calculatedFees > 0 && (
                  <p className="text-sm text-muted-foreground">Estimated Network Fees: {calculatedFees.toString()} sats</p>
                )}
                {generatedInvoice && (
                  <div className="mt-4 space-y-2">
                    <div className="flex justify-between items-center">
                      <h3 className="text-sm font-medium">Lightning Invoice</h3>
                      <UiButton
                        variant="outline"
                        size="sm"
                        onClick={() => {
                          navigator.clipboard.writeText(generatedInvoice);
                          toast.success("Invoice Copied!");
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
        return <p>Loading...</p>;
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

**IV. Implement `src/components/LoginScreen.tsx`**

```typescript
import { Button } from "@/components/ui/button";
import { Github } from "lucide-react";

interface LoginScreenProps {
  onCreateWallet: () => void;
  onEnterSeed: () => void;
}

const LoginScreen: React.FC<LoginScreenProps> = ({ onCreateWallet, onEnterSeed }) => {
  return (
    <div className="flex flex-col items-center justify-center min-h-screen bg-background text-foreground p-4">
      <div className="flex flex-col items-center text-center">
        {/* You can add an SVG logo here if you have one */}
        {/* <img src="/logo.svg" alt="OpenAgents Logo" className="w-24 h-24 mb-6" /> */}
        <h1 className="text-5xl font-bold mb-10 tracking-tight">OpenAgents</h1>
        <div className="flex flex-col space-y-4 w-full max-w-xs">
          <Button onClick={onCreateWallet} className="w-full" size="lg">
            Create New Wallet
          </Button>
          <Button onClick={onEnterSeed} variant="outline" className="w-full" size="lg">
            Enter Seed Phrase
          </Button>
        </div>
      </div>
      <footer className="fixed bottom-6 text-center text-xs text-muted-foreground w-full px-4">
        Self-custody bitcoin wallet. 100%{" "}
        <a
          href="https://github.com/OpenAgentsInc/openagents/tree/main/apps/wallet"
          target="_blank"
          rel="noopener noreferrer"
          className="underline hover:text-primary inline-flex items-center gap-1"
        >
          open source <Github size={14} />
        </a>
      </footer>
    </div>
  );
};

export default LoginScreen;
```
