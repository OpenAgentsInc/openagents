import { contextBridge, ipcRenderer } from "electron";

export function exposeMcpContext() {
  contextBridge.exposeInMainWorld("electron", {
    mcpInvoke: (channel: string, ...args: any[]) => {
      return ipcRenderer.invoke(channel, ...args);
    },
    mcpGetUrl: () => {
      return ipcRenderer.invoke('mcp:getUrl');
    },
  });
}
