import { html } from "@openagentsinc/psionic"

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

export interface AgentCardProps {
  agent: AgentIdentity
  onSelect?: (agent: any) => void
}

export function agentCard({ agent, onSelect }: AgentCardProps) {
  const stateColors: Record<string, string> = {
    "bootstrapping": "background2",
    "active": "foreground0",
    "hibernating": "foreground2",
    "reproducing": "accent",
    "dying": "danger",
    "dead": "background3",
    "rebirth": "warning"
  }

  const stateColor = stateColors[agent.lifecycleState || "bootstrapping"] || "background2"
  const balance = agent.balance || 0
  const metabolicRate = agent.metabolicRate || 100
  const hoursRemaining = metabolicRate > 0 ? Math.floor(balance / metabolicRate) : 0

  return html`
    <div 
      class="agent-card" 
      box-="square"
      onclick="${onSelect ? `handleAgentSelect('${agent.id}')` : ""}"
      style="cursor: ${onSelect ? "pointer" : "default"}"
    >
      <div class="agent-header">
        <h3 class="agent-name">${agent.name}</h3>
        <span is-="badge" variant-="${stateColor}" cap-="round">
          ${agent.lifecycleState || "bootstrapping"}
        </span>
      </div>
      
      <div class="agent-details">
        <div class="detail-row">
          <span class="detail-label">ID:</span>
          <span class="detail-value">${agent.id}</span>
        </div>
        <div class="detail-row">
          <span class="detail-label">Public Key:</span>
          <span class="detail-value npub" title="${agent.nostrKeys.public}">
            ${agent.nostrKeys.public.slice(0, 16)}...
          </span>
        </div>
        <div class="detail-row">
          <span class="detail-label">Generation:</span>
          <span class="detail-value">${agent.generation}</span>
        </div>
        <div class="detail-row">
          <span class="detail-label">Balance:</span>
          <span class="detail-value">${balance.toLocaleString()} sats</span>
        </div>
        <div class="detail-row">
          <span class="detail-label">Metabolic Rate:</span>
          <span class="detail-value">${metabolicRate} sats/hour</span>
        </div>
        <div class="detail-row">
          <span class="detail-label">Time Remaining:</span>
          <span class="detail-value ${hoursRemaining < 24 ? "warning" : ""}">
            ${hoursRemaining}h
          </span>
        </div>
      </div>
      
      <div class="agent-actions">
        <button is-="button" size-="small" variant-="foreground1" onclick="fundAgent('${agent.id}')">
          Fund
        </button>
        <button is-="button" size-="small" variant-="background1" onclick="viewAgentDetails('${agent.id}')">
          Details
        </button>
      </div>
    </div>

    <style>
      .agent-card {
        padding: 1.5rem;
        background: var(--background1);
        transition: all 0.2s ease;
      }

      .agent-card:hover {
        background: var(--background2);
      }

      .agent-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        margin-bottom: 1rem;
      }

      .agent-name {
        margin: 0;
        font-size: 1.2rem;
        color: var(--foreground0);
      }

      .agent-details {
        margin-bottom: 1rem;
      }

      .detail-row {
        display: flex;
        justify-content: space-between;
        margin-bottom: 0.5rem;
        font-size: 0.9rem;
      }

      .detail-label {
        color: var(--foreground2);
      }

      .detail-value {
        color: var(--foreground1);
        font-family: "Berkeley Mono", monospace;
      }

      .detail-value.npub {
        cursor: help;
      }

      .detail-value.warning {
        color: var(--warning);
      }

      .agent-actions {
        display: flex;
        gap: 0.5rem;
        justify-content: flex-end;
      }
    </style>
  `
}
