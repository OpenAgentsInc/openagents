import type { AgentCapabilities, AgentId, AgentInfo } from "./types.js"
import {
  connectUnifiedAgent,
  disconnectUnifiedAgent,
  sendUnifiedMessage,
  startUnifiedSession,
} from "../ipc/unified.js"

export abstract class Agent {
  abstract readonly id: AgentId
  abstract readonly name: string
  abstract readonly version: string

  abstract getCapabilities(): AgentCapabilities

  getInfo(): AgentInfo {
    return {
      id: this.id,
      name: this.name,
      version: this.version,
    }
  }

  connect(workspacePath: string, workspaceId: string) {
    return connectUnifiedAgent({
      agentIdStr: this.id,
      workspacePath,
      workspaceId,
    })
  }

  startSession(sessionId: string, workspacePath: string) {
    return startUnifiedSession({
      sessionId,
      workspacePath,
    })
  }

  sendMessage(sessionId: string, text: string) {
    return sendUnifiedMessage({
      sessionId,
      text,
    })
  }

  disconnect(sessionId: string) {
    return disconnectUnifiedAgent({
      sessionId,
    })
  }
}
