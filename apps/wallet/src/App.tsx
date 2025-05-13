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
import { Alert, AlertTitle, AlertDescription } from '@/components/ui/alert'
import { Toaster } from '@/components/ui/sonner'
import { toast } from 'sonner'
import { ModeToggle } from '@/components/mode-toggle'
import { Loader2, Key, AlertCircle, X } from 'lucide-react';
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
import SendPaymentCard from './components/SendPaymentCard';
import SendSparkPaymentCard from './components/SendSparkPaymentCard';

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
  const [sendInvoice, setSendInvoice] = useState('');
  const [isSendingPayment, setIsSendingPayment] = useState(false);
  const [showBetaAlert, setShowBetaAlert] = useState(true);
  const [userSparkAddress, setUserSparkAddress] = useState<string>('');
  const [recipientSparkAddress, setRecipientSparkAddress] = useState('');
  const [sendSparkAmount, setSendSparkAmount] = useState(BigInt(0));
  const [isSendingSparkPayment, setIsSendingSparkPayment] = useState(false);
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

      // Fetch user's Spark Address
      try {
        const sparkAddr = await wallet.getSparkAddress();
        setUserSparkAddress(sparkAddr);
      } catch (addrError) {
        console.error('Failed to fetch Spark address:', addrError);
        toast.error("Could not fetch your Spark address.");
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
      const wallet = sdkRef.current;

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

  const handlePayInvoice = async () => {
    if (!sdkRef.current) {
      toast.error("Wallet not connected.");
      return;
    }
    if (!sendInvoice.trim()) {
      toast.error("Please enter a Lightning invoice to pay.");
      return;
    }
    if (isSendingPayment) return;

    setIsSendingPayment(true);
    toast.loading("Processing payment...", { id: "pay-invoice" });

    try {
      // sdkRef.current is the wallet instance from SparkWallet.initialize
      const wallet = sdkRef.current;

      // The Spark SDK's payLightningInvoice takes the BOLT11 string directly.
      // No separate decode step is strictly necessary for just paying,
      // but you might want to decode it for UI display (amount, memo) before sending.
      // For this step, we'll just pay directly.

      const trimmedInvoice = sendInvoice.trim();
      console.log("Attempting to pay invoice:", trimmedInvoice);

      // Log available methods for debugging
      console.log("Available wallet methods:",
        Object.getOwnPropertyNames(Object.getPrototypeOf(wallet))
          .filter(method => typeof wallet[method] === 'function')
      );

      // Based on the actual SDK error logs, we need a different approach
      // The error "Lightning Payment Request must be string" suggests that
      // the SDK expects only the string and not an object

      // For safety, let's check if our invoice has features that might be
      // causing issues (whitespace, non-ASCII characters, etc.)
      let cleanedInvoice = trimmedInvoice
        .replace(/\s+/g, '')  // Remove any whitespace
        .replace(/[\u0000-\u001F\u007F-\u009F]/g, ''); // Remove control characters

      // Make sure the string starts with ln (case insensitive)
      if (!cleanedInvoice.toLowerCase().startsWith('ln')) {
        throw new Error("Invalid Lightning invoice format. Must start with 'ln'.");
      }

      console.log("Using cleaned invoice:", cleanedInvoice);

      // Based on the available methods in the logs, we'll try the direct approach
      try {
        // The logs show the SDK is expecting a direct string parameter
        await wallet.payLightningInvoice(cleanedInvoice);
      } catch (err) {
        console.error("Direct payment failed:", err);

        // If that fails, try with an object (some SDK versions expect this)
        try {
          await wallet.payLightningInvoice({
            invoice: cleanedInvoice
          });
        } catch (objErr) {
          console.error("Object payment failed:", objErr);

          // Last resort: try to use getLightningSendRequest if available
          if (typeof wallet.getLightningSendRequest === 'function') {
            const sendRequest = await wallet.getLightningSendRequest({
              invoice: cleanedInvoice
            });
            console.log("Generated send request:", sendRequest);

            // Execute the generated request
            if (sendRequest && typeof sendRequest === 'string') {
              await wallet.payLightningInvoice(sendRequest);
            } else {
              // If we got a request object, try to use that
              throw new Error("Could not process Lightning invoice. Please try a different invoice.");
            }
          } else {
            // If everything failed, rethrow the original error
            throw err;
          }
        }
      }

      console.log("Payment successful");

      toast.dismiss("pay-invoice");
      toast.success("Payment Sent Successfully!");
      setSendInvoice(''); // Clear the input field

      // Re-fetch wallet data to update balance and transaction history
      await fetchWalletData(sdkRef.current);

    } catch (error) {
      console.error('Failed to pay invoice:', error);
      toast.dismiss("pay-invoice");
      const message = error instanceof Error ? error.message : String(error);

      console.log("Payment error details:", {
        error,
        message,
        sdkReady: !!sdkRef.current,
        invoiceLength: sendInvoice.length
      });

      // More specific error messages
      if (message.toLowerCase().includes("insufficient balance") || message.toLowerCase().includes("not enough funds")) {
        toast.error("Payment Failed: Insufficient balance.", { description: "Please check your balance and try again." });
      } else if (message.toLowerCase().includes("invalid invoice") || message.toLowerCase().includes("decode error") || message.toLowerCase().includes("malformed")) {
        toast.error("Payment Failed: Invalid invoice.", { description: "Please check the invoice string and try again." });
      } else if (message.toLowerCase().includes("route") || message.toLowerCase().includes("path not found") || message.toLowerCase().includes("no route")) {
        toast.error("Payment Failed: No route found.", { description: "Could not find a path to the destination. The recipient might be offline or there might be network issues." });
      } else if (message.toLowerCase().includes("timeout") || message.toLowerCase().includes("timed out")) {
        toast.error("Payment Failed: Timeout", { description: "The payment request timed out. Please try again." });
      } else if (message.toLowerCase().includes("invoice must be") || message.toLowerCase().includes("lightning payment request")) {
        toast.error("Payment Failed: Format issue", { description: "The invoice format was not recognized. Please check the invoice and try again." });
      } else if (message.toLowerCase().includes("invalid amount") || message.toLowerCase().includes("amount")) {
        toast.error("Payment Failed: Amount issue", { description: "The payment amount in the invoice is invalid or not specified. Try a different invoice." });
      } else if (message.toLowerCase().includes("not a function") || message.toLowerCase().includes("undefined")) {
        toast.error("Payment Failed: SDK error", { description: "There was an issue with the wallet SDK. Please restart the wallet or try again later." });
      } else if (message.toLowerCase().includes("network") || message.toLowerCase().includes("failed to execute") || message.toLowerCase().includes("graphql")) {
        toast.error("Payment Failed: Network Error", {
          description: "Unable to connect to the Lightning Network. Please check your internet connection and try again later."
        });
      } else {
        toast.error("Payment Failed", { description: message });
      }
    } finally {
      setIsSendingPayment(false);
    }
  };

  const handleSendSparkPayment = async () => {
    if (!sdkRef.current) {
      toast.error("Wallet not connected.");
      return;
    }
    const trimmedAddress = recipientSparkAddress.trim();
    if (!trimmedAddress) {
      toast.error("Please enter a recipient's Spark address.");
      return;
    }
    
    // Check that the address has the right format
    if (!(trimmedAddress.startsWith("sp1p") || trimmedAddress.startsWith("sprt1p"))) {
      toast.error("Invalid Spark address format. Should start with 'sp1p'.");
      return;
    }
    if (sendSparkAmount <= BigInt(0)) {
      toast.error("Please enter a valid amount greater than 0.");
      return;
    }
    if (isSendingSparkPayment) return;

    setIsSendingSparkPayment(true);
    toast.loading("Sending Spark payment...", { id: "send-spark-payment" });

    try {
      const wallet = sdkRef.current;
      const amountSatsNumber = Number(sendSparkAmount); // SDK expects number

      console.log("Attempting to send Spark payment to:", trimmedAddress, "Amount:", amountSatsNumber);
      console.log("SDK reference type:", typeof sdkRef.current, "Available methods:", 
        Object.getOwnPropertyNames(Object.getPrototypeOf(sdkRef.current))
          .filter(method => typeof sdkRef.current[method] === 'function'));

      // The transfer method in Spark SDK is direct.
      // It might return a Transfer object upon successful initiation.
      const transferResult = await wallet.transfer({
        receiverSparkAddress: trimmedAddress,
        amountSats: amountSatsNumber,
      });

      console.log("Spark transfer initiated:", transferResult); // transferResult might be void or basic info

      toast.dismiss("send-spark-payment");
      toast.success("Spark Payment Sent Successfully!");
      setRecipientSparkAddress(''); // Clear recipient address
      setSendSparkAmount(BigInt(0)); // Clear amount

      // Re-fetch wallet data to update balance and transaction history
      await fetchWalletData(sdkRef.current);

    } catch (error) {
      console.error('Failed to send Spark payment:', error);
      toast.dismiss("send-spark-payment");
      const message = error instanceof Error ? error.message : "Unknown error during Spark payment.";

      if (message.toLowerCase().includes("insufficient balance")) {
        toast.error("Payment Failed: Insufficient balance.");
      } else if (message.toLowerCase().includes("invalid address") || message.toLowerCase().includes("receiver address")) {
        toast.error("Payment Failed: Invalid recipient Spark address.");
      } else if (message.toLowerCase().includes("amount")) {
        toast.error("Payment Failed: Invalid amount.");
      } else {
        toast.error("Spark Payment Failed", { description: message });
      }
    } finally {
      setIsSendingSparkPayment(false);
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
                <AlertDialog>
                  <AlertDialogTrigger asChild>
                    <UiButton variant="outline" size="sm">Logout</UiButton>
                  </AlertDialogTrigger>
                  <AlertDialogContent>
                    <AlertDialogHeader>
                      <AlertDialogTitle>Are you sure you want to logout?</AlertDialogTitle>
                      <AlertDialogDescription className="text-left">
                        <p className="mb-2">If you haven't backed up your seed phrase, you won't be able to access your funds.</p>
                        <div className="bg-amber-100 dark:bg-amber-950 p-3 rounded-md border border-amber-300 dark:border-amber-800 mt-2">
                          <p className="text-amber-800 dark:text-amber-200 font-medium">Make sure you've saved your seed phrase before logging out!</p>
                        </div>
                      </AlertDialogDescription>
                    </AlertDialogHeader>
                    <AlertDialogFooter>
                      <AlertDialogCancel>Cancel</AlertDialogCancel>
                      <AlertDialogAction
                        onClick={handleLogout}
                        className="bg-destructive text-destructive-foreground hover:bg-destructive/90"
                      >
                        Logout Anyway
                      </AlertDialogAction>
                    </AlertDialogFooter>
                  </AlertDialogContent>
                </AlertDialog>
              </div>
            </div>

            {showBetaAlert && (
              <div className="mb-4 relative">
                <Alert className="bg-amber-50 border-amber-200 dark:bg-amber-950 dark:border-amber-800 text-amber-800 dark:text-amber-200 pr-10">
                  <AlertCircle className="h-4 w-4" />
                  <AlertTitle>OpenAgents Wallet is in beta</AlertTitle>
                  <AlertDescription>
                    Don't use it with anything more than small amounts you'd be willing to lose. Withdraw regularly to a different wallet.
                  </AlertDescription>
                  <button
                    onClick={() => setShowBetaAlert(false)}
                    className="absolute top-3 right-3 p-1 rounded-sm opacity-70 ring-offset-background transition-opacity hover:opacity-100 focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2"
                    aria-label="Close"
                  >
                    <X className="h-4 w-4" />
                  </button>
                </Alert>
              </div>
            )}

            <Card className="mb-6">
              <CardHeader>
                <CardTitle>Bitcoin Balance</CardTitle>
              </CardHeader>
              <CardContent>
                <div className="flex flex-col items-center">
                  <div className="relative mb-6">
                    <p className="text-3xl font-bold text-center">₿<span className="mx-[1px]">{walletInfo.balanceSat.toString()}</span></p>
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

            {/* Spark Address Card - Now positioned right under balance */}
            <Card className="mt-6 mb-6">
              <CardHeader>
                <CardTitle>Your Spark Address</CardTitle>
                <CardDescription>Share this address to receive Spark payments.</CardDescription>
              </CardHeader>
              <CardContent>
                {userSparkAddress ? (
                  <div className="space-y-2">
                    <ScrollArea className="h-16 w-full rounded-md border p-2">
                      <div className="p-1 font-mono text-sm break-all">
                        {userSparkAddress}
                      </div>
                    </ScrollArea>
                    <UiButton
                      variant="outline"
                      size="sm"
                      className="w-full"
                      onClick={() => {
                        navigator.clipboard.writeText(userSparkAddress);
                        toast.success("Spark Address Copied!");
                      }}
                    >
                      Copy Address
                    </UiButton>
                  </div>
                ) : (
                  <p className="text-sm text-muted-foreground">Fetching your Spark address...</p>
                )}
              </CardContent>
            </Card>

            {/* Send Spark Payment Card - Moved to be under Your Spark Address */}
            <SendSparkPaymentCard
              recipientSparkAddress={recipientSparkAddress}
              setRecipientSparkAddress={setRecipientSparkAddress}
              sendSparkAmount={sendSparkAmount}
              setSendSparkAmount={setSendSparkAmount}
              handleSendSparkPayment={handleSendSparkPayment}
              isSendingSparkPayment={isSendingSparkPayment}
              disabled={!sdkRef.current}
            />

            <Card className="mt-6">
              <CardHeader>
                <CardTitle>Receive Bitcoin (Lightning)</CardTitle>
                <CardDescription>Generate a Lightning invoice to receive Bitcoin</CardDescription>
              </CardHeader>
              <CardContent className="flex flex-col gap-4">
                <div className="flex items-center gap-3">
                  <div className="w-1/3">
                    <label className="text-sm font-medium block mb-1">Amount (sats)</label>
                    <UiInput
                      type="number"
                      value={receiveAmount.toString()}
                      onChange={(e) => setReceiveAmount(BigInt(e.target.value || "0"))}
                      min="1"
                      className="w-full"
                    />
                  </div>
                  <div className="flex-1">
                    <UiButton
                      onClick={generateInvoice}
                      disabled={!sdkRef.current || isGeneratingInvoice || receiveAmount <= 0}
                      className="h-10 mt-6"
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
                </div>

                {invoice && (
                  <div className="mt-4 space-y-4">
                    <h3 className="text-sm font-medium text-center">Lightning Invoice</h3>

                    {/* QR Code */}
                    <div className="flex flex-col items-center bg-card p-4 rounded-md">
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
                      <div className="flex justify-between items-center mb-1">
                        <p className="text-sm font-medium">BOLT11 Invoice</p>
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
                  </div>
                )}
              </CardContent>
            </Card>

            {/* Send Payment Card */}
            <SendPaymentCard
              sendInvoice={sendInvoice}
              setSendInvoice={setSendInvoice}
              handlePayInvoice={handlePayInvoice}
              isSendingPayment={isSendingPayment}
              disabled={!sdkRef.current}
            />

            {/* Transaction History Card */}
            <TransactionHistoryCard transactions={transactions} />

            <div className="h-16" /> {/* Spacer for scroll */}
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
