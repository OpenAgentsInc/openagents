// apps/coder/src/helpers/ipc/window/window-context.ts
import { contextBridge, ipcRenderer } from 'electron'; // Import directly
import {
  WIN_MINIMIZE_CHANNEL,
  WIN_MAXIMIZE_CHANNEL,
  WIN_CLOSE_CHANNEL,
} from "./window-channels";

export function exposeWindowContext() {
  // Remove: const { contextBridge, ipcRenderer } = window.require("electron");
  console.log('[Preload] Exposing ElectronWindowContext'); // Add log
  contextBridge.exposeInMainWorld("electronWindow", {
    minimize: () => ipcRenderer.invoke(WIN_MINIMIZE_CHANNEL),
    maximize: () => ipcRenderer.invoke(WIN_MAXIMIZE_CHANNEL),
    close: () => ipcRenderer.invoke(WIN_CLOSE_CHANNEL),
  });
}
