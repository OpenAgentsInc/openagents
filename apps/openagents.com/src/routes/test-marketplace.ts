/**
 * Test page for autonomous marketplace agents
 * Demonstrates agent service discovery, bidding, and delivery
 */

import { html } from "@openagentsinc/psionic"

export default function testMarketplace() {
  return html`
    <div class="test-marketplace">
      <h1>Autonomous Marketplace Agent Test</h1>
      
      <div class="controls">
        <button id="spawn-marketplace-agent">Spawn Marketplace Agent</button>
        <button id="create-job-request">Create Test Job Request</button>
        <button id="stop-all">Stop All Agents</button>
      </div>
      
      <div class="status" id="status">
        <p>Ready to test marketplace agents...</p>
      </div>
      
      <div class="activity-log" id="activity-log">
        <h3>Activity Log</h3>
        <div id="log-entries"></div>
      </div>
    </div>
    
    <script type="module">
      import { Effect, Runtime, Layer } from "https://esm.sh/effect@3.10.3"
      import * as SDK from "@openagentsinc/sdk/browser"
      import * as NostrLib from "@openagentsinc/nostr"
      
      const logActivity = (message) => {
        const logEntries = document.getElementById('log-entries')
        const entry = document.createElement('div')
        entry.className = 'log-entry'
        entry.textContent = \`[\${new Date().toLocaleTimeString()}] \${message}\`
        logEntries.appendChild(entry)
        logEntries.scrollTop = logEntries.scrollHeight
      }
      
      let activeAgents = []
      
      // Spawn marketplace agent button
      document.getElementById('spawn-marketplace-agent').addEventListener('click', async () => {
        try {
          logActivity('Generating agent keys...')
          const mnemonic = await SDK.Agent.generateMnemonic()
          const agent = await SDK.Agent.createFromMnemonic(mnemonic, {
            name: \`MarketAgent-\${Date.now().toString(36)}\`
          })
          
          logActivity(\`Created agent: \${agent.name} (\${agent.nostrKeys.public.slice(0, 16)}...)\`)
          
          // Create marketplace personality
          const personality = {
            // Base personality
            name: agent.name,
            role: "analyst",
            traits: ["analytical", "efficient", "reliable"],
            responseStyle: "concise",
            topics: ["code-review", "text-analysis", "data-processing"],
            chattiness: 0.3,
            temperature: 0.5,
            
            // Marketplace extension
            riskTolerance: "medium",
            pricingStrategy: "competitive",
            serviceSpecializations: ["code-review", "text-generation", "summarization"],
            minimumProfit: 100,
            workloadCapacity: 3
          }
          
          logActivity(\`Agent personality: \${personality.role} specializing in \${personality.serviceSpecializations.join(", ")}\`)
          
          // Start marketplace loop
          const program = Effect.gen(function*() {
            const marketplaceAgent = yield* SDK.AutonomousMarketplaceAgent
            
            yield* marketplaceAgent.startMarketplaceLoop(personality, {
              privateKey: agent.nostrKeys.private,
              publicKey: agent.nostrKeys.public
            })
            
            logActivity(\`Agent \${agent.name} is now monitoring marketplace for jobs\`)
          })
          
          // Create runtime with services
          const runtime = Runtime.defaultRuntime
          const layer = SDK.createBrowserServicesLayer({
            model: "@cf/meta/llama-3.1-8b-instruct"
          })
          
          await Runtime.runPromise(runtime)(
            program.pipe(Effect.provide(layer))
          )
          
          activeAgents.push(agent)
          
        } catch (error) {
          console.error('Error spawning agent:', error)
          logActivity(\`Error: \${error.message}\`)
        }
      })
      
      // Create job request button
      document.getElementById('create-job-request').addEventListener('click', async () => {
        try {
          logActivity('Creating test job request...')
          
          // Generate requester keys
          const requesterMnemonic = await SDK.Agent.generateMnemonic()
          const requester = await SDK.Agent.createFromMnemonic(requesterMnemonic, {
            name: "TestRequester"
          })
          
          const jobRequest = {
            jobId: \`job_\${Date.now()}_\${Math.random().toString(36).substring(2, 10)}\`,
            serviceId: "test-service",
            requestKind: NostrLib.Nip90Service.NIP90_JOB_REQUEST_KINDS.CODE_REVIEW,
            input: \`Please review this TypeScript code:
\\\`\\\`\\\`typescript
function calculateSum(numbers: number[]): number {
  let sum = 0
  for (let i = 0; i <= numbers.length; i++) {
    sum += numbers[i]
  }
  return sum
}
\\\`\\\`\\\`
What issues do you see?\`,
            inputType: "text",
            bidAmount: 500,
            requester: requester.nostrKeys.public,
            provider: "" // Open to any provider
          }
          
          logActivity(\`Created job request: \${jobRequest.jobId} for code review (500 sats)\`)
          
          // Publish job request
          const program = Effect.gen(function*() {
            const nip90 = yield* NostrLib.Nip90Service.Nip90Service
            
            const jobEvent = yield* nip90.requestJob({
              serviceId: jobRequest.serviceId,
              requestKind: jobRequest.requestKind,
              input: jobRequest.input,
              inputType: jobRequest.inputType,
              bidAmount: jobRequest.bidAmount,
              providerPubkey: "",
              privateKey: requester.nostrKeys.private
            })
            
            logActivity(\`Published job request to marketplace: \${jobEvent.id}\`)
          })
          
          // Create runtime with Nostr services
          const runtime = Runtime.defaultRuntime
          const nostrLayer = Layer.mergeAll(
            NostrLib.CryptoService.CryptoServiceLive,
            NostrLib.EventService.EventServiceLive,
            NostrLib.RelayService.RelayServiceLive.pipe(
              Layer.provide(NostrLib.WebSocketService.WebSocketServiceLive)
            ),
            NostrLib.Nip90Service.Nip90ServiceLive.pipe(
              Layer.provide(Layer.merge(
                NostrLib.EventService.EventServiceLive,
                NostrLib.RelayService.RelayServiceLive.pipe(
                  Layer.provide(NostrLib.WebSocketService.WebSocketServiceLive)
                )
              ))
            )
          )
          
          await Runtime.runPromise(runtime)(
            program.pipe(Effect.provide(nostrLayer))
          )
          
        } catch (error) {
          console.error('Error creating job:', error)
          logActivity(\`Error: \${error.message}\`)
        }
      })
      
      // Stop all agents button
      document.getElementById('stop-all').addEventListener('click', async () => {
        try {
          logActivity('Stopping all agents...')
          
          const program = Effect.gen(function*() {
            const marketplaceAgent = yield* SDK.AutonomousMarketplaceAgent
            
            for (const agent of activeAgents) {
              yield* marketplaceAgent.stopMarketplaceLoop(agent.nostrKeys.public)
              logActivity(\`Stopped agent: \${agent.name}\`)
            }
          })
          
          const runtime = Runtime.defaultRuntime
          const layer = SDK.createBrowserServicesLayer()
          
          await Runtime.runPromise(runtime)(
            program.pipe(Effect.provide(layer))
          )
          
          activeAgents = []
          logActivity('All agents stopped')
          
        } catch (error) {
          console.error('Error stopping agents:', error)
          logActivity(\`Error: \${error.message}\`)
        }
      })
      
      logActivity('Marketplace test interface ready')
    </script>
    
    <style>
      .test-marketplace {
        max-width: 1200px;
        margin: 0 auto;
        padding: 2rem;
      }
      
      .controls {
        display: flex;
        gap: 1rem;
        margin-bottom: 2rem;
      }
      
      .controls button {
        padding: 0.75rem 1.5rem;
        background: var(--foreground0);
        color: var(--background0);
        border: none;
        cursor: pointer;
        font-family: inherit;
      }
      
      .controls button:hover {
        opacity: 0.8;
      }
      
      .status {
        background: var(--background1);
        padding: 1rem;
        margin-bottom: 2rem;
        border: 1px solid var(--background3);
      }
      
      .activity-log {
        background: var(--background0);
        border: 1px solid var(--background3);
        padding: 1rem;
        height: 400px;
        overflow-y: auto;
      }
      
      .log-entry {
        font-family: monospace;
        font-size: 0.875rem;
        padding: 0.25rem 0;
        color: var(--foreground1);
      }
      
      #log-entries {
        max-height: 350px;
        overflow-y: auto;
      }
    </style>
  `
}
