import { useEffect, useRef, useCallback } from 'react'
import init, { defaultConfig, connect, BindingLiquidSdk, type WalletInfo as SdkWalletInfo, type LightningPaymentLimitsResponse } from '@breeztech/breez-sdk-liquid'
import * as bip39 from '@scure/bip39';
import { wordlist } from '@scure/bip39/wordlists/english';
import { useState } from 'react';

// Shadcn UI components
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from '@/components/ui/card'
import { Input as UiInput } from '@/components/ui/input'
import { Button as UiButton } from '@/components/ui/button'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Toaster } from '@/components/ui/sonner'
import { toast } from 'sonner'
import { ModeToggle } from '@/components/mode-toggle'
import { Loader2 } from 'lucide-react';

// Screen Components
import LoginScreen from './components/LoginScreen';
import CreateWalletDisclaimerScreen from './components/CreateWalletDisclaimerScreen';
import ShowMnemonicScreen from './components/ShowMnemonicScreen';
import EnterSeedScreen from './components/EnterSeedScreen';

// State management
import { useWalletStore, WalletState } from './lib/store';

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
  // Zustand store for app state
  const { 
    appState, 
    setAppState, 
    mnemonic, 
    setMnemonic, 
    errorMessage, 
    setErrorMessage,
    resetWallet
  } = useWalletStore();

  // Wallet UI state (not persisted)
  const [walletInfo, setWalletInfo] = useState<WalletInfo>({
    balanceSat: BigInt(0),
    pendingSendSat: BigInt(0),
    pendingReceiveSat: BigInt(0)
  });
  const [lightningLimits, setLightningLimits] = useState<LightningLimits>({
    min: BigInt(0),
    max: BigInt(0)
  });
  const [receiveAmount, setReceiveAmount] = useState(BigInt(100)); // Default 100 sats
  const [invoice, setInvoice] = useState('');
  const [fees, setFees] = useState(BigInt(0));
  const [isGeneratingInvoice, setIsGeneratingInvoice] = useState(false);
  const sdkRef = useRef<BindingLiquidSdk | null>(null);
  const listenerIdRef = useRef<string | null>(null);

  const fetchWalletData = useCallback(async (sdk: BindingLiquidSdk) => {
    if (!sdk) return;
    
    try {
      const info = await sdk.getInfo();
      setWalletInfo({
        balanceSat: info.walletInfo.balanceSat,
        pendingSendSat: info.walletInfo.pendingSendSat,
        pendingReceiveSat: info.walletInfo.pendingReceiveSat
      });

      try {
        // Separate try-catch for lightning limits which might fail
        const limits: LightningPaymentLimitsResponse = await sdk.fetchLightningLimits();
        setLightningLimits({
          min: limits.receive.minSat,
          max: limits.receive.maxSat
        });
      } catch (limitsError) {
        console.warn('Failed to fetch lightning limits:', limitsError);
        // Set default limits if the API call fails
        setLightningLimits({
          min: BigInt(1000),
          max: BigInt(100000)
        });
      }
    } catch (error) {
      console.error('Failed to fetch wallet data:', error);
      toast.error("Failed to fetch wallet data. Some features may be limited.");
    }
  }, []);

  const connectToBreez = useCallback(async (seedPhrase: string) => {
    if (sdkRef.current) {
      console.log("SDK already connected or connecting.");
      return;
    }
    
    setAppState('initializing_wallet');
    setErrorMessage(null);

    try {
      await init(); // Initialize WASM

      // Create configuration with API key
      const config = defaultConfig('mainnet', import.meta.env.VITE_BREEZ_API_KEY);
      
      // Connect to Breez SDK with the provided mnemonic
      const sdk = await connect({ mnemonic: seedPhrase, config });
      sdkRef.current = sdk;

      // Set up event listener for SDK events
      const eventListener = {
        onEvent: (event: any) => { // Use proper type if available
          console.log('Breez SDK Event:', event.type, event);
          if (['synced', 'paymentSucceeded', 'paymentFailed', 'paymentPending'].includes(event.type)) {
            fetchWalletData(sdk);
          }
        }
      };
      
      try {
        const listenerId = await sdk.addEventListener(eventListener);
        listenerIdRef.current = listenerId;
      } catch (listenerError) {
        console.warn('Failed to add event listener:', listenerError);
        // Continue without the listener
      }

      await fetchWalletData(sdk);
      setAppState('wallet_ready');
      setMnemonic(seedPhrase); // Store in zustand
    } catch (error) {
      console.error('Failed to initialize Breez SDK:', error);
      setErrorMessage(`Failed to initialize wallet: ${error instanceof Error ? error.message : String(error)}`);
      setAppState('error');
      sdkRef.current = null;
    }
  }, [fetchWalletData, setAppState, setErrorMessage, setMnemonic]);

  // Auto-login with stored mnemonic
  useEffect(() => {
    // If we have a mnemonic in the store, try to connect
    if (mnemonic && appState === 'login') {
      connectToBreez(mnemonic);
    }
  }, [mnemonic, appState, connectToBreez]);

  const handleCreateWallet = () => {
    setAppState('creating_disclaimer');
  };

  const handleDisclaimerNext = () => {
    const newMnemonic = bip39.generateMnemonic(wordlist);
    setMnemonic(newMnemonic);
    setAppState('showing_mnemonic');
  };

  const handleMnemonicConfirmed = (seedPhrase: string) => {
    connectToBreez(seedPhrase);
  };

  const handleEnterSeed = () => {
    setAppState('entering_seed');
  };

  const handleSeedEntered = (seedPhrase: string) => {
    // Validate the seed phrase
    if (!bip39.validateMnemonic(seedPhrase, wordlist)) {
      toast.error("Invalid seed phrase. Please check and try again.");
      setErrorMessage("Invalid seed phrase format.");
      return;
    }
    
    // Connect to Breez with the validated seed phrase
    setMnemonic(seedPhrase);
    connectToBreez(seedPhrase);
  };

  const handleLogout = async () => {
    // Clean up SDK resources
    if (sdkRef.current) {
      try {
        // Remove event listener if it exists
        if (listenerIdRef.current) {
          await sdkRef.current.removeEventListener(listenerIdRef.current);
          listenerIdRef.current = null;
        }
        
        // Disconnect the SDK
        await sdkRef.current.disconnect();
        sdkRef.current = null;
      } catch (e) {
        console.error("Error during SDK cleanup:", e);
      }
    }
    
    // Reset UI state
    setInvoice('');
    setFees(BigInt(0));
    setWalletInfo({
      balanceSat: BigInt(0),
      pendingSendSat: BigInt(0),
      pendingReceiveSat: BigInt(0)
    });
    
    // Reset store state (clears mnemonic from persistence)
    resetWallet();
    
    toast.info("Logged out successfully.");
  };

  const generateInvoice = async () => {
    if (!sdkRef.current) {
      toast.error("Wallet not connected");
      return;
    }
    
    // Prevent multiple clicks
    if (isGeneratingInvoice) {
      return;
    }
    
    setIsGeneratingInvoice(true);
    
    try {
      // Show loading toast
      toast.loading("Generating invoice...", { id: "invoice-generation" });
      
      // For demo purposes, add a small delay to simulate network latency
      // This helps prevent rapid consecutive calls that might trigger the WASM error
      await new Promise(resolve => setTimeout(resolve, 500));
      
      // Define the payment amount
      const optionalAmount = {
        type: 'bitcoin',
        payerAmountSat: receiveAmount
      };

      // First prepare the payment
      let prepareResponse;
      try {
        prepareResponse = await sdkRef.current.prepareReceivePayment({
          paymentMethod: 'lightning',
          amount: optionalAmount
        });
      } catch (prepareError) {
        console.error('Failed to prepare payment:', prepareError);
        toast.dismiss("invoice-generation");
        toast.error("Failed to prepare payment. Please try a different amount.");
        
        // If this is likely a connection or network issue, suggest reconnecting
        if (prepareError instanceof Error && 
            (prepareError.message.includes("network") || 
             prepareError.message.includes("connect") ||
             prepareError.message.includes("timeout"))) {
          toast.error("Network connection issue. Try logging out and back in.");
        }
        return;
      }

      // Update the fees display
      setFees(prepareResponse.feesSat);

      // Then generate the invoice
      try {
        const receiveResponse = await sdkRef.current.receivePayment({
          prepareResponse
        });

        if (receiveResponse?.destination) {
          setInvoice(receiveResponse.destination);
          toast.dismiss("invoice-generation");
          toast.success("Invoice generated successfully!");
        } else {
          throw new Error("No destination in response");
        }
      } catch (receiveError) {
        console.error('Failed in receivePayment step:', receiveError);
        toast.dismiss("invoice-generation");
        toast.error("Invoice generation failed at final step");
        return;
      }
    } catch (error) {
      console.error('Failed to generate invoice (outer catch):', error);
      toast.dismiss("invoice-generation");
      
      // Handle different error types with more specific messages
      if (error instanceof Error) {
        if (error.message.includes('amount') || error.message.includes('limits')) {
          toast.error("Amount issue: Try a different amount within the allowed limits");
        } else if (error.message.includes('node') || error.message.includes('connect')) {
          toast.error("Lightning node connection issue. Network may be congested.");
        } else if (error.message.includes('unwrap') || error.message.includes('Err value')) {
          // This is likely the WASM error you're seeing
          toast.error("SDK internal error. Try logging out and back in.");
          
          // Attempt recovery by cleaning up SDK resources
          try {
            if (sdkRef.current && listenerIdRef.current) {
              sdkRef.current.removeEventListener(listenerIdRef.current);
            }
            if (sdkRef.current) {
              // Don't actually disconnect as that might lose wallet data
              // but clear any pending operations
              await fetchWalletData(sdkRef.current);
            }
          } catch (cleanupError) {
            console.error("Failed cleanup after error:", cleanupError);
          }
        } else {
          toast.error(`Error: ${error.message}`);
        }
      } else {
        toast.error("Unknown error occurred");
      }
    } finally {
      // Always reset loading state
      setIsGeneratingInvoice(false);
    }
  };

  // Helper function to format satoshis with the bitcoin symbol
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
        if (!mnemonic) return <p>Error: Mnemonic not generated.</p>;
        return <ShowMnemonicScreen mnemonic={mnemonic} onNext={() => handleMnemonicConfirmed(mnemonic)} onBack={handleBackToLogin} />;
      case 'entering_seed':
        return <EnterSeedScreen onSeedEntered={handleSeedEntered} onBack={handleBackToLogin} />;
      case 'initializing_wallet':
        return (
          <div className="flex flex-col items-center justify-center min-h-screen">
            <Loader2 className="h-10 w-10 animate-spin text-primary mb-4" />
            <p className="text-lg">Initializing Wallet...</p>
            <p className="text-sm text-muted-foreground mt-2">This might take a moment</p>
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
                    autoFocus
                  />
                  <p className="text-sm text-muted-foreground">
                    Min: {lightningLimits.min.toString()} sats, Max: {lightningLimits.max.toString()} sats
                  </p>
                </div>
                <UiButton
                  onClick={generateInvoice}
                  disabled={!sdkRef.current || isGeneratingInvoice || receiveAmount < lightningLimits.min || receiveAmount > lightningLimits.max}
                  className="w-full"
                >
                  {isGeneratingInvoice ? (
                    <>
                      <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                      Generating...
                    </>
                  ) : (
                    "Generate Invoice"
                  )}
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
          <div className="flex flex-col items-center justify-center min-h-screen p-4">
            <div className="text-destructive mb-4">
              <svg xmlns="http://www.w3.org/2000/svg" width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className="mx-auto mb-4">
                <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"></path>
                <line x1="12" y1="9" x2="12" y2="13"></line>
                <line x1="12" y1="17" x2="12.01" y2="17"></line>
              </svg>
              <h2 className="text-2xl font-semibold text-center">Wallet Error</h2>
            </div>
            <p className="text-muted-foreground mb-6 text-center max-w-md">
              {errorMessage || "An unexpected error occurred while initializing the wallet."}
            </p>
            <UiButton onClick={handleBackToLogin}>Return to Login</UiButton>
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