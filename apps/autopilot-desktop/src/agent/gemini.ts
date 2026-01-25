import { Agent } from "./base.js"
import type { AgentCapabilities, AgentId } from "./types.js"

export class GeminiAgent extends Agent {
  readonly id: AgentId = "Gemini"
  readonly name = "Gemini"
  readonly version = "0.1.0"

  getCapabilities(): AgentCapabilities {
    return {
      session_new: true,
      session_load: true,
      session_list: false, // Gemini CLI might not support listing yet
      prompt: true,
      fs_write: true,
      fs_read: true,
      terminal: true,
    }
  }
}
