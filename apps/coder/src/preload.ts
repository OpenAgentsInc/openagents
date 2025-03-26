import { contextBridge, ipcRenderer } from "electron";

// Expose protected methods that allow the renderer process to use
// the ipcRenderer without exposing the entire object
contextBridge.exposeInMainWorld("api", {
  listIssues: (owner: string, repo: string) => 
    ipcRenderer.invoke("mcp:list-issues", owner, repo),
  
  listPullRequests: (owner: string, repo: string) => 
    ipcRenderer.invoke("mcp:list-prs", owner, repo),
  
  viewFileContents: (owner: string, repo: string, path: string) => 
    ipcRenderer.invoke("mcp:view-file", owner, repo, path),
});