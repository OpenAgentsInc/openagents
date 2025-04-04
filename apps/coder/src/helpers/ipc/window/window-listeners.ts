import { BrowserWindow, ipcMain } from "electron";
import {
  WIN_CLOSE_CHANNEL,
  WIN_MAXIMIZE_CHANNEL,
  WIN_MINIMIZE_CHANNEL,
} from "./window-channels";

// Track which handlers have been registered to avoid duplicates
const registeredHandlers = new Set<string>();

export function addWindowEventListeners(mainWindow: BrowserWindow) {
  // Only register if not already registered
  if (!registeredHandlers.has(WIN_MINIMIZE_CHANNEL)) {
    ipcMain.handle(WIN_MINIMIZE_CHANNEL, () => {
      mainWindow.minimize();
    });
    registeredHandlers.add(WIN_MINIMIZE_CHANNEL);
  }
  
  if (!registeredHandlers.has(WIN_MAXIMIZE_CHANNEL)) {
    ipcMain.handle(WIN_MAXIMIZE_CHANNEL, () => {
      if (mainWindow.isMaximized()) {
        mainWindow.unmaximize();
      } else {
        mainWindow.maximize();
      }
    });
    registeredHandlers.add(WIN_MAXIMIZE_CHANNEL);
  }
  
  if (!registeredHandlers.has(WIN_CLOSE_CHANNEL)) {
    ipcMain.handle(WIN_CLOSE_CHANNEL, () => {
      mainWindow.close();
    });
    registeredHandlers.add(WIN_CLOSE_CHANNEL);
  }
}
