import React, { useEffect, useState } from "react";
import { initWasm } from "@/lib/wallet/wasmLoader";
import { walletLogger, LogCategory } from "@/lib/wallet/logger";
import { WalletProvider } from "./WalletContext";
import { WalletToastProvider } from "./WalletToastContext";
import WalletFlow from "./WalletFlow";

export default function WalletApp() {
  const [ready, setReady] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        walletLogger.info(LogCategory.UI, "Initializing wallet app");
        await initWasm();
        if (!cancelled) setReady(true);
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e);
        walletLogger.error(LogCategory.SDK, "WASM init failed", { error: msg });
        if (!cancelled) setError(msg);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  if (error) {
    return (
      <div className="flex min-h-[50vh] flex-col items-center justify-center gap-4 p-6 text-center">
        <h2 className="text-lg font-semibold text-destructive">Failed to load wallet</h2>
        <p className="text-sm text-muted-foreground max-w-md">{error}</p>
        <p className="text-xs text-muted-foreground">
          Ensure this page is served with Cross-Origin-Opener-Policy and Cross-Origin-Embedder-Policy headers (e.g. /wallet).
        </p>
      </div>
    );
  }

  if (!ready) {
    return (
      <div className="flex min-h-[50vh] flex-col items-center justify-center gap-4 p-6">
        <div className="size-10 animate-spin rounded-full border-2 border-primary border-t-transparent" />
        <p className="text-sm text-muted-foreground">Loading wallet...</p>
      </div>
    );
  }

  return (
    <WalletProvider>
      <WalletToastProvider>
        <WalletFlow />
      </WalletToastProvider>
    </WalletProvider>
  );
}
