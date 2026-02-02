import { useState, useEffect } from "react";
import type { SendPaymentOptions } from "@breeztech/breez-sdk-spark";
import { useWallet } from "./WalletContext";
import type { SendInput } from "@/lib/wallet/domain";
import { walletLogger, LogCategory } from "@/lib/wallet/logger";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";

interface Props {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onClose: () => void;
}

export default function SendPaymentDialog({ open, onOpenChange, onClose }: Props) {
  const wallet = useWallet();
  const [step, setStep] = useState<"input" | "amount" | "confirm" | "processing" | "result">("input");
  const [paymentInput, setPaymentInput] = useState<SendInput | null>(null);
  const [amount, setAmount] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [prepareResponse, setPrepareResponse] = useState<Awaited<ReturnType<typeof wallet.prepareSendPayment>> | null>(null);
  const [result, setResult] = useState<"success" | "failure" | null>(null);

  useEffect(() => {
    if (open) {
      setStep("input");
      setPaymentInput(null);
      setAmount("");
      setError(null);
      setLoading(false);
      setPrepareResponse(null);
      setResult(null);
    }
  }, [open]);

  const processInput = async (input: string) => {
    const raw = input?.trim();
    if (!raw) {
      setError("Enter a payment destination.");
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const parsed = await wallet.parseInput(raw);
      setPaymentInput({ rawInput: raw, parsedInput: parsed });
      if (parsed.type === "bolt11Invoice" && parsed.amountMsat && parsed.amountMsat > 0n) {
        const msat = parsed.amountMsat;
        const sats =
          typeof msat === "bigint" ? Number(msat / 1000n) : Math.floor(msat / 1000);
        setAmount(String(sats));
        await doPrepare(raw, sats);
      } else if (parsed.type === "bitcoinAddress" || parsed.type === "sparkAddress") {
        setStep("amount");
      } else if (parsed.type === "lnurlPay" || parsed.type === "lightningAddress") {
        setError("LNURL / Lightning address not fully supported in this demo. Use Bolt11 or Spark address.");
      } else {
        setError("Unsupported payment type.");
      }
    } catch (e) {
      walletLogger.warn(LogCategory.PAYMENT, "Parse failed", { error: e instanceof Error ? e.message : String(e) });
      setError("Invalid payment destination.");
    } finally {
      setLoading(false);
    }
  };

  const doPrepare = async (paymentRequest: string, amountSats: number) => {
    if (amountSats <= 0) {
      setError("Enter a valid amount.");
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const res = await wallet.prepareSendPayment({ paymentRequest, amount: BigInt(amountSats) });
      setPrepareResponse(res);
      setStep("confirm");
    } catch (e) {
      setError(`Prepare failed: ${e instanceof Error ? e.message : "Unknown error"}`);
      setStep("amount");
    } finally {
      setLoading(false);
    }
  };

  const handleAmountNext = (amountSats: number) => {
    setAmount(String(amountSats));
    doPrepare(paymentInput?.rawInput ?? "", amountSats);
  };

  const handleSend = async (options?: SendPaymentOptions) => {
    if (!prepareResponse) return;
    setStep("processing");
    setLoading(true);
    setError(null);
    try {
      await wallet.sendPayment({ prepareResponse, options });
      setResult("success");
    } catch (e) {
      setError(`Payment failed: ${e instanceof Error ? e.message : "Unknown error"}`);
      setResult("failure");
    } finally {
      setLoading(false);
      setStep("result");
    }
  };

  const title =
    step === "input"
      ? "Send"
      : step === "amount"
        ? "Amount"
        : step === "confirm"
          ? "Confirm"
          : step === "processing"
            ? "Sending..."
            : step === "result"
              ? result === "success"
                ? "Payment Sent"
                : "Payment Failed"
              : "Send";

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{title}</DialogTitle>
        </DialogHeader>

        {step === "input" && (
          <div className="space-y-4">
            <div>
              <Label htmlFor="send-dest">Invoice or address</Label>
              <textarea
                id="send-dest"
                placeholder="lnbc... / bc1... / sp1..."
                className="mt-2 min-h-[80px] w-full resize-none rounded-lg border border-input bg-background px-3 py-2 font-mono text-sm"
                onBlur={(e) => e.target.value && processInput(e.target.value)}
              />
            </div>
            {error && <p className="text-sm text-destructive">{error}</p>}
            <Button
              className="w-full"
              disabled={loading}
              onClick={() => {
                const el = document.getElementById("send-dest") as HTMLTextAreaElement;
                if (el?.value) processInput(el.value);
              }}
            >
              {loading ? "Processing..." : "Continue"}
            </Button>
          </div>
        )}

        {step === "amount" && (
          <div className="space-y-4">
            <p className="text-sm text-muted-foreground break-all font-mono">{paymentInput?.rawInput}</p>
            <div>
              <Label htmlFor="send-amount">Amount (sats)</Label>
              <Input
                id="send-amount"
                type="number"
                min={1}
                value={amount}
                onChange={(e) => setAmount(e.target.value)}
                className="mt-2 font-mono"
              />
            </div>
            {error && <p className="text-sm text-destructive">{error}</p>}
            <div className="flex gap-2">
              <Button variant="outline" onClick={() => setStep("input")} className="flex-1">
                Back
              </Button>
              <Button
                className="flex-1"
                disabled={loading || !amount || parseInt(amount, 10) <= 0}
                onClick={() => handleAmountNext(parseInt(amount, 10) || 0)}
              >
                {loading ? "Preparing..." : "Continue"}
              </Button>
            </div>
          </div>
        )}

        {step === "confirm" && prepareResponse && (
          <div className="space-y-4">
            <p className="text-center text-2xl font-mono font-semibold">
              {Number(prepareResponse.amount).toLocaleString()} sats
            </p>
            <div className="flex gap-2">
              <Button variant="outline" onClick={() => setStep("amount")} className="flex-1">
                Back
              </Button>
              <Button className="flex-1" disabled={loading} onClick={() => handleSend()}>
                Confirm & Send
              </Button>
            </div>
          </div>
        )}

        {step === "processing" && (
          <div className="flex flex-col items-center justify-center py-8">
            <div className="size-12 animate-spin rounded-full border-2 border-primary border-t-transparent" />
            <p className="mt-4 text-sm text-muted-foreground">Processing payment...</p>
          </div>
        )}

        {step === "result" && (
          <div className="space-y-4 text-center">
            <p className={result === "success" ? "text-chart-2 font-medium" : "text-destructive"}>
              {result === "success" ? "Payment sent successfully." : error ?? "Payment failed."}
            </p>
            <Button className="w-full" onClick={onClose}>
              Done
            </Button>
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
}
