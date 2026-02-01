import * as breezSdk from "@breeztech/breez-sdk-spark";
import {
  BreezSdk,
  Config,
  connect,
  initLogging,
  GetInfoRequest,
  GetInfoResponse,
  ListPaymentsRequest,
  ListPaymentsResponse,
  Payment,
  SendPaymentRequest,
  SendPaymentResponse,
  ReceivePaymentRequest,
  ReceivePaymentResponse,
  PrepareSendPaymentRequest,
  PrepareSendPaymentResponse,
  SdkEvent,
  EventListener,
  LogEntry,
  PrepareLnurlPayResponse,
  PrepareLnurlPayRequest,
  LnurlPayRequest,
  LnurlPayResponse,
  DepositInfo,
  Fee,
  UserSettings,
  UpdateUserSettingsRequest,
  FiatCurrency,
  Rate,
} from "@breeztech/breez-sdk-spark";
import type { WalletAPI } from "./WalletAPI";
import { walletLogger, LogCategory, logSdkMessage } from "./logger";
import { getAllSessions, isStorageAvailable } from "./logStorage";
import JSZip from "jszip";

class WebLogger {
  log = (logEntry: LogEntry) => {
    logSdkMessage(logEntry.level, logEntry.line);
  };
}

let sdk: BreezSdk | null = null;
let sdkLogger: WebLogger | null = null;

const STORAGE_DIR = "openagents-spark-wallet";

export const initWallet = async (mnemonic: string, config: Config): Promise<void> => {
  if (sdk) {
    walletLogger.warn(LogCategory.SDK, "initWallet called but SDK is already initialized; skipping");
    return;
  }
  try {
    if (!sdkLogger) {
      sdkLogger = new WebLogger();
      await initLogging(sdkLogger);
    }
    sdk = await connect({
      config,
      seed: { type: "mnemonic", mnemonic },
      storageDir: STORAGE_DIR,
    });
    walletLogger.sdkInitialized();
    walletLogger.authSuccess("mnemonic");
  } catch (error) {
    const msg = error instanceof Error ? error.message : "Unknown error";
    walletLogger.sdkError("initWallet", msg);
    walletLogger.authFailure("mnemonic", msg);
    throw error;
  }
};

export const parseInput = async (input: string): Promise<breezSdk.InputType> => {
  if (!sdk) throw new Error("SDK not initialized");
  return await sdk.parse(input);
};

export const prepareLnurlPay = async (params: PrepareLnurlPayRequest): Promise<PrepareLnurlPayResponse> => {
  if (!sdk) throw new Error("SDK not initialized");
  return await sdk.prepareLnurlPay(params);
};

export const lnurlPay = async (params: LnurlPayRequest): Promise<LnurlPayResponse> => {
  if (!sdk) throw new Error("SDK not initialized");
  return await sdk.lnurlPay(params);
};

export const prepareSendPayment = async (
  params: PrepareSendPaymentRequest
): Promise<PrepareSendPaymentResponse> => {
  if (!sdk) throw new Error("SDK not initialized");
  return await sdk.prepareSendPayment(params);
};

export const sendPayment = async (params: SendPaymentRequest): Promise<SendPaymentResponse> => {
  if (!sdk) throw new Error("SDK not initialized");
  return await sdk.sendPayment(params);
};

export const receivePayment = async (
  params: ReceivePaymentRequest
): Promise<ReceivePaymentResponse> => {
  if (!sdk) throw new Error("SDK not initialized");
  return await sdk.receivePayment(params);
};

export const unclaimedDeposits = async (): Promise<DepositInfo[]> => {
  if (!sdk) throw new Error("SDK not initialized");
  return (await sdk.listUnclaimedDeposits({})).deposits;
};

export const claimDeposit = async (txid: string, vout: number, maxFee: Fee): Promise<void> => {
  if (!sdk) throw new Error("SDK not initialized");
  await sdk.claimDeposit({ txid, vout, maxFee });
};

export const refundDeposit = async (
  txid: string,
  vout: number,
  destinationAddress: string,
  fee: Fee
): Promise<void> => {
  if (!sdk) throw new Error("SDK not initialized");
  await sdk.refundDeposit({ txid, vout, destinationAddress, fee });
};

export const getUserSettings = async (): Promise<UserSettings> => {
  if (!sdk) throw new Error("SDK not initialized");
  return await sdk.getUserSettings();
};

export const setUserSettings = async (settings: UpdateUserSettingsRequest): Promise<void> => {
  if (!sdk) throw new Error("SDK not initialized");
  await sdk.updateUserSettings(settings);
};

export const listFiatCurrencies = async (): Promise<FiatCurrency[]> => {
  if (!sdk) throw new Error("SDK not initialized");
  return (await sdk.listFiatCurrencies()).currencies;
};

export const listFiatRates = async (): Promise<Rate[]> => {
  if (!sdk) throw new Error("SDK not initialized");
  return (await sdk.listFiatRates()).rates;
};

export const getSdkLogs = (): string => {
  return walletLogger
    .getLogsByCategory(LogCategory.SDK_INTERNAL)
    .map((e) => `[${e.timestamp}] ${e.level} ${e.message}`)
    .join("\n");
};

export const getAllLogs = (): string => walletLogger.getLogsAsString();

export const getAllLogsAsZip = async (): Promise<Blob> => {
  const zip = new JSZip();
  const now = new Date();
  const ts = Math.floor(now.getTime() / 1000);
  zip.file(`${ts}_wallet_current.txt`, "Wallet Log Export\n" + now.toISOString() + "\n\n" + getAllLogs());
  if (isStorageAvailable()) {
    try {
      const sessions = await getAllSessions();
      for (const s of sessions) {
        const sts = Math.floor(new Date(s.startedAt).getTime() / 1000);
        zip.file(`${sts}_wallet_session.txt`, s.logs || "(no logs)");
      }
    } catch {
      // ignore
    }
  }
  return zip.generateAsync({ type: "blob", compression: "DEFLATE" });
};

export const canShareFiles = (): boolean =>
  typeof navigator !== "undefined" &&
  typeof navigator.share === "function" &&
  typeof navigator.canShare === "function";

export const shareOrDownloadLogs = async (): Promise<void> => {
  const blob = await getAllLogsAsZip();
  const filename = `${Math.floor(Date.now() / 1000)}_wallet_logs.zip`;
  if (canShareFiles()) {
    const file = new File([blob], filename, { type: "application/zip" });
    if (navigator.canShare({ files: [file] })) {
      try {
        await navigator.share({ files: [file], title: "Wallet Logs" });
        return;
      } catch (e) {
        if ((e as Error).name === "AbortError") return;
      }
    }
  }
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  a.remove();
  URL.revokeObjectURL(url);
};

export const addEventListener = async (callback: (event: SdkEvent) => void): Promise<string> => {
  if (!sdk) throw new Error("SDK not initialized");
  const listener: EventListener = { onEvent: callback };
  return await sdk.addEventListener(listener);
};

export const removeEventListener = async (listenerId: string): Promise<void> => {
  if (!sdk || !listenerId) return;
  await sdk.removeEventListener(listenerId);
};

export const getWalletInfo = async (): Promise<GetInfoResponse | null> => {
  if (!sdk) return null;
  try {
    return await sdk.getInfo({} as GetInfoRequest);
  } catch (error) {
    walletLogger.sdkError("getWalletInfo", error instanceof Error ? error.message : "Unknown error");
    throw error;
  }
};

export const getTransactions = async (): Promise<Payment[]> => {
  if (!sdk) return [];
  try {
    const res = await sdk.listPayments({ offset: 0, limit: 100 } as ListPaymentsRequest);
    return (res as ListPaymentsResponse).payments;
  } catch (error) {
    walletLogger.sdkError("getTransactions", error instanceof Error ? error.message : "Unknown error");
    throw error;
  }
};

export const disconnect = async (): Promise<void> => {
  if (sdk) {
    try {
      await sdk.disconnect();
      sdk = null;
      walletLogger.sessionEnd("disconnect");
    } catch (error) {
      walletLogger.sdkError("disconnect", error instanceof Error ? error.message : "Unknown error");
      throw error;
    }
  }
};

export const connected = (): boolean => sdk !== null;

const MNEMONIC_KEY = "walletMnemonic";
export const saveMnemonic = (mnemonic: string): void => {
  try {
    localStorage.setItem(MNEMONIC_KEY, mnemonic);
  } catch {
    // ignore
  }
};
export const getSavedMnemonic = (): string | null => {
  try {
    return localStorage.getItem(MNEMONIC_KEY);
  } catch {
    return null;
  }
};
export const clearMnemonic = (): void => {
  try {
    localStorage.removeItem(MNEMONIC_KEY);
  } catch {
    // ignore
  }
};

export const getLightningAddress = async (): Promise<breezSdk.LightningAddressInfo | null> => {
  if (!sdk) throw new Error("SDK not initialized");
  try {
    const r = await sdk.getLightningAddress();
    return r ?? null;
  } catch (error) {
    walletLogger.sdkError("getLightningAddress", error instanceof Error ? error.message : "Unknown error");
    return null;
  }
};

export const checkLightningAddressAvailable = async (username: string): Promise<boolean> => {
  if (!sdk) throw new Error("SDK not initialized");
  return await sdk.checkLightningAddressAvailable({ username });
};

export const registerLightningAddress = async (username: string, description: string): Promise<void> => {
  if (!sdk) throw new Error("SDK not initialized");
  await sdk.registerLightningAddress({ username, description });
};

export const deleteLightningAddress = async (): Promise<void> => {
  if (!sdk) throw new Error("SDK not initialized");
  await sdk.deleteLightningAddress();
};

export const walletApi: WalletAPI = {
  initWallet,
  disconnect,
  connected,
  parseInput,
  prepareLnurlPay,
  lnurlPay,
  prepareSendPayment,
  sendPayment,
  receivePayment,
  unclaimedDeposits,
  claimDeposit,
  refundDeposit,
  getWalletInfo,
  getTransactions,
  addEventListener,
  removeEventListener,
  saveMnemonic,
  getSavedMnemonic,
  clearMnemonic,
  getLightningAddress,
  checkLightningAddressAvailable,
  registerLightningAddress,
  deleteLightningAddress,
  getUserSettings,
  setUserSettings,
  listFiatCurrencies,
  listFiatRates,
  getSdkLogs,
  getAppLogs: () => walletLogger.getLogsAsString(),
  getAllLogs,
  getAllLogsAsZip,
  canShareFiles,
  shareOrDownloadLogs,
  initLogSession: () => Promise.resolve(),
  endLogSession: () => Promise.resolve(),
};
