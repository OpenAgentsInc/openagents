/** Browser-safe Effect Native application seam for non-Electron hosts. */
export {
  desktopShellIntents,
  desktopShellMainView,
  initialDesktopShellState,
  makeDesktopShellHandlers,
  withThreads,
} from "./shell.ts"

export type {
  ChatHost,
  DesktopShellState,
  DesktopWorkspaceName,
} from "./shell.ts"

export type { DesktopThread } from "../chat-contract.ts"
