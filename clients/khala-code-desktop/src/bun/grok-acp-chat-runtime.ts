/**
 * Desktop entry for MH-3 Grok Axis A.
 * ACP package lives in @openagentsinc/grok-harness; desktop adapter implements
 * the CodexAppServerChatRuntime surface for rpc-handlers.
 */
export {
  createGrokAcpChatRuntime,
  type CreateGrokAcpChatRuntimeOptions,
  type GrokAcpChatRuntime,
} from "@openagentsinc/grok-harness"

export {
  createGrokDesktopChatRuntime,
  type CreateGrokDesktopChatRuntimeOptions,
  type GrokDesktopChatRuntime,
} from "./grok-desktop-chat-runtime.js"
