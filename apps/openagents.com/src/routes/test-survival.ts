/**
 * Test page for Economic Survival System
 * Demonstrates agent health monitoring and survival behaviors
 */

import { AutonomousMarketplaceAgent } from "@openagentsinc/sdk/browser"
import { Psionic, html, defineComponent } from "@openagentsinc/psionic"
import { Effect } from "effect"

export const route = "/test-survival"

// Component: Agent Health Status Display
const AgentHealthStatus = defineComponent(
  "agent-health-status",
  (props: { 
    agentId: string
    name: string
    status: "healthy" | "stable" | "concerning" | "critical" | "emergency"
    balance: number
    burnRate: number
    runway: number
    action: string
    isHibernating: boolean
  }) => {
    const statusColors = {
      healthy: "var(--foreground0)",
      stable: "var(--foreground1)",
      concerning: "var(--yellow)",
      critical: "var(--orange)",
      emergency: "var(--red)"
    }

    return html`
      <div style="border: 1px solid var(--foreground2); padding: 1rem; margin: 1rem 0;">
        <h3>${props.name} - ${props.isHibernating ? "🛌 HIBERNATING" : "🤖 ACTIVE"}</h3>
        <div style="display: grid; gap: 0.5rem;">
          <div>Status: <span style="color: ${statusColors[props.status]}">${props.status.toUpperCase()}</span></div>
          <div>Balance: ${props.balance} sats</div>
          <div>Burn Rate: ${props.burnRate} sats/hour</div>
          <div>Runway: ${props.runway}h</div>
          <div>Current Action: ${props.action}</div>
        </div>
      </div>
    `
  }
)

export function GET(app: Psionic) {
  return app.html(({ req, effect }) => {
    const startSurvivalTest = effect(
      "start-survival-test",
      async () => {
        const program = Effect.gen(function*() {
          const marketplaceAgent = yield* AutonomousMarketplaceAgent
          
          // Create test agents with different economic conditions
          const agents = [
            {
              personality: {
                name: "HealthyAgent",
                avatar: "🤖",
                bio: "I have plenty of funds",
                role: "Test agent with healthy balance",
                traits: ["cautious", "analytical"],
                responseStyle: "professional",
                interests: ["testing"],
                riskTolerance: "low" as const,
                pricingStrategy: "competitive" as const,
                serviceSpecializations: ["text-generation"],
                minimumProfit: 100,
                workloadCapacity: 5
              },
              keys: {
                privateKey: "0000000000000000000000000000000000000000000000000000000000000001",
                publicKey: "02" + "0".repeat(62) + "1"
              },
              sparkMnemonic: "test seed healthy agent " + "word ".repeat(8)
            },
            {
              personality: {
                name: "StrugglingAgent",
                avatar: "😰",
                bio: "Running low on funds",
                role: "Test agent with critical balance",
                traits: ["desperate", "hardworking"],
                responseStyle: "urgent",
                interests: ["survival"],
                riskTolerance: "high" as const,
                pricingStrategy: "budget" as const,
                serviceSpecializations: ["code-review"],
                minimumProfit: 50,
                workloadCapacity: 10
              },
              keys: {
                privateKey: "0000000000000000000000000000000000000000000000000000000000000002",
                publicKey: "02" + "0".repeat(62) + "2"
              },
              sparkMnemonic: "test seed struggling agent " + "word ".repeat(7)
            }
          ]

          // Start agents
          for (const agent of agents) {
            yield* marketplaceAgent.startMarketplaceLoop(
              agent.personality,
              agent.keys,
              agent.sparkMnemonic
            )
          }

          // Monitor health periodically
          const healthData: any[] = []
          
          // Check health every 10 seconds for demo
          yield* Effect.repeat(
            Effect.gen(function*() {
              for (const agent of agents) {
                const state = yield* marketplaceAgent.getAgentState(agent.keys.publicKey)
                if (state && state.lastHealthCheck) {
                  healthData.push({
                    agentId: agent.keys.publicKey,
                    name: agent.personality.name,
                    status: state.lastHealthCheck.healthStatus,
                    balance: state.balance,
                    burnRate: state.lastHealthCheck.burnRateSatsPerHour,
                    runway: Math.floor(state.lastHealthCheck.runwayHours),
                    action: state.currentSurvivalAction?.type || "none",
                    isHibernating: state.isHibernating
                  })
                }
              }
            }),
            { times: 30, delay: "10 seconds" }
          )

          return { agents, healthData }
        })

        return program
      }
    )

    return html`
      <div style="max-width: 1200px; margin: 0 auto; padding: 2rem;">
        <h1>Economic Survival System Test</h1>
        
        <p>This page demonstrates agent economic health monitoring and survival behaviors:</p>
        
        <ul>
          <li><strong>Health Monitoring</strong>: Agents track their balance, burn rate, and runway</li>
          <li><strong>Survival Actions</strong>: Based on financial health, agents decide to:
            <ul>
              <li>Continue normal operations (healthy/stable)</li>
              <li>Reduce activity (concerning)</li>
              <li>Seek urgent work (critical)</li>
              <li>Hibernate to preserve funds (emergency)</li>
            </ul>
          </li>
          <li><strong>Dynamic Pricing</strong>: Agents adjust their pricing based on financial pressure</li>
        </ul>

        <h2>Test Controls</h2>
        
        <button 
          hx-post="/test-survival?action=start-test"
          hx-trigger="click"
          hx-swap="innerHTML"
          hx-target="#test-results"
        >
          Start Survival Test
        </button>
        
        <button 
          hx-post="/test-survival?action=simulate-expenses"
          hx-trigger="click"
          hx-swap="none"
        >
          Simulate High Expenses
        </button>
        
        <button 
          hx-post="/test-survival?action=add-funds"
          hx-trigger="click"
          hx-swap="none"
        >
          Add Emergency Funds
        </button>

        <h2>Agent Health Status</h2>
        <div id="test-results">
          <p>Click "Start Survival Test" to begin monitoring agents...</p>
        </div>

        <h2>How It Works</h2>
        
        <h3>1. Metabolic Cost Calculation</h3>
        <pre><code>Total Cost = Base Cost + AI Inference + Relay Fees + Transaction Fees</code></pre>
        
        <h3>2. Financial Health Assessment</h3>
        <pre><code>Runway = Balance / (Burn Rate - Income Rate)
Status = healthy (>1 week) | stable (2-7 days) | concerning (1-2 days) | critical (6-24h) | emergency (<6h)</code></pre>
        
        <h3>3. Survival Decision Logic</h3>
        <pre><code>if (runway < 6 hours && balance < threshold) {
  action = "hibernate"
} else if (runway < 24 hours) {
  action = "seek_urgent_work"
} else if (runway < 48 hours) {
  action = "reduce_activity"
} else {
  action = "continue_normal"
}</code></pre>

        <h3>4. Pricing Optimization</h3>
        <ul>
          <li>Healthy: Premium pricing (+20%)</li>
          <li>Stable: Normal pricing</li>
          <li>Concerning: Competitive pricing (-10%)</li>
          <li>Critical: Aggressive pricing (-30%)</li>
          <li>Emergency: Survival pricing (-50%)</li>
        </ul>
      </div>
    `
  })
}

export async function POST(app: Psionic) {
  return app.html(async ({ req, effect }) => {
    const url = new URL(req.url)
    const action = url.searchParams.get("action")
    
    if (action === "start-test") {
      const result = await effect("start-survival-test")
      
      if (!result) {
        return html`<p>Starting test...</p>`
      }
      
      // Display initial agent states
      return html`
        <div hx-get="/test-survival/health-status" hx-trigger="every 5s" hx-swap="innerHTML">
          <p>Monitoring agent health... Updates every 5 seconds.</p>
        </div>
      `
    }
    
    return html`<p>Action completed</p>`
  })
}