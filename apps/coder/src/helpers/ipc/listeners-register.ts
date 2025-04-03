import { BrowserWindow } from "electron";
import { addThemeEventListeners } from "./theme/theme-listeners";
import { addWindowEventListeners } from "./window/window-listeners";
import { registerApiPortListeners } from "./api-port/api-port-listeners";
// import { registerCommandListeners } from "./command/command-listeners";

export default function registerListeners(mainWindow: BrowserWindow) {
  addWindowEventListeners(mainWindow);
  addThemeEventListeners();
  registerApiPortListeners();
  // registerCommandListeners();
}
