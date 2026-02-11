import { contextBridge } from "electron";

const config = {
  openAgentsBaseUrl: process.env.OA_DESKTOP_OPENAGENTS_BASE_URL,
  convexUrl: process.env.OA_DESKTOP_CONVEX_URL,
  executorTickMs: process.env.OA_DESKTOP_EXECUTOR_TICK_MS
    ? Number(process.env.OA_DESKTOP_EXECUTOR_TICK_MS)
    : undefined,
};

contextBridge.exposeInMainWorld("openAgentsDesktop", {
  config,
});
