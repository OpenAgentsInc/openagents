import { useEffect, useState, useRef } from 'react'
import init, { defaultConfig, connect, ReceiveAmount, BindingLiquidSdk } from '@breeztech/breez-sdk-liquid'
import * as bip39 from '@scure/bip39';
import { wordlist } from '@scure/bip39/wordlists/english';
import { Card, CardContent, CardHeader, CardTitle, CardDescription, CardFooter } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Button } from '@/components/ui/button'
import { Alert, AlertDescription } from '@/components/ui/alert'
import { Badge } from '@/components/ui/badge'
import { ScrollArea } from '@/components/ui/scroll-area'
import {
  AlertDialog,
  AlertDialogContent,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogAction
} from '@/components/ui/alert-dialog'

function App() {
  const [isInitialized, setIsInitialized] = useState(false)
  const [walletInfo, setWalletInfo] = useState({
    balanceSat: 0,
    pendingSendSat: 0,
    pendingReceiveSat: 0
  })
  const [lightningLimits, setLightningLimits] = useState({
    min: 0,
    max: 0
  })
  const [receiveAmount, setReceiveAmount] = useState(100)
  const [invoice, setInvoice] = useState('')
  const [fees, setFees] = useState(0)
  const [showCopyDialog, setShowCopyDialog] = useState(false)
  const sdkRef = useRef<BindingLiquidSdk | null>(null)
  const initializationRef = useRef(false)

  const connectToBreez = async () => {
    if (isInitialized || initializationRef.current) return;
    initializationRef.current = true;

    try {
      // Generate x random words. Uses Cryptographically-Secure Random Number Generator.
      const mn = bip39.generateMnemonic(wordlist);
      console.log(mn);

      // Call init when using the SDK in a web environment before calling any other SDK
      // methods. This is not needed when using the SDK in a Node.js/Deno environment.
      await init()

      // Create the default config, providing your Breez API key
      const config = defaultConfig('mainnet', import.meta.env.VITE_BREEZ_API_KEY)

      const sdk = await connect({ mnemonic: mn, config })
      sdkRef.current = sdk
      console.log(sdk)

      // Fetch wallet info
      const info = await sdk.getInfo()
      setWalletInfo({
        balanceSat: info.walletInfo.balanceSat,
        pendingSendSat: info.walletInfo.pendingSendSat,
        pendingReceiveSat: info.walletInfo.pendingReceiveSat
      })

      // Fetch lightning limits
      const limits = await sdk.fetchLightningLimits()
      setLightningLimits({
        min: limits.receive.minSat,
        max: limits.receive.maxSat
      })

      setIsInitialized(true)
    } catch (error) {
      console.error('Failed to initialize Breez SDK:', error)
      setIsInitialized(false)
      initializationRef.current = false; // Reset ref on error to allow retry
    }
  }

  const generateInvoice = async () => {
    if (!sdkRef.current) return;

    try {
      // First prepare the payment to check fees
      const optionalAmount = {
        type: 'bitcoin',
        payerAmountSat: receiveAmount
      } as ReceiveAmount

      // Step 1: Prepare the payment
      const prepareResponse = await sdkRef.current.prepareReceivePayment({
        paymentMethod: 'lightning',
        amount: optionalAmount
      })

      // Store the fees
      setFees(prepareResponse.feesSat)

      // Step 2: Generate the actual invoice using the prepare response
      const receiveResponse = await sdkRef.current.receivePayment({
        prepareResponse
      })

      // Store the invoice - it's directly in the destination field
      if (typeof receiveResponse === 'object' && receiveResponse !== null && receiveResponse.destination) {
        setInvoice(receiveResponse.destination)
      }
    } catch (error) {
      console.error('Failed to generate invoice:', error)
    }
  }

  useEffect(() => {
    connectToBreez()
  }, []) // Empty dependency array ensures this runs only once

  // Helper function to format satoshis to BTC
  const formatSatToBTC = (sats: number) => {
    return (sats / 100000000).toFixed(8)
  }

  return (
    <div className="container mx-auto p-4 max-w-3xl">
      <h1 className="text-3xl font-bold text-center mb-6">Bitcoin Liquid Wallet</h1>

      <Card className="mb-6">
        <CardHeader>
          <CardTitle>Wallet Balance</CardTitle>
          <CardDescription>Overview of your current wallet balances</CardDescription>
        </CardHeader>
        <CardContent className="grid grid-cols-1 md:grid-cols-3 gap-4">
          <div className="flex flex-col">
            <h3 className="text-sm font-medium mb-1">Available Balance</h3>
            <p className="text-xl font-bold">{formatSatToBTC(walletInfo.balanceSat)} BTC</p>
            <Badge variant="secondary" className="mt-1">{walletInfo.balanceSat} sats</Badge>
          </div>
          <div className="flex flex-col">
            <h3 className="text-sm font-medium mb-1">Pending Send</h3>
            <p className="text-xl font-bold">{formatSatToBTC(walletInfo.pendingSendSat)} BTC</p>
            <Badge variant="secondary" className="mt-1">{walletInfo.pendingSendSat} sats</Badge>
          </div>
          <div className="flex flex-col">
            <h3 className="text-sm font-medium mb-1">Pending Receive</h3>
            <p className="text-xl font-bold">{formatSatToBTC(walletInfo.pendingReceiveSat)} BTC</p>
            <Badge variant="secondary" className="mt-1">{walletInfo.pendingReceiveSat} sats</Badge>
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
            <Input
              type="number"
              value={receiveAmount}
              onChange={(e) => setReceiveAmount(Number(e.target.value))}
              min={lightningLimits.min}
              max={lightningLimits.max}
            />
            <p className="text-sm text-muted-foreground">
              Min: {lightningLimits.min} sats, Max: {lightningLimits.max} sats
            </p>
          </div>

          <Button
            onClick={generateInvoice}
            disabled={!isInitialized || receiveAmount < lightningLimits.min || receiveAmount > lightningLimits.max}
            className="w-full"
          >
            Generate Invoice
          </Button>

          {fees > 0 && (
            <Alert variant="default" className="mt-2">
              <AlertDescription>Network Fees: {fees} sats</AlertDescription>
            </Alert>
          )}

          {invoice && (
            <div className="mt-4 space-y-2">
              <div className="flex justify-between items-center">
                <h3 className="text-sm font-medium">Lightning Invoice</h3>
                <Button 
                  variant="outline" 
                  size="sm" 
                  onClick={() => {
                    navigator.clipboard.writeText(invoice);
                    setShowCopyDialog(true);
                  }}
                >
                  Copy Invoice
                </Button>
              </div>
              <ScrollArea className="h-24 w-full rounded-md border p-2">
                <div className="p-2">
                  {invoice}
                </div>
              </ScrollArea>
              
              <AlertDialog open={showCopyDialog} onOpenChange={setShowCopyDialog}>
                <AlertDialogContent>
                  <AlertDialogHeader>
                    <AlertDialogTitle>Invoice Copied</AlertDialogTitle>
                    <AlertDialogDescription>
                      The lightning invoice has been copied to your clipboard.
                    </AlertDialogDescription>
                  </AlertDialogHeader>
                  <AlertDialogFooter>
                    <AlertDialogAction onClick={() => setShowCopyDialog(false)}>
                      OK
                    </AlertDialogAction>
                  </AlertDialogFooter>
                </AlertDialogContent>
              </AlertDialog>
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  )
}

export default App
