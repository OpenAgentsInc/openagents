import { useEffect, useRef } from "react";
import { createPortal } from "react-dom";

interface Props {
  onRestoreWallet: () => void;
  onCreateNewWallet: () => void;
}

export default function WalletHomePage({ onRestoreWallet, onCreateNewWallet }: Props) {
  const getStartedRef = useRef<HTMLButtonElement>(null);
  const restoreRef = useRef<HTMLButtonElement>(null);

  // Native DOM listeners so nothing in React or parents can block events
  useEffect(() => {
    const getStartedEl = getStartedRef.current;
    const restoreEl = restoreRef.current;
    if (!getStartedEl || !restoreEl) return;

    const onGetStarted = (e: Event) => {
      e.preventDefault();
      e.stopPropagation();
      onCreateNewWallet();
    };
    const onRestore = (e: Event) => {
      e.preventDefault();
      e.stopPropagation();
      onRestoreWallet();
    };

    // Capture-phase so we get the event before any other handler
    for (const ev of ["mousedown", "click", "touchstart"] as const) {
      getStartedEl.addEventListener(ev, onGetStarted, true);
      restoreEl.addEventListener(ev, onRestore, true);
    }
    return () => {
      for (const ev of ["mousedown", "click", "touchstart"] as const) {
        getStartedEl.removeEventListener(ev, onGetStarted, true);
        restoreEl.removeEventListener(ev, onRestore, true);
      }
    };
  }, [onCreateNewWallet, onRestoreWallet]);

  const content = (
    <div className="fixed inset-0 z-[9998] flex min-h-screen items-center justify-center bg-background">
      <div className="flex min-h-[70vh] flex-col items-center justify-center gap-10 px-6">
        <div className="text-center">
          <h1 className="text-3xl font-bold tracking-tight text-foreground">Wallet</h1>
          <p className="mt-2 text-sm text-muted-foreground">Powered by Breez SDK</p>
        </div>
        <div className="flex w-full max-w-xs flex-col gap-4">
          <button
            ref={getStartedRef}
            type="button"
            className="inline-flex h-10 w-full cursor-pointer items-center justify-center rounded-md bg-primary px-6 text-sm font-medium text-primary-foreground transition-colors hover:bg-primary/90 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
            data-testid="create-wallet-button"
          >
            Get Started
          </button>
          <button
            ref={restoreRef}
            type="button"
            className="inline-flex h-10 w-full cursor-pointer items-center justify-center rounded-md border border-input bg-background px-6 text-sm font-medium shadow-xs transition-colors hover:bg-accent hover:text-accent-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
            data-testid="restore-wallet-button"
          >
            Restore from Backup
          </button>
        </div>
      </div>
    </div>
  );

  // Render into document.body so no layout parent can capture or block events
  if (typeof document !== "undefined") {
    return createPortal(content, document.body, "wallet-home-portal");
  }
  return content;
}
