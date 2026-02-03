import { useState } from "react";
import type { GetInfoResponse, Payment, Rate, FiatCurrency, DepositInfo } from "@breeztech/breez-sdk-spark";
import { Button } from "@/components/ui/button";
import { mergeDepositsWithTransactions } from "@/lib/wallet/depositHelpers";
import SendPaymentDialog from "./SendPaymentDialog";
import ReceivePaymentDialog from "./ReceivePaymentDialog";

interface WalletPageProps {
  walletInfo: GetInfoResponse | null;
  transactions: Payment[];
  unclaimedDeposits: DepositInfo[];
  fiatRates: Rate[];
  fiatCurrencies: FiatCurrency[];
  refreshWalletData: (showLoading?: boolean) => Promise<void>;
  isRestoring: boolean;
  error: string | null;
  onClearError: () => void;
  onLogout: () => void;
  hasUnclaimedDeposits: boolean;
  onOpenGetRefund: (source?: "menu" | "icon") => void;
  onOpenSettings: () => void;
  onOpenBackup: () => void;
}

export default function WalletPage({
  walletInfo,
  transactions,
  unclaimedDeposits,
  refreshWalletData,
  isRestoring,
  error,
  onClearError,
  onLogout,
}: WalletPageProps) {
  const [sendOpen, setSendOpen] = useState(false);
  const [receiveOpen, setReceiveOpen] = useState(false);

  const list = mergeDepositsWithTransactions(transactions, unclaimedDeposits);
  const balanceSats = walletInfo?.balanceSats ?? 0;

  return (
    <div className="flex flex-col min-h-[70vh]">
      {isRestoring && (
        <div className="absolute inset-0 z-50 flex items-center justify-center bg-background/90 backdrop-blur-sm">
          <div className="size-10 animate-spin rounded-full border-2 border-primary border-t-transparent" />
        </div>
      )}

      <header className="sticky top-0 z-10 border-b border-border bg-background/95 px-4 py-4 backdrop-blur">
        <div className="flex items-center justify-between">
          <div>
            <p className="text-xs text-muted-foreground uppercase tracking-wider">Balance</p>
            <p className="text-2xl font-mono font-semibold text-foreground">
              {Number(balanceSats).toLocaleString()} sats
            </p>
          </div>
          <Button variant="ghost" size="sm" onClick={onLogout}>
            Logout
          </Button>
        </div>
        {error && (
          <p className="mt-2 text-sm text-destructive" onClick={onClearError}>
            {error}
          </p>
        )}
      </header>

      <div className="flex-1 overflow-y-auto p-4">
        {list.length === 0 ? (
          <div className="flex flex-col items-center justify-center py-16 text-center">
            <p className="text-muted-foreground text-sm">No payments yet</p>
            <p className="mt-1 text-xs text-muted-foreground">
              Send or receive to see history here.
            </p>
          </div>
        ) : (
          <ul className="space-y-2">
            {list.slice(0, 50).map((tx) => (
              <li
                key={"id" in tx ? tx.id : `deposit-${(tx as { depositInfo?: DepositInfo }).depositInfo?.txid}`}
                className="flex items-center justify-between rounded-lg border border-border bg-card px-4 py-3"
              >
                <span className="text-sm text-muted-foreground">
                  {(tx as Payment).paymentType === "receive" ? "Received" : "Sent"}
                </span>
                <span className="font-mono text-sm font-medium text-foreground">
                  {(tx as Payment).paymentType === "receive" ? "+" : "-"}
                  {Number((tx as Payment).amount).toLocaleString()} sats
                </span>
              </li>
            ))}
          </ul>
        )}
      </div>

      <div className="bottom-bar flex items-center gap-2 border-t border-border bg-background p-3">
        <Button
          className="action-button action-button-send flex-1"
          onClick={() => setSendOpen(true)}
          data-testid="send-button"
        >
          <svg className="size-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2.5}>
            <path strokeLinecap="round" strokeLinejoin="round" d="M7 11l5-5m0 0l5 5m-5-5v12" />
          </svg>
          Send
        </Button>
        <Button
          className="action-button action-button-receive flex-1"
          onClick={() => setReceiveOpen(true)}
          data-testid="receive-button"
        >
          <svg className="size-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2.5}>
            <path strokeLinecap="round" strokeLinejoin="round" d="M17 13l-5 5m0 0l-5-5m5 5V6" />
          </svg>
          Receive
        </Button>
      </div>

      <SendPaymentDialog
        open={sendOpen}
        onOpenChange={setSendOpen}
        onClose={() => {
          setSendOpen(false);
          refreshWalletData(false);
        }}
      />
      <ReceivePaymentDialog
        open={receiveOpen}
        onOpenChange={setReceiveOpen}
        onClose={() => {
          setReceiveOpen(false);
          refreshWalletData(false);
        }}
      />
    </div>
  );
}
