import { contextBridge, ipcRenderer } from "electron";

import { LND_RUNTIME_CHANNELS } from "./main/lndRuntimeIpc";

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
});
