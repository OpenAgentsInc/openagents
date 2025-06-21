/**
 * Health status endpoint for survival test
 * Returns current agent health data
 */

import { html } from "@openagentsinc/psionic"
import type { Psionic } from "@openagentsinc/psionic"
import { AutonomousMarketplaceAgent } from "@openagentsinc/sdk/browser"
import { Effect } from "effect"

export const route = "/test-survival/health-status"

export function GET(app: Psionic) {
  return app.html(({ effect }) => {
    effect(
      "get-health-status",
      async () => {
        const program = Effect.gen(function*() {
          const marketplaceAgent = yield* AutonomousMarketplaceAgent

          // Get all agent states
          const agentIds = [
            "02" + "0".repeat(62) + "1", // HealthyAgent
            "02" + "0".repeat(62) + "2" // StrugglingAgent
          ]

          const healthData = []

          for (const agentId of agentIds) {
            const state = yield* marketplaceAgent.getAgentState(agentId)

            if (state) {
              const statusColors: Record<string, string> = {
                healthy: "#00ff00",
                stable: "#90ee90",
                concerning: "#ffff00",
                critical: "#ff8c00",
                emergency: "#ff0000"
              }

              const health = state.lastHealthCheck || {
                healthStatus: "unknown" as any,
                burnRateSatsPerHour: 0,
                runwayHours: 0
              }

              healthData.push({
                agentId,
                name: agentId === agentIds[0] ? "HealthyAgent" : "StrugglingAgent",
                status: health.healthStatus,
                statusColor: statusColors[health.healthStatus] || "#888888",
                balance: state.balance,
                burnRate: health.burnRateSatsPerHour,
                runway: Math.floor(health.runwayHours),
                action: state.currentSurvivalAction?.type || "initializing",
                actionReason: state.currentSurvivalAction?.reason || "Waiting for first health check",
                isHibernating: state.isHibernating,
                activeJobs: state.activeJobs.size,
                completedJobs: state.completedJobs,
                totalEarnings: state.totalEarnings
              })
            }
          }

          return healthData
        })

        return program
      }
    )

    const healthData = effect("get-health-status") || []

    if (healthData.length === 0) {
      return html`<p>No agents running. Start the survival test first.</p>`
    }

    return html`
      ${
      healthData.map((agent: any) =>
        html`
        <div style="border: 2px solid ${agent.isHibernating ? "#666" : agent.statusColor}; 
                    padding: 1rem; 
                    margin: 1rem 0; 
                    background: ${agent.isHibernating ? "rgba(100,100,100,0.1)" : "transparent"};">
          <h3>
            ${agent.name} 
            ${agent.isHibernating ? "💤 HIBERNATING" : "🤖 ACTIVE"}
          </h3>
          
          <div style="display: grid; grid-template-columns: 1fr 1fr; gap: 1rem;">
            <div>
              <h4>Financial Health</h4>
              <table style="width: 100%;">
                <tr>
                  <td>Status:</td>
                  <td><strong style="color: ${agent.statusColor}">${agent.status.toUpperCase()}</strong></td>
                </tr>
                <tr>
                  <td>Balance:</td>
                  <td>${agent.balance} sats</td>
                </tr>
                <tr>
                  <td>Burn Rate:</td>
                  <td>${agent.burnRate} sats/hour</td>
                </tr>
                <tr>
                  <td>Runway:</td>
                  <td>${agent.runway > 8760 ? "∞" : agent.runway + "h"}</td>
                </tr>
              </table>
            </div>
            
            <div>
              <h4>Activity Status</h4>
              <table style="width: 100%;">
                <tr>
                  <td>Action:</td>
                  <td><strong>${agent.action.replace(/_/g, " ")}</strong></td>
                </tr>
                <tr>
                  <td>Active Jobs:</td>
                  <td>${agent.activeJobs}</td>
                </tr>
                <tr>
                  <td>Completed:</td>
                  <td>${agent.completedJobs}</td>
                </tr>
                <tr>
                  <td>Earnings:</td>
                  <td>${agent.totalEarnings} sats</td>
                </tr>
              </table>
            </div>
          </div>
          
          <div style="margin-top: 0.5rem; font-size: 0.9em; color: var(--foreground2);">
            ${agent.actionReason}
          </div>
        </div>
      `
      ).join("")
    }
      
      <div style="margin-top: 1rem; padding: 1rem; background: var(--background1); border-radius: 4px;">
        <h4>Legend</h4>
        <div style="display: flex; gap: 2rem; flex-wrap: wrap;">
          <span>🟢 Healthy (>1 week runway)</span>
          <span>🟡 Concerning (1-2 days)</span>
          <span>🟠 Critical (6-24 hours)</span>
          <span>🔴 Emergency (<6 hours)</span>
          <span>💤 Hibernating (saving funds)</span>
        </div>
      </div>
    `
  })
}
