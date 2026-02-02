import { useState, useEffect } from "react";
import { useWallet } from "./WalletContext";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Tabs, TabsList, TabsTrigger, TabsContent } from "@/components/ui/tabs";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";

type Tab = "lightning" | "spark" | "bitcoin";

interface Props {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onClose: () => void;
}

export default function ReceivePaymentDialog({ open, onOpenChange, onClose }: Props) {
  const wallet = useWallet();
  const [tab, setTab] = useState<Tab>("lightning");
  const [amount, setAmount] = useState("");
  const [description, setDescription] = useState("");
  const [paymentRequest, setPaymentRequest] = useState("");
  const [feeSats, setFeeSats] = useState(0);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [sparkAddress, setSparkAddress] = useState<string | null>(null);
  const [bitcoinAddress, setBitcoinAddress] = useState<string | null>(null);

  useEffect(() => {
    if (open) {
      setPaymentRequest("");
      setFeeSats(0);
      setError(null);
      setAmount("");
      setDescription("");
      setSparkAddress(null);
      setBitcoinAddress(null);
    }
  }, [open]);

  const generateBolt11 = async () => {
    const sats = parseInt(amount, 10);
    if (isNaN(sats) || sats < 1) {
      setError("Enter a valid amount (sats).");
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const res = await wallet.receivePayment({
        paymentMethod: { type: "bolt11Invoice", description, amountSats: sats },
      });
      setPaymentRequest(res.paymentRequest);
      setFeeSats(Number(res.fee));
    } catch (e) {
      setError(`Failed to create invoice: ${e instanceof Error ? e.message : "Unknown error"}`);
    } finally {
      setLoading(false);
    }
  };

  const generateSpark = async () => {
    if (sparkAddress) return;
    setLoading(true);
    setError(null);
    try {
      const res = await wallet.receivePayment({
        paymentMethod: { type: "sparkAddress" },
      });
      setSparkAddress(res.paymentRequest);
    } catch (e) {
      setError(`Failed to generate Spark address: ${e instanceof Error ? e.message : "Unknown error"}`);
    } finally {
      setLoading(false);
    }
  };

  const generateBitcoin = async () => {
    if (bitcoinAddress) return;
    setLoading(true);
    setError(null);
    try {
      const res = await wallet.receivePayment({
        paymentMethod: { type: "bitcoinAddress" },
      });
      setBitcoinAddress(res.paymentRequest);
    } catch (e) {
      setError(`Failed to generate Bitcoin address: ${e instanceof Error ? e.message : "Unknown error"}`);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    if (open && tab === "spark" && !sparkAddress && !loading) generateSpark();
  }, [open, tab]);
  useEffect(() => {
    if (open && tab === "bitcoin" && !bitcoinAddress && !loading) generateBitcoin();
  }, [open, tab]);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Receive</DialogTitle>
        </DialogHeader>

        <Tabs value={tab} onValueChange={(v) => setTab(v as Tab)}>
          <TabsList className="grid w-full grid-cols-3">
            <TabsTrigger value="lightning">Lightning</TabsTrigger>
            <TabsTrigger value="spark">Spark</TabsTrigger>
            <TabsTrigger value="bitcoin">Bitcoin</TabsTrigger>
          </TabsList>

          <TabsContent value="lightning" className="space-y-4 pt-4">
            <div>
              <Label htmlFor="rec-amount">Amount (sats)</Label>
              <Input
                id="rec-amount"
                type="number"
                min={1}
                value={amount}
                onChange={(e) => setAmount(e.target.value)}
                className="mt-2 font-mono"
              />
            </div>
            <div>
              <Label htmlFor="rec-desc">Description (optional)</Label>
              <Input
                id="rec-desc"
                value={description}
                onChange={(e) => setDescription(e.target.value)}
                className="mt-2"
                placeholder="Payment for..."
              />
            </div>
            {error && <p className="text-sm text-destructive">{error}</p>}
            {!paymentRequest ? (
              <Button className="w-full" disabled={loading} onClick={generateBolt11}>
                {loading ? "Generating..." : "Create Invoice"}
              </Button>
            ) : (
              <div className="space-y-2">
                <p className="break-all font-mono text-xs text-muted-foreground">{paymentRequest}</p>
                {feeSats > 0 && <p className="text-xs text-muted-foreground">Fee: {feeSats} sats</p>}
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => navigator.clipboard.writeText(paymentRequest)}
                >
                  Copy
                </Button>
              </div>
            )}
          </TabsContent>

          <TabsContent value="spark" className="space-y-4 pt-4">
            {error && <p className="text-sm text-destructive">{error}</p>}
            {loading && !sparkAddress && <p className="text-sm text-muted-foreground">Generating Spark address...</p>}
            {sparkAddress && (
              <div className="space-y-2">
                <p className="break-all font-mono text-xs text-muted-foreground">{sparkAddress}</p>
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => navigator.clipboard.writeText(sparkAddress)}
                >
                  Copy
                </Button>
              </div>
            )}
          </TabsContent>

          <TabsContent value="bitcoin" className="space-y-4 pt-4">
            {error && <p className="text-sm text-destructive">{error}</p>}
            {loading && !bitcoinAddress && <p className="text-sm text-muted-foreground">Generating Bitcoin address...</p>}
            {bitcoinAddress && (
              <div className="space-y-2">
                <p className="break-all font-mono text-xs text-muted-foreground">{bitcoinAddress}</p>
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => navigator.clipboard.writeText(bitcoinAddress)}
                >
                  Copy
                </Button>
              </div>
            )}
          </TabsContent>
        </Tabs>

        <Button variant="outline" className="w-full" onClick={onClose}>
          Close
        </Button>
      </DialogContent>
    </Dialog>
  );
}
