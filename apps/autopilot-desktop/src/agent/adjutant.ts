import { Agent } from "./base.js"
import type { AgentCapabilities, AgentId } from "./types.js"

export class AdjutantAgent extends Agent {
  readonly id: AgentId = "Adjutant"
  readonly name = "Adjutant"
  readonly version = "1.0.0"

  getCapabilities(): AgentCapabilities {
    return {
      session_new: true,
      session_load: false,
      session_list: false,
      prompt: true,
      fs_write: false,
      fs_read: false,
      terminal: false,
    }
  }
}
