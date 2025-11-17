import { ClaudeAcpAgent } from '@zed-industries/claude-code-acp'

export type AgentFactory = (client: any) => any

export type AgentName = 'claude-code' | 'codex' | string

export function getAgentFactory(name: AgentName): AgentFactory {
  switch (name) {
    case 'claude-code':
    default:
      return (client) => new ClaudeAcpAgent(client)
  }
}

