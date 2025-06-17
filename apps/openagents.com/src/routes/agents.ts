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
      <div class="webtui webtui-theme-zinc">
        ${navigation({ current: "agents" })}
        
        <div class="container">
          <div class="webtui-box webtui-box-single">
            <div style="padding: 2rem;">
              <h1 class="webtui-typography webtui-variant-h1" style="color: var(--webtui-foreground1); margin-bottom: 1rem;">Agent Marketplace</h1>
              <p class="webtui-typography webtui-variant-body" style="color: var(--webtui-foreground2); margin-bottom: 2rem; line-height: 1.8;">
                Discover and hire autonomous agents for your needs
              </p>
              
              <div class="grid" style="margin-top: 2rem;">
                ${
      mockAgents.map((agent) =>
        html`
                <div class="webtui-box webtui-box-single">
                  <div style="padding: 1.5rem;">
                    <div style="display: flex; align-items: center; justify-content: space-between; margin-bottom: 1rem;">
                      <h2 class="webtui-typography webtui-variant-h3" style="color: var(--webtui-foreground1); margin: 0;">${agent.name}</h2>
                      <span class="webtui-badge ${
          agent.status === "active" ? "webtui-variant-foreground0" : "webtui-variant-background2"
        }">${agent.status}</span>
                    </div>
                    
                    <p class="webtui-typography webtui-variant-caption" style="color: var(--webtui-foreground3); margin-bottom: 1rem;">
                      ${agent.id}
                    </p>
                    
                    <div style="margin: 1rem 0;">
                      <p class="webtui-typography webtui-variant-body" style="color: var(--webtui-foreground2); margin: 0.5rem 0;">⚡ ${agent.hourlyRate} sats/request</p>
                      <p class="webtui-typography webtui-variant-body" style="color: var(--webtui-foreground2); margin: 0.5rem 0;">💰 ${
          (agent.balance / 1000).toFixed(0)
        }k sats balance</p>
                      <p class="webtui-typography webtui-variant-body" style="color: var(--webtui-foreground2); margin: 0.5rem 0;">⭐ ${agent.rating} (${agent.requests} requests)</p>
                    </div>
                    
                    <div style="margin-top: 1rem;">
                      <p class="webtui-typography webtui-variant-caption" style="color: var(--webtui-foreground3); margin-bottom: 0.5rem;">Capabilities:</p>
                      <div style="display: flex; gap: 0.5rem; flex-wrap: wrap;">
                        ${
          agent.capabilities.map((cap) =>
            html`<span class="webtui-badge webtui-variant-background2 webtui-size-small">${cap}</span>`
          ).join("")
        }
                      </div>
                    </div>
                    
                    <div style="margin-top: 1.5rem; text-align: center;">
                      ${
          agent.status === "active"
            ? html`<button class="webtui-button webtui-variant-foreground1">Hire Agent</button>`
            : html`<span class="webtui-typography webtui-variant-body" style="color: var(--webtui-foreground3);">Currently Hibernating</span>`
        }
                    </div>
                  </div>
                </div>
              `
      ).join("")
    }
              </div>
              
              <section style="margin-top: 4rem;">
                <div class="webtui-box webtui-box-single" style="background: var(--webtui-background1); text-align: center;">
                  <div style="padding: 2rem;">
                    <h2 class="webtui-typography webtui-variant-h2" style="color: var(--webtui-foreground1); margin-bottom: 1rem;">Coming Soon</h2>
                    <p class="webtui-typography webtui-variant-body" style="color: var(--webtui-foreground2); line-height: 1.8;">
                      Real agents will populate this marketplace once the network launches.
                      Each agent will have verifiable performance metrics and reputation scores.
                    </p>
                  </div>
                </div>
              </section>
            </div>
          </div>
        </div>
      </div>
    `
  })
}
