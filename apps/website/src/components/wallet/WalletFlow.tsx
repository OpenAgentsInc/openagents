import React, { useEffect, useState, useCallback, useRef } from "react";
import type { Config, GetInfoResponse, Network, Payment, SdkEvent } from "@breeztech/breez-sdk-spark";
import { defaultConfig } from "@breeztech/breez-sdk-spark";
import { useWallet } from "./WalletContext";
import { useWalletToast } from "./WalletToastContext";
import { getSettings } from "@/lib/wallet/settings";
import { walletLogger, LogCategory } from "@/lib/wallet/logger";
import WalletHomePage from "./WalletHomePage";
import WalletGeneratePage from "./WalletGeneratePage";
import WalletRestorePage from "./WalletRestorePage";
import WalletPage from "./WalletPage";

const VITE_BREEZ_API_KEY = import.meta.env.VITE_BREEZ_API_KEY as string | undefined;

type Screen = "home" | "generate" | "restore" | "wallet";

export default function WalletFlow() {
  const wallet = useWallet();
  const { showToast } = useWalletToast();
  const [screen, setScreen] = useState<Screen>("home");
  const [isConnected, setIsConnected] = useState(false);
  const [isLoading, setIsLoading] = useState(true);
  const [isRestoring, setIsRestoring] = useState(false);
  const [walletInfo, setWalletInfo] = useState<GetInfoResponse | null>(null);
  const [transactions, setTransactions] = useState<Payment[]>([]);
  const [error, setError] = useState<string | null>(null);
  const listenerIdRef = useRef<string | null>(null);

  const refreshWalletData = useCallback(
    async (showLoading = true) => {
      if (!isConnected) return;
      try {
        if (showLoading) setIsLoading(true);
        const [info, txns] = await Promise.all([wallet.getWalletInfo(), wallet.getTransactions()]);
        setWalletInfo(info);
        setTransactions(txns);
      } catch (e) {
        walletLogger.error(LogCategory.SDK, "Refresh failed", {
          error: e instanceof Error ? e.message : String(e),
        });
        setError("Failed to refresh wallet data.");
      } finally {
        if (showLoading) setIsLoading(false);
      }
    },
    [isConnected, wallet]
  );

  const handleSdkEvent = useCallback(
    (event: SdkEvent) => {
      if (event.type === "synced") {
        if (isRestoring) setIsRestoring(false);
        refreshWalletData(false);
      } else if (event.type === "paymentSucceeded") {
        const amt = event.payment.amount;
        const isReceived = event.payment.paymentType === "receive";
        if (isReceived) showToast("success", "Payment received", `${amt} sats`);
        else showToast("success", "Payment sent", `${amt} sats sent`);
        refreshWalletData(false);
      }
    },
    [refreshWalletData, showToast, isRestoring]
  );

  useEffect(() => {
    if (isConnected) {
      wallet
        .addEventListener(handleSdkEvent)
        .then((id) => {
          listenerIdRef.current = id;
        })
        .catch((e) => {
          walletLogger.error(LogCategory.SDK, "Event listener failed", {
            error: e instanceof Error ? e.message : String(e),
          });
        });
      return () => {
        if (listenerIdRef.current) {
          wallet.removeEventListener(listenerIdRef.current).catch(() => {});
          listenerIdRef.current = null;
        }
      };
    }
  }, [isConnected, handleSdkEvent, wallet]);

  useEffect(() => {
    wallet.initLogSession().catch(() => {});
    const saved = wallet.getSavedMnemonic();
    if (saved) {
      (async () => {
        try {
          setIsLoading(true);
          const ok = await connectWallet(saved, true);
          if (ok) setScreen("wallet");
          else {
            setError("Failed to connect with saved phrase.");
            wallet.clearMnemonic();
            setScreen("home");
          }
        } catch (e) {
          walletLogger.error(LogCategory.SDK, "Auto-connect failed", {
            error: e instanceof Error ? e.message : String(e),
          });
          setError("Failed to connect with saved phrase.");
          wallet.clearMnemonic();
          setScreen("home");
        } finally {
          setIsLoading(false);
        }
      })();
    } else {
      setScreen("home");
      setIsLoading(false);
    }
  }, []);

  const connectWallet = async (mnemonic: string, restore: boolean, overrideNetwork?: Network): Promise<boolean> => {
    const apiKey = VITE_BREEZ_API_KEY as string | undefined;
    if (!apiKey) {
      showToast("error", "Missing API Key", "Set VITE_BREEZ_API_KEY in .env for local dev, or in build env for deploy.");
      setError("Wallet cannot connect: Breez API key not configured. Set VITE_BREEZ_API_KEY.");
      return false;
    }
    if (wallet.connected()) return true;
    setIsLoading(true);
    setIsRestoring(restore);
    setError(null);
    try {
      const network = (overrideNetwork ?? (typeof window !== "undefined" ? new URLSearchParams(window.location.search).get("network") ?? "mainnet" : "mainnet")) as Network;
      const config: Config = defaultConfig(network);
      config.apiKey = apiKey;
      config.privateEnabledDefault = false;
      try {
        const s = getSettings();
        if (s.depositMaxFee) config.maxDepositClaimFee = s.depositMaxFee as unknown as Config["maxDepositClaimFee"];
        if (s.syncIntervalSecs != null) config.syncIntervalSecs = s.syncIntervalSecs;
        if (s.lnurlDomain != null) config.lnurlDomain = s.lnurlDomain;
        if (s.preferSparkOverLightning != null) config.preferSparkOverLightning = s.preferSparkOverLightning;
      } catch {
        // ignore
      }
      await wallet.initWallet(mnemonic, config);
      wallet.saveMnemonic(mnemonic);
      const [info, txns] = await Promise.all([wallet.getWalletInfo(), wallet.getTransactions()]);
      setWalletInfo(info);
      setTransactions(txns);
      setIsConnected(true);
      setScreen("wallet");
      showToast("success", "Wallet connected");
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      walletLogger.error(LogCategory.SDK, "Connect failed", { error: msg });
      setError("Failed to connect. Check your phrase and try again.");
      throw e;
    } finally {
      setIsLoading(false);
      setIsRestoring(false);
    }
  };

  const handleLogout = useCallback(async () => {
    try {
      setIsLoading(true);
      if (isConnected) await wallet.disconnect();
      await wallet.endLogSession();
      wallet.clearMnemonic();
      setIsConnected(false);
      setWalletInfo(null);
      setTransactions([]);
      setScreen("home");
      showToast("success", "Logged out");
    } catch (e) {
      walletLogger.error(LogCategory.SESSION, "Logout failed", { error: e instanceof Error ? e.message : String(e) });
      setError("Logout failed.");
    } finally {
      setIsLoading(false);
    }
  }, [isConnected, wallet, showToast]);

  if (isLoading && screen === "home" && wallet.getSavedMnemonic()) {
    return (
      <div className="flex min-h-[50vh] items-center justify-center p-6">
        <div className="size-10 animate-spin rounded-full border-2 border-primary border-t-transparent" />
      </div>
    );
  }

  const goToRestore = useCallback(() => {
    console.log("[WalletFlow] goToRestore");
    setScreen("restore");
  }, []);
  const goToGenerate = useCallback(() => {
    console.log("[WalletFlow] goToGenerate");
    setScreen("generate");
  }, []);

  switch (screen) {
    case "home":
      return (
        <WalletHomePage
          onRestoreWallet={goToRestore}
          onCreateNewWallet={goToGenerate}
        />
      );
    case "restore":
      return (
        <WalletRestorePage
          onConnect={(mnemonic) => connectWallet(mnemonic, true)}
          onBack={() => setScreen("home")}
          onClearError={() => setError(null)}
        />
      );
    case "generate":
      return (
        <WalletGeneratePage
          onMnemonicConfirmed={(mnemonic) => connectWallet(mnemonic, false)}
          onBack={() => setScreen("home")}
          error={error}
          onClearError={() => setError(null)}
        />
      );
    case "wallet":
      return (
        <WalletPage
          walletInfo={walletInfo}
          transactions={transactions}
          unclaimedDeposits={[]}
          fiatRates={[]}
          fiatCurrencies={[]}
          refreshWalletData={refreshWalletData}
          isRestoring={isRestoring}
          error={error}
          onClearError={() => setError(null)}
          onLogout={handleLogout}
          hasUnclaimedDeposits={false}
          onOpenGetRefund={() => {}}
          onOpenSettings={() => {}}
          onOpenBackup={() => {}}
        />
      );
    default:
      return null;
  }
}
