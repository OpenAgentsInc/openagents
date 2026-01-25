import type { AgentId } from "./types.js"
import { CodexAgent } from "./codex.js"
import { GeminiAgent } from "./gemini.js"
import { AdjutantAgent } from "./adjutant.js"
import type { Agent } from "./base.js"

export class AgentRegistry {
  private static readonly agents: Map<AgentId, Agent> = new Map<AgentId, Agent>([
    ["Codex", new CodexAgent()],
    ["Gemini", new GeminiAgent()],
    ["Adjutant", new AdjutantAgent()],
  ])

  static getAgent(id: AgentId): Agent | undefined {
    return this.agents.get(id)
  }

  static getAllAgents(): Agent[] {
    return Array.from(this.agents.values())
  }
}
