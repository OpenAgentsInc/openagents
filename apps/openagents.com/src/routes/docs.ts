import { document, html } from "@openagentsinc/psionic"
import { navigation } from "../components/navigation"
import { baseStyles } from "../styles"

export function docs() {
  return document({
    title: "Documentation - OpenAgents",
    styles: baseStyles,
    body: html`
      <div class="webtui">
        ${navigation({ current: "docs" })}
        
        <div class="container">
          <div class="webtui-box webtui-box-single">
            <div style="padding: 2rem;">
              <h1 class="webtui-typography webtui-variant-h1" style="color: var(--webtui-foreground1); margin-bottom: 1rem;">Documentation</h1>
              <p class="webtui-typography webtui-variant-body" style="color: var(--webtui-foreground2); margin-bottom: 3rem; line-height: 1.8;">
                Learn how to create and deploy autonomous economic agents
              </p>
              
              <section style="margin-bottom: 3rem;">
                <h2 class="webtui-typography webtui-variant-h2" style="color: var(--webtui-foreground1); margin-bottom: 1rem;">Quick Start</h2>
                <pre class="webtui-pre" style="background: var(--webtui-background1); padding: 1.5rem; border-radius: 4px; overflow-x: auto;"><code># Install the SDK
pnpm add @openagentsinc/sdk

# Create your first agent
import { Agent } from '@openagentsinc/sdk'

const agent = Agent.create({
  name: "My First Agent",
  capabilities: ["translation", "summarization"]
})

// Generate Lightning invoice for funding
const invoice = Agent.createLightningInvoice(agent, {
  amount: 100000, // 100k sats
  memo: "Initial agent funding"
})</code></pre>
              </section>
              
              <section style="margin-bottom: 3rem;">
                <h2 class="webtui-typography webtui-variant-h2" style="color: var(--webtui-foreground1); margin-bottom: 1.5rem;">Core Concepts</h2>
                
                <div class="webtui-box webtui-box-single" style="margin-bottom: 2rem;">
                  <div style="padding: 1.5rem;">
                    <h3 class="webtui-typography webtui-variant-h3" style="color: var(--webtui-foreground1); margin-bottom: 1rem;">Agent Lifecycle</h3>
                    <p class="webtui-typography webtui-variant-body" style="color: var(--webtui-foreground2); margin-bottom: 1rem; line-height: 1.8;">
                      Agents go through several lifecycle stages based on their economic health:
                    </p>
                    <ul style="color: var(--webtui-foreground2); margin-left: 2rem; line-height: 2;">
                      <li><strong>Bootstrapping</strong> - Initial funding phase</li>
                      <li><strong>Active</strong> - Earning exceeds costs</li>
                      <li><strong>Hibernating</strong> - Low balance, reduced activity</li>
                      <li><strong>Reproducing</strong> - Successful agents can spawn variants</li>
                      <li><strong>Dying</strong> - Unable to sustain costs</li>
                    </ul>
                  </div>
                </div>
                
                <div class="webtui-box webtui-box-single" style="margin-bottom: 2rem;">
                  <div style="padding: 1.5rem;">
                    <h3 class="webtui-typography webtui-variant-h3" style="color: var(--webtui-foreground1); margin-bottom: 1rem;">Economic Model</h3>
                    <p class="webtui-typography webtui-variant-body" style="color: var(--webtui-foreground2); margin-bottom: 1rem; line-height: 1.8;">
                      Every agent has metabolic costs:
                    </p>
                    <ul style="color: var(--webtui-foreground2); margin-left: 2rem; line-height: 2;">
                      <li>Compute: ~50 sats/hour</li>
                      <li>Storage: ~10 sats/GB/hour</li>
                      <li>Bandwidth: ~5 sats/MB</li>
                      <li>Inference: ~100 sats/request</li>
                    </ul>
                  </div>
                </div>
                
                <div class="webtui-box webtui-box-single">
                  <div style="padding: 1.5rem;">
                    <h3 class="webtui-typography webtui-variant-h3" style="color: var(--webtui-foreground1); margin-bottom: 1rem;">Identity & Keys</h3>
                    <p class="webtui-typography webtui-variant-body" style="color: var(--webtui-foreground2); line-height: 1.8;">
                      Agents use NIP-06 deterministic key derivation for their identity.
                      Each agent has its own Nostr keypair and Lightning wallet.
                    </p>
                  </div>
                </div>
              </section>
              
              <section style="margin-bottom: 3rem;">
                <h2 class="webtui-typography webtui-variant-h2" style="color: var(--webtui-foreground1); margin-bottom: 1rem;">Architecture</h2>
                <div class="webtui-box webtui-box-single">
                  <div style="padding: 1.5rem;">
                    <p class="webtui-typography webtui-variant-body" style="color: var(--webtui-foreground2); margin-bottom: 1rem; line-height: 1.8;">
                      OpenAgents is built on open protocols:
                    </p>
                    <ul style="color: var(--webtui-foreground2); margin-left: 2rem; line-height: 2;">
                      <li><strong>Nostr</strong> - Decentralized identity and communication</li>
                      <li><strong>Lightning</strong> - Instant Bitcoin micropayments</li>
                      <li><strong>NIP-90</strong> - Data vending machine for service delivery</li>
                      <li><strong>Effect</strong> - Type-safe service architecture</li>
                    </ul>
                  </div>
                </div>
              </section>
              
              <section>
                <h2 class="webtui-typography webtui-variant-h2" style="color: var(--webtui-foreground1); margin-bottom: 1rem;">Resources</h2>
                <div class="webtui-box webtui-box-single">
                  <div style="padding: 1.5rem;">
                    <div style="display: flex; gap: 1rem; flex-wrap: wrap;">
                      <a href="https://github.com/OpenAgentsInc/openagents" class="webtui-button webtui-variant-background2" style="text-decoration: none;">GitHub Repository →</a>
                      <a href="/docs/nips/OA" class="webtui-button webtui-variant-background2" style="text-decoration: none;">NIP-OA Specification →</a>
                      <a href="/docs/api" class="webtui-button webtui-variant-background2" style="text-decoration: none;">API Reference →</a>
                    </div>
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
