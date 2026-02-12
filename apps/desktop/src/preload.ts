import { contextBridge, ipcRenderer } from "electron";

import { LND_RUNTIME_CHANNELS } from "./main/lndRuntimeIpc";
import { LND_WALLET_CHANNELS } from "./main/lndWalletIpc";
import { SPARK_WALLET_CHANNELS } from "./main/sparkWalletIpc";

const config = {
  openAgentsBaseUrl: process.env.OA_DESKTOP_OPENAGENTS_BASE_URL,
  convexUrl: process.env.OA_DESKTOP_CONVEX_URL,
  executorTickMs: process.env.OA_DESKTOP_EXECUTOR_TICK_MS
    ? Number(process.env.OA_DESKTOP_EXECUTOR_TICK_MS)
    : undefined,
};

contextBridge.exposeInMainWorld("openAgentsDesktop", {
  config,
  lndRuntime: {
    snapshot: () => ipcRenderer.invoke(LND_RUNTIME_CHANNELS.snapshot),
    start: () => ipcRenderer.invoke(LND_RUNTIME_CHANNELS.start),
    stop: () => ipcRenderer.invoke(LND_RUNTIME_CHANNELS.stop),
    restart: () => ipcRenderer.invoke(LND_RUNTIME_CHANNELS.restart),
  },
  lndWallet: {
    snapshot: () => ipcRenderer.invoke(LND_WALLET_CHANNELS.snapshot),
    initialize: (input: { readonly passphrase: string; readonly seedMnemonic?: ReadonlyArray<string> }) =>
      ipcRenderer.invoke(LND_WALLET_CHANNELS.initialize, input),
    unlock: (input?: { readonly passphrase?: string }) =>
      ipcRenderer.invoke(LND_WALLET_CHANNELS.unlock, input),
    lock: () => ipcRenderer.invoke(LND_WALLET_CHANNELS.lock),
    acknowledgeSeedBackup: () => ipcRenderer.invoke(LND_WALLET_CHANNELS.acknowledgeSeedBackup),
    prepareRestore: () => ipcRenderer.invoke(LND_WALLET_CHANNELS.prepareRestore),
    restore: (input: {
      readonly passphrase: string;
      readonly seedMnemonic: ReadonlyArray<string>;
      readonly recoveryWindowDays?: number;
    }) => ipcRenderer.invoke(LND_WALLET_CHANNELS.restore, input),
  },
  sparkWallet: {
    snapshot: () => ipcRenderer.invoke(SPARK_WALLET_CHANNELS.snapshot),
    bootstrap: () => ipcRenderer.invoke(SPARK_WALLET_CHANNELS.bootstrap),
    refresh: () => ipcRenderer.invoke(SPARK_WALLET_CHANNELS.refresh),
    payInvoice: (input: {
      readonly invoice: string;
      readonly host: string;
      readonly maxAmountMsats: number;
    }) => ipcRenderer.invoke(SPARK_WALLET_CHANNELS.payInvoice, input),
    disconnect: () => ipcRenderer.invoke(SPARK_WALLET_CHANNELS.disconnect),
  },
});
