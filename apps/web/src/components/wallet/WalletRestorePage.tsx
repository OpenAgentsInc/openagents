import React, { useState } from "react";
import { Button } from "@/components/ui/button";
import { Alert, AlertDescription } from "@/components/ui/alert";

interface Props {
  onConnect: (mnemonic: string) => void;
  onBack: () => void;
  onClearError: () => void;
}

export default function WalletRestorePage({ onConnect, onBack, onClearError }: Props) {
  const [mnemonic, setMnemonic] = useState("");
  const [error, setError] = useState<string | null>(null);

  const handleSubmit = () => {
    const cleaned = mnemonic.trim().replace(/\s+/g, " ");
    const wordCount = cleaned.split(" ").length;
    if (wordCount !== 12 && wordCount !== 24) {
      setError("Please enter a valid 12 or 24-word recovery phrase.");
      return;
    }
    setError(null);
    onConnect(cleaned);
  };

  return (
    <div className="mx-auto max-w-xl space-y-6 px-6 py-8">
      <div className="text-center">
        <h2 className="text-lg font-semibold text-foreground">Restore from Backup</h2>
        <p className="mt-2 text-sm text-muted-foreground">
          Enter your 12 or 24-word recovery phrase. Words should be separated by spaces.
        </p>
      </div>
      <textarea
        value={mnemonic}
        onChange={(e) => setMnemonic(e.target.value)}
        placeholder="word1 word2 word3 ..."
        className="min-h-[8rem] w-full resize-none rounded-xl border border-input bg-background px-4 py-3 font-mono text-sm text-foreground placeholder:text-muted-foreground focus:border-ring focus:ring-2 focus:ring-ring/20"
        data-testid="mnemonic-input"
      />
      {error && (
        <Alert variant="destructive" onClick={() => setError(null)}>
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      )}
      <div className="flex gap-3">
        <Button variant="outline" onClick={onBack} className="flex-1">
          Back
        </Button>
        <Button
          onClick={handleSubmit}
          disabled={!mnemonic.trim()}
          className="flex-1"
          data-testid="restore-confirm-button"
        >
          Restore Wallet
        </Button>
      </div>
    </div>
  );
}
