import { useState, useEffect, useCallback } from "react";
import * as bip39 from "bip39";
import { Button } from "@/components/ui/button";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { walletLogger, LogCategory } from "@/lib/wallet/logger";

interface Props {
  onMnemonicConfirmed: (mnemonic: string) => void;
  onBack: () => void;
  error: string | null;
  onClearError: () => void;
}

export default function WalletGeneratePage({
  onMnemonicConfirmed,
  onBack,
  error,
  onClearError,
}: Props) {
  const [mnemonic, setMnemonic] = useState("");
  const [loading, setLoading] = useState(true);
  const [copied, setCopied] = useState(false);
  const [genError, setGenError] = useState<string | null>(null);

  const runGenerate = useCallback(() => {
    setGenError(null);
    setLoading(true);
    setMnemonic("");
    const id = setTimeout(() => {
      try {
        let phrase: string;
        try {
          phrase = bip39.generateMnemonic(128);
        } catch {
          // Fallback: use Web Crypto for entropy (bip39 RNG can fail in some bundles)
          const bytes = new Uint8Array(16);
          if (typeof crypto !== "undefined" && crypto.getRandomValues) {
            crypto.getRandomValues(bytes);
            const hex = Array.from(bytes)
              .map((b) => b.toString(16).padStart(2, "0"))
              .join("");
            phrase = bip39.entropyToMnemonic(hex);
          } else {
            throw new Error("No secure random source");
          }
        }
        if (!phrase || phrase.trim().split(/\s+/).length < 12) {
          throw new Error("Invalid mnemonic generated");
        }
        setMnemonic(phrase);
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e);
        walletLogger.error(LogCategory.AUTH, "Generate mnemonic failed", { error: msg });
        setGenError(msg || "Failed to generate phrase");
      } finally {
        setLoading(false);
      }
    }, 0);
    return () => clearTimeout(id);
  }, []);

  useEffect(() => {
    const cancel = runGenerate();
    return cancel;
  }, [runGenerate]);

  const handleCopy = () => {
    navigator.clipboard
      .writeText(mnemonic)
      .then(() => {
        setCopied(true);
        setTimeout(() => setCopied(false), 2000);
      })
      .catch(() => {});
  };

  if (loading) {
    return (
      <div className="flex min-h-[50vh] items-center justify-center p-6">
        <div className="size-10 animate-spin rounded-full border-2 border-primary border-t-transparent" />
      </div>
    );
  }

  if (genError) {
    return (
      <div className="mx-auto max-w-xl space-y-6 px-6 py-8">
        <div className="text-center">
          <h2 className="text-lg font-semibold text-foreground">Get Started</h2>
          <p className="mt-2 text-sm text-muted-foreground">Could not generate recovery phrase.</p>
        </div>
        <Alert variant="destructive">
          <AlertDescription>{genError}</AlertDescription>
        </Alert>
        <div className="flex gap-3">
          <Button variant="outline" onClick={onBack} className="flex-1">
            Back
          </Button>
          <Button onClick={runGenerate} className="flex-1">
            Retry
          </Button>
        </div>
      </div>
    );
  }

  const words = mnemonic.split(/\s+/).filter(Boolean);
  if (words.length < 12) {
    return (
      <div className="flex min-h-[50vh] items-center justify-center p-6">
        <div className="size-10 animate-spin rounded-full border-2 border-primary border-t-transparent" />
      </div>
    );
  }

  return (
    <div className="mx-auto max-w-xl space-y-6 px-6 py-8">
      <div className="text-center">
        <h2 className="text-lg font-semibold text-foreground">Get Started</h2>
        <p className="mt-2 text-sm text-muted-foreground">
          Write down these words in order. This is your only backup to recover your wallet.
        </p>
      </div>
      <div className="rounded-xl border border-border bg-card p-4">
        <div className="grid grid-cols-3 gap-2">
          {words.map((word, i) => (
            <div
              key={i}
              className="flex items-center gap-2 rounded-lg bg-muted/50 px-3 py-2 font-mono text-sm"
            >
              <span className="w-5 text-right text-xs text-muted-foreground">{i + 1}.</span>
              <span className="font-medium text-foreground">{word}</span>
            </div>
          ))}
        </div>
      </div>
      <div className="flex justify-center gap-4">
        <Button variant="outline" onClick={handleCopy}>
          {copied ? "Copied!" : "Copy to clipboard"}
        </Button>
      </div>
      {error && (
        <Alert variant="destructive" onClick={onClearError}>
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      )}
      <Alert variant="default" className="border-amber-500/50 bg-amber-500/10 text-amber-700 dark:text-amber-400">
        <AlertDescription>
          Never share your recovery phrase. Anyone with these words can access your funds.
        </AlertDescription>
      </Alert>
      <div className="flex gap-3">
        <Button variant="outline" onClick={onBack} className="flex-1">
          Back
        </Button>
        <Button onClick={() => onMnemonicConfirmed(mnemonic)} className="flex-1">
          I&apos;ve Saved My Phrase
        </Button>
      </div>
    </div>
  );
}
