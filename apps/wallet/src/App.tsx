import { useEffect, useRef, useCallback } from 'react'
import { SparkWallet, type TokenInfo } from '@buildonspark/spark-sdk'

// Define transaction data interface
export interface SparkTransferData {
  id: string;
  createdTime?: string; // The field found in Spark logs
  updatedTime?: string;
  created_at_time?: string; // Original field names kept for compatibility
  updated_at_time?: string;
  created_at?: string; // Alternative date field
  timestamp?: string | number; // Alternative date field
  network: string;
  type: string;
  status: string;
  transfer_direction: "INCOMING" | "OUTGOING";
  transferDirection?: "INCOMING" | "OUTGOING"; // Alternative field name
  total_sent?: bigint; // Original expected field
  totalValue?: number | bigint; // The field found in Spark logs
  description?: string;
  fee?: bigint;
  senderIdentityPublicKey?: string; // Field name from logs
  receiverIdentityPublicKey?: string;
  sender_identity_public_key?: string; // Original field names kept for compatibility
  receiver_identity_public_key?: string;
  leaves?: any[]; // Additional field observed in logs
  amount?: number | bigint; // Additional field for transaction amount
  amountSat?: number | bigint; // Additional field for transaction amount
  invoice?: {
    amount?: {
      amountSat?: number | bigint;
    }
  }; // Support for invoice object with nested amount
}
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
import { Loader2, Key } from 'lucide-react';
import QRCode from 'react-qr-code';
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
} from "@/components/ui/alert-dialog";

// Screen Components
import LoginScreen from './components/LoginScreen';
import CreateWalletDisclaimerScreen from './components/CreateWalletDisclaimerScreen';
import ShowMnemonicScreen from './components/ShowMnemonicScreen';
import EnterSeedScreen from './components/EnterSeedScreen';
import TransactionHistoryCard from './components/TransactionHistoryCard';

// State management
import { useWalletStore } from './lib/store';

// Extended TokenInfo to ensure it has a name property
interface ExtendedTokenInfo extends Partial<TokenInfo> {
  name?: string;
}

interface WalletInfo {
  balanceSat: bigint;
  tokenBalances?: Map<string, { balance: bigint, tokenInfo: ExtendedTokenInfo }>;
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

  const [receiveAmount, setReceiveAmount] = useState(BigInt(10)); // Default 10 sats
  const [invoice, setInvoice] = useState('');
  const [isGeneratingInvoice, setIsGeneratingInvoice] = useState(false);
  const [transactions, setTransactions] = useState<SparkTransferData[]>([]);
  const sdkRef = useRef<any>(null);
  const initializedRef = useRef<boolean>(false); // To prevent double initialization in StrictMode

  const fetchWalletData = useCallback(async (sdk: any) => {
    if (!sdk) return;

    try {
      // Try accessing the wallet property if initialize returns an object with wallet
      const wallet = sdk.wallet || sdk;
      
      // Fetch balance
      const balanceData = await wallet.getBalance();

      setWalletInfo({
        balanceSat: balanceData.balance || BigInt(0),
        tokenBalances: balanceData.tokenBalances
      });

      // Fetch transactions
      try {
        const transfersResponse = await wallet.getTransfers(20, 0); // Fetch 20 transactions, offset 0
        
        if (transfersResponse && transfersResponse.transfers) {
          // Sort by createdTime descending (newest first)
          const sortedTransactions = transfersResponse.transfers.sort(
            (a: SparkTransferData, b: SparkTransferData) => {
              const dateA = a.createdTime ? new Date(a.createdTime).getTime() : 0;
              const dateB = b.createdTime ? new Date(b.createdTime).getTime() : 0;
              return dateB - dateA;
            }
          );
          
          setTransactions(sortedTransactions);
        } else {
          setTransactions([]);
        }
      } catch (txError) {
        console.error('Failed to fetch transactions:', txError);
        setTransactions([]);
      }
    } catch (error) {
      console.error('Failed to fetch wallet data:', error);
      toast.error("Failed to fetch wallet data. Some features may be limited.");
    }
  }, []);

  const connectToSparkSDK = useCallback(async (seedPhrase: string) => {
    // Prevent double connection attempts
    if (sdkRef.current) {
      return;
    }

    setAppState('initializing_wallet');
    setErrorMessage(null);

    try {
      // Initialize Spark wallet with mnemonic
      const { wallet: sparkInstance } = await SparkWallet.initialize({
        mnemonicOrSeed: seedPhrase,
        options: {
          network: "MAINNET",
        }
      });

      sdkRef.current = sparkInstance;

      await fetchWalletData(sparkInstance);
      setAppState('wallet_ready');
      setMnemonic(seedPhrase); // Store in zustand
      toast.success("Wallet Connected!");
    } catch (error) {
      console.error('Failed to initialize wallet:', error);
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

      // Spark requires a number for amountSats
      const amountNumber = Number(receiveAmount);

      if (isNaN(amountNumber) || amountNumber <= 0) {
        toast.error("Invalid amount for invoice.");
        setIsGeneratingInvoice(false);
        toast.dismiss("invoice-generation");
        return;
      }

      // Generate invoice directly with Spark SDK
      const wallet = sdkRef.current.wallet || sdkRef.current;
      
      const invoiceResponse = await wallet.createLightningInvoice({
        amountSats: amountNumber,
        memo: "OpenAgents Invoice" // Example memo
      });
      
      // Extract the encoded invoice string from the response object
      const encodedInvoice = invoiceResponse?.invoice?.encodedInvoice;
      
      if (!encodedInvoice) {
        throw new Error("Failed to get encoded invoice from response");
      }
      
      setInvoice(encodedInvoice);
      toast.dismiss("invoice-generation");
      toast.success("Lightning Invoice Generated!");
      
      // Refresh wallet data to potentially update any pending transactions
      if (sdkRef.current) {
        fetchWalletData(sdkRef.current);
      }
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

  // Format functions are now implemented directly in the JSX

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
                <AlertDialog>
                  <AlertDialogTrigger asChild>
                    <UiButton variant="outline" size="icon">
                      <Key className="h-4 w-4" />
                    </UiButton>
                  </AlertDialogTrigger>
                  <AlertDialogContent>
                    <AlertDialogHeader>
                      <AlertDialogTitle>Your Seed Phrase</AlertDialogTitle>
                      <AlertDialogDescription>
                        Keep this phrase safe. Anyone with access to it can control your wallet.
                      </AlertDialogDescription>
                    </AlertDialogHeader>
                    <div className="mt-2 text-center">
                      <div className="p-4 bg-muted rounded-md font-mono text-sm whitespace-normal break-words">
                        {mnemonic}
                      </div>
                    </div>
                    <AlertDialogFooter>
                      <AlertDialogCancel>Close</AlertDialogCancel>
                      <AlertDialogAction onClick={() => {
                        navigator.clipboard.writeText(mnemonic || "");
                        toast.success("Seed phrase copied to clipboard");
                      }}>
                        Copy
                      </AlertDialogAction>
                    </AlertDialogFooter>
                  </AlertDialogContent>
                </AlertDialog>
                <ModeToggle />
                <UiButton variant="outline" size="sm" onClick={handleLogout}>Logout</UiButton>
              </div>
            </div>
            <Card className="mb-6">
              <CardHeader>
                <CardTitle>Bitcoin Balance</CardTitle>
              </CardHeader>
              <CardContent>
                <div className="flex flex-col items-center">
                  <div className="relative">
                    <p className="text-3xl font-bold text-center">₿ {walletInfo.balanceSat.toString()}</p>
                    <span className="absolute bottom-0 right-0 text-xs text-muted-foreground translate-y-full -translate-x-3">sats</span>
                  </div>
                </div>

                {/* Token balances could be displayed here if needed */}
                {walletInfo.tokenBalances && walletInfo.tokenBalances.size > 0 && (
                  <div className="mt-4">
                    <h4 className="text-md font-medium mb-2">Token Balances:</h4>
                    {Array.from(walletInfo.tokenBalances.entries()).map(([tokenId, tokenData]) => (
                      <div key={tokenId} className="text-sm">
                        {tokenData.tokenInfo?.name ?? tokenId.substring(0, 8)}: {tokenData.balance.toString()}
                      </div>
                    ))}
                  </div>
                )}
              </CardContent>
            </Card>

            <Card>
              <CardHeader>
                <CardTitle>Receive Bitcoin (Lightning)</CardTitle>
                <CardDescription>Generate a Lightning invoice to receive Bitcoin</CardDescription>
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
                <div className="flex justify-center">
                  <UiButton
                    onClick={generateInvoice}
                    disabled={!sdkRef.current || isGeneratingInvoice || receiveAmount <= 0}
                  >
                    {isGeneratingInvoice ? (
                      <>
                        <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                        Generating...
                      </>
                    ) : (
                      "Generate Lightning Invoice"
                    )}
                  </UiButton>
                </div>

                {invoice && (
                  <div className="mt-4 space-y-4">
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

            {/* Transaction History Card */}
            <TransactionHistoryCard transactions={transactions} />

            <div className="h-16"/> {/* Spacer for scroll */}
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
