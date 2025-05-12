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

  // Handle navigation back to login screen
  const handleBackToLogin = () => {
    setAppState('login');
  };
  
  // Render different screens based on appState
  const renderContent = () => {
    switch (appState) {
      case 'login':
        return <LoginScreen onCreateWallet={handleCreateWallet} onEnterSeed={handleEnterSeed} />;
      case 'creating_disclaimer':
        return <CreateWalletDisclaimerScreen onNext={handleDisclaimerNext} onBack={handleBackToLogin} />;
      case 'showing_mnemonic':
        if (!currentMnemonic) return <p>Error: Mnemonic not generated.</p>;
        return <ShowMnemonicScreen mnemonic={currentMnemonic} onNext={() => handleMnemonicConfirmed(currentMnemonic)} onBack={handleBackToLogin} />;
      case 'entering_seed':
        return <EnterSeedScreen onSeedEntered={handleSeedEntered} onBack={handleBackToLogin} />;
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