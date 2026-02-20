import { contextBridge, ipcRenderer } from "electron";

import { LND_RUNTIME_CHANNELS } from "./main/lndRuntimeIpc";
import { LND_WALLET_CHANNELS } from "./main/lndWalletIpc";
import { SPARK_WALLET_CHANNELS } from "./main/sparkWalletIpc";
import { L402_CREDENTIAL_CACHE_CHANNELS } from "./main/l402CredentialCacheIpc";

const config = {
  openAgentsBaseUrl: process.env.OA_DESKTOP_OPENAGENTS_BASE_URL,
  convexUrl: process.env.OA_DESKTOP_CONVEX_URL,
  khalaSyncEnabled: process.env.OA_DESKTOP_KHALA_SYNC_ENABLED === "true",
  khalaSyncUrl: process.env.OA_DESKTOP_KHALA_SYNC_URL,
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
  l402CredentialCache: {
    getByHost: (input: { readonly host: string; readonly scope: string; readonly nowMs: number }) =>
      ipcRenderer.invoke(L402_CREDENTIAL_CACHE_CHANNELS.getByHost, input),
    putByHost: (input: {
      readonly host: string;
      readonly scope: string;
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      readonly credential: any;
      readonly options?: { readonly ttlMs?: number };
    }) => ipcRenderer.invoke(L402_CREDENTIAL_CACHE_CHANNELS.putByHost, input),
    markInvalid: (input: { readonly host: string; readonly scope: string }) =>
      ipcRenderer.invoke(L402_CREDENTIAL_CACHE_CHANNELS.markInvalid, input),
    clearHost: (input: { readonly host: string; readonly scope: string }) =>
      ipcRenderer.invoke(L402_CREDENTIAL_CACHE_CHANNELS.clearHost, input),
  },
});
