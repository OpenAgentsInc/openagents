import { useEffect, useRef, useCallback } from 'react'
import { SparkWallet, type Network as SparkNetwork, type TokenInfo } from '@buildonspark/spark-sdk'
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
import QRCode from 'react-qr-code';

// Screen Components
import LoginScreen from './components/LoginScreen';
import CreateWalletDisclaimerScreen from './components/CreateWalletDisclaimerScreen';
import ShowMnemonicScreen from './components/ShowMnemonicScreen';
import EnterSeedScreen from './components/EnterSeedScreen';

// State management
import { useWalletStore, WalletState } from './lib/store';

interface WalletInfo {
  balanceSat: bigint;
  tokenBalances?: Map<string, { balance: bigint, tokenInfo: TokenInfo }>;
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
    balanceSat: BigInt(0)
  });
  
  const [receiveAmount, setReceiveAmount] = useState(BigInt(100)); // Default 100 sats
  const [invoice, setInvoice] = useState('');
  const [isGeneratingInvoice, setIsGeneratingInvoice] = useState(false);
  const sdkRef = useRef<SparkWallet | null>(null);
  const initializedRef = useRef<boolean>(false); // To prevent double initialization in StrictMode

  const fetchWalletData = useCallback(async (sdk: SparkWallet) => {
    if (!sdk) return;
    
    try {
      const balanceData = await sdk.getBalance();
      setWalletInfo({
        balanceSat: balanceData.balance,
        tokenBalances: balanceData.tokenBalances
      });
    } catch (error) {
      console.error('Failed to fetch wallet data:', error);
      toast.error("Failed to fetch wallet data. Some features may be limited.");
    }
  }, []);

  const connectToSparkSDK = useCallback(async (seedPhrase: string) => {
    // Prevent double connection attempts
    if (sdkRef.current) {
      console.log("SDK already connected or connecting.");
      return;
    }
    
    setAppState('initializing_wallet');
    setErrorMessage(null);

    try {
      console.log("Initializing Spark SDK");
      
      // Create Spark wallet with mnemonic
      const { wallet: sparkInstance } = await SparkWallet.create({
        mnemonicOrSeed: seedPhrase,
        options: {
          network: "MAINNET" as SparkNetwork,
        }
      });
      
      console.log("Spark SDK connected successfully");
      sdkRef.current = sparkInstance;

      await fetchWalletData(sparkInstance);
      setAppState('wallet_ready');
      setMnemonic(seedPhrase); // Store in zustand
      toast.success("Spark Wallet Connected!");
    } catch (error) {
      console.error('Failed to initialize Spark SDK:', error);
      setErrorMessage(`Failed to initialize wallet: ${error instanceof Error ? error.message : String(error)}`);
      setAppState('error');
      sdkRef.current = null;
    }
  }, [fetchWalletData, setAppState, setErrorMessage, setMnemonic]);

  // Auto-login with stored mnemonic
  useEffect(() => {
    // Only initialize once - prevents double initialization in React StrictMode
    if (initializedRef.current) {
      return;
    }
    
    // If we have a mnemonic in the store, try to connect
    if (mnemonic && appState === 'login') {
      initializedRef.current = true;
      console.log("Initial SDK connection - first render only");
      connectToSparkSDK(mnemonic);
    }
  }, [mnemonic, appState, connectToSparkSDK]);

  const handleCreateWallet = () => {
    setAppState('creating_disclaimer');
  };

  const handleDisclaimerNext = () => {
    const newMnemonic = bip39.generateMnemonic(wordlist);
    setMnemonic(newMnemonic);
    setAppState('showing_mnemonic');
  };

  const handleMnemonicConfirmed = (seedPhrase: string) => {
    initializedRef.current = true; // Mark as initialized for this session
    connectToSparkSDK(seedPhrase);
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
    
    // Mark as initialized for this session
    initializedRef.current = true;
    
    // Connect to Spark with the validated seed phrase
    setMnemonic(seedPhrase);
    connectToSparkSDK(seedPhrase);
  };

  const handleLogout = async () => {
    // Clean up SDK resources (Spark doesn't have a disconnect method)
    sdkRef.current = null;
    
    // Reset UI state
    setInvoice('');
    setWalletInfo({
      balanceSat: BigInt(0)
    });
    
    // Reset store state (clears mnemonic from persistence)
    resetWallet();
    
    // Reset initialization flag to allow reinitializing after logout
    initializedRef.current = false;
    
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
      
      console.log("Generating invoice for amount:", receiveAmount.toString(), "sats");
      
      // Spark requires a number for amountSats
      const amountNumber = Number(receiveAmount);
      
      if (isNaN(amountNumber) || amountNumber <= 0) {
        toast.error("Invalid amount for invoice.");
        return;
      }
      
      // Generate invoice directly with Spark SDK
      const invoiceString = await sdkRef.current.createLightningInvoice({
        amountSats: amountNumber,
        memo: "OpenAgents Invoice" // Example memo
      });
      
      setInvoice(invoiceString);
      toast.dismiss("invoice-generation");
      toast.success("Spark Lightning Invoice Generated!");
    } catch (error) {
      console.error('Failed to generate invoice:', error);
      toast.dismiss("invoice-generation");
      
      // Handle different error types with more specific messages
      if (error instanceof Error) {
        if (error.message.includes('amount')) {
          toast.error("Amount issue: Try a different amount");
        } else if (error.message.includes('node') || error.message.includes('connect')) {
          toast.error("Lightning node connection issue. Network may be congested.");
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
            <p className="text-lg">Initializing Spark Wallet...</p>
            <p className="text-sm text-muted-foreground mt-2">This might take a moment</p>
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
                  <p className="text-xl font-bold">{formatSatsWithBitcoinSymbol(walletInfo.balanceSat)}</p>
                </div>
                
                {/* Token balances could be displayed here if needed */}
                {walletInfo.tokenBalances && walletInfo.tokenBalances.size > 0 && (
                  <div className="mt-4">
                    <h4 className="text-md font-medium mb-2">Token Balances:</h4>
                    {Array.from(walletInfo.tokenBalances.entries()).map(([tokenId, tokenData]) => (
                      <div key={tokenId} className="text-sm">
                        {tokenData.tokenInfo?.name || tokenId.substring(0,8)}: {tokenData.balance.toString()}
                      </div>
                    ))}
                  </div>
                )}
              </CardContent>
            </Card>

            <Card>
              <CardHeader>
                <CardTitle>Receive Payment (Lightning)</CardTitle>
                <CardDescription>Generate a Lightning invoice to receive funds via Spark</CardDescription>
              </CardHeader>
              <CardContent className="flex flex-col gap-4">
                <div className="space-y-2">
                  <label className="text-sm font-medium">Amount (sats)</label>
                  <UiInput
                    type="number"
                    value={receiveAmount.toString()}
                    onChange={(e) => setReceiveAmount(BigInt(e.target.value || "0"))}
                    min="1"
                    autoFocus
                  />
                </div>
                <UiButton
                  onClick={generateInvoice}
                  disabled={!sdkRef.current || isGeneratingInvoice || receiveAmount <= 0}
                  className="w-full"
                >
                  {isGeneratingInvoice ? (
                    <>
                      <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                      Generating...
                    </>
                  ) : (
                    "Generate Spark Invoice"
                  )}
                </UiButton>
                
                {invoice && (
                  <div className="mt-4 space-y-4">
                    <div className="flex justify-between items-center">
                      <h3 className="text-sm font-medium">Spark Lightning Invoice</h3>
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
                    
                    {/* QR Code */}
                    <div className="flex flex-col items-center bg-white p-4 rounded-md">
                      <QRCode
                        value={invoice}
                        size={200}
                        bgColor={"#FFFFFF"}
                        fgColor={"#000000"}
                        level={"M"}
                        className="mx-auto"
                      />
                      <p className="text-xs text-center mt-2 text-muted-foreground">
                        Scan with a Lightning wallet
                      </p>
                    </div>
                    
                    <div className="mt-2">
                      <p className="text-sm font-medium mb-1">Invoice Text</p>
                      <ScrollArea className="h-24 w-full rounded-md border p-2">
                        <div className="p-2 font-mono text-sm break-all">
                          {invoice}
                        </div>
                      </ScrollArea>
                    </div>
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