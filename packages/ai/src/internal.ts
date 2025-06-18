// Internal file to force proper exports
export * from "./AiService.js"
export * from "./config/ClaudeCodeConfig.js"
export * from "./errors/index.js"
export * from "./providers/ClaudeCodeProvider.js"
// Removed ClaudeCodePty exports as they require node-pty which is incompatible with Bun
// export { ClaudeCodePtyClientLive, makeClaudeCodePtyClient } from "./providers/ClaudeCodePty.js"
export { ClaudeCodeClient, ClaudeCodeClientLive, makeClaudeCodeClient } from "./providers/ClaudeCodeSimple.js"
export type { ClaudeCodeJsonResponse, ClaudeCodeTextResponse, PromptOptions } from "./providers/ClaudeCodeSimple.js"
