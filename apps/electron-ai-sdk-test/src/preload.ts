import { contextBridge, ipcRenderer } from "electron";

contextBridge.exposeInMainWorld("harnessDesktop", {
  getEndpoint: () => ipcRenderer.invoke("harness:get-endpoint"),
});
