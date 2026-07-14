export const KHALA_CODE_CODEX_PARITY_REFERENCE_COMMIT =
  "08ba14b03d0b3ce3cfdf8c88c0469b9b1924953d"

export const KHALA_CODE_CODEX_PARITY_REFERENCE_LABEL =
  "openai/codex app-server v2 schema at 08ba14b03d"

export const KHALA_CODE_CODEX_PARITY_REQUIRED_THREAD_ITEM_TYPES = [
  "userMessage", "hookPrompt", "agentMessage", "plan", "reasoning",
  "commandExecution", "fileChange", "mcpToolCall", "dynamicToolCall",
  "collabAgentToolCall", "subAgentActivity", "webSearch", "imageView",
  "sleep", "imageGeneration", "enteredReviewMode", "exitedReviewMode",
  "contextCompaction",
] as const
