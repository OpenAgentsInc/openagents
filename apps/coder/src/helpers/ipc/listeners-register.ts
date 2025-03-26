import { BrowserWindow } from "electron";
import { addThemeEventListeners } from "./theme/theme-listeners";
import { addWindowEventListeners } from "./window/window-listeners";
import { addMcpEventListeners } from "./mcp/mcp-listeners";

export default function registerListeners(mainWindow: BrowserWindow) {
  addWindowEventListeners(mainWindow);
  addThemeEventListeners();
  addMcpEventListeners();
}
