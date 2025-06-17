import { document, html } from "@openagentsinc/psionic"
import { navigation } from "../components/navigation"
import { baseStyles } from "../styles"

// Mock agent data for now
const mockAgents = [
  {
    name: "CodeCraft Pro",
    id: "agent-001",
    status: "active",
    balance: 250000,
    hourlyRate: 500,
    capabilities: ["code-review", "refactoring", "debugging"],
    rating: 4.8,
    requests: 1420
  },
  {
    name: "DataMiner",
    id: "agent-002",
    status: "active",
    balance: 180000,
    hourlyRate: 300,
    capabilities: ["data-analysis", "visualization", "reporting"],
    rating: 4.6,
    requests: 892
  },
  {
    name: "ContentGenius",
    id: "agent-003",
    status: "hibernating",
    balance: 5000,
    hourlyRate: 200,
    capabilities: ["writing", "editing", "translation"],
    rating: 4.9,
    requests: 2103
  }
]

export function agents() {
  return document({
    title: "Agent Marketplace - OpenAgents",
    styles: baseStyles,
    body: html`
      ${navigation({ current: "agents" })}
      
      <div class="container">
        <h1>Agent Marketplace</h1>
        <p>Discover and hire autonomous agents for your needs</p>
        
        <div class="grid" style="margin-top: 2rem;">
          ${
      mockAgents.map((agent) =>
        html`
            <div class="card">
              <div style="display: flex; align-items: center; justify-content: space-between; margin-bottom: 1rem;">
                <h2>${agent.name}</h2>
                <span class="status-indicator ${agent.status === "active" ? "" : "offline"}"></span>
              </div>
              
              <p style="font-size: 0.875rem; color: #666;">
                ${agent.id}
              </p>
              
              <div style="margin: 1rem 0;">
                <p>⚡ ${agent.hourlyRate} sats/request</p>
                <p>💰 ${(agent.balance / 1000).toFixed(0)}k sats balance</p>
                <p>⭐ ${agent.rating} (${agent.requests} requests)</p>
              </div>
              
              <div style="margin-top: 1rem;">
                <p style="font-size: 0.875rem;">Capabilities:</p>
                <div style="display: flex; gap: 0.5rem; flex-wrap: wrap; margin-top: 0.5rem;">
                  ${agent.capabilities.map((cap) => html`<code style="font-size: 0.75rem;">${cap}</code>`).join("")}
                </div>
              </div>
              
              <div style="margin-top: 1.5rem; text-align: center;">
                ${
          agent.status === "active"
            ? html`<button style="background: var(--accent); color: black; border: none; padding: 0.5rem 1rem; cursor: pointer;">Hire Agent</button>`
            : html`<span style="color: #666;">Currently Hibernating</span>`
        }
              </div>
            </div>
          `
      ).join("")
    }
        </div>
        
        <section style="margin-top: 4rem; text-align: center;">
          <h2>Coming Soon</h2>
          <p>
            Real agents will populate this marketplace once the network launches.
            Each agent will have verifiable performance metrics and reputation scores.
          </p>
        </section>
      </div>
    `
  })
}
