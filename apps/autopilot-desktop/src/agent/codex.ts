import { Agent } from "./base.js"
import type { AgentCapabilities, AgentId } from "./types.js"

export class CodexAgent extends Agent {
  readonly id: AgentId = "Codex"
  readonly name = "Codex"
  readonly version = "0.1.0"

  getCapabilities(): AgentCapabilities {
    return {
      session_new: true,
      session_load: true,
      session_list: true,
      prompt: true,
      fs_write: true,
      fs_read: true,
      terminal: true,
    }
  }
}
