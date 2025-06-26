import { html } from "@openagentsinc/psionic"
import { agentCard } from "./agent-card"

// Local type definition to avoid SDK import
interface AgentIdentity {
  id: string
  name: string
  nostrKeys: {
    public: string
    private: string
  }
  birthTimestamp: number
  generation: number
  lifecycleState?: string
  balance?: number
  metabolicRate?: number
  parentId?: string
}

export interface AgentListProps {
  agents: Array<AgentIdentity>
  title?: string
  emptyMessage?: string
  onSelectAgent?: (agent: any) => void
}

export function agentList({
  agents,
  emptyMessage = "No agents yet. Spawn your first agent!",
  onSelectAgent,
  title = "Your Agents"
}: AgentListProps) {
  return html`
    <div class="agent-list-container">
      <div class="list-header">
        <h2>${title}</h2>
        <span is-="badge" variant-="foreground1">${agents.length} agents</span>
      </div>
      
      ${
    agents.length === 0 ?
      html`
        <div class="empty-state" box-="square">
          <p>${emptyMessage}</p>
        </div>
      ` :
      html`
        <div class="agent-grid">
          ${agents.map((agent) => agentCard({ agent, ...(onSelectAgent ? { onSelect: onSelectAgent } : {}) })).join("")}
        </div>
      `
  }
    </div>

    <style>
      .agent-list-container {
        width: 100%;
      }

      .list-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        margin-bottom: 1.5rem;
      }

      .list-header h2 {
        margin: 0;
        color: var(--foreground0);
      }

      .empty-state {
        padding: 3rem;
        text-align: center;
        background: var(--background1);
      }

      .empty-state p {
        margin: 0;
        color: var(--foreground2);
        font-size: 1.1rem;
      }

      .agent-grid {
        display: grid;
        grid-template-columns: repeat(auto-fill, minmax(350px, 1fr));
        gap: 1.5rem;
      }

      @media (max-width: 768px) {
        .agent-grid {
          grid-template-columns: 1fr;
        }
      }
    </style>
  `
}
