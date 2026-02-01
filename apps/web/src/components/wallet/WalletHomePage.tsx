import React from "react";
import { Button } from "@/components/ui/button";

interface Props {
  onRestoreWallet: () => void;
  onCreateNewWallet: () => void;
}

export default function WalletHomePage({ onRestoreWallet, onCreateNewWallet }: Props) {
  return (
    <div className="flex min-h-[70vh] flex-col items-center justify-center gap-10 px-6">
      <div className="text-center">
        <h1 className="text-3xl font-bold tracking-tight text-foreground">Wallet</h1>
        <p className="mt-2 text-sm text-muted-foreground">Powered by Breez SDK</p>
      </div>
      <div className="flex w-full max-w-xs flex-col gap-4">
        <Button onClick={onCreateNewWallet} size="lg" className="w-full" data-testid="create-wallet-button">
          Get Started
        </Button>
        <Button
          onClick={onRestoreWallet}
          variant="outline"
          size="lg"
          className="w-full"
          data-testid="restore-wallet-button"
        >
          Restore from Backup
        </Button>
      </div>
    </div>
  );
}
