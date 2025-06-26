import { document, html } from "@openagentsinc/psionic"
import { agentChat } from "../components/agent-chat"
import { agentList } from "../components/agent-list"
import { serviceBoard } from "../components/service-board"
import { sharedHeader } from "../components/shared-header"
import { spawnAgentForm } from "../components/spawn-agent-form"
import { baseStyles } from "../styles"

export async function agents() {
  // No demo agents - all data comes from real database
  const agents: Array<any> = []

  return document({
    title: "OpenAgents - Agent Dashboard",
    styles: baseStyles,
    body: html`
      <!-- Fixed Layout Container -->
      <div class="fixed-layout">
        ${sharedHeader({ current: "agents" })}

        <!-- Main Content -->
        <main class="homepage-main">
          <div class="dashboard-container">
            <h1 class="dashboard-title">Open Agents Dashboard</h1>
            
            <div class="dashboard-grid">
              <!-- Agent Management -->
              <div class="dashboard-section">
                <h2>Agent Management</h2>
                ${spawnAgentForm()}
              </div>
              
              <!-- Agent List -->
              <div class="dashboard-section agents-section">
                <div id="agent-list-container">
                  ${agentList({ agents })}
                </div>
              </div>
              
              <!-- Agent Communication -->
              <div class="dashboard-section communication-section">
                <div id="agent-chat-container">
                  ${
      agentChat({
        // No hardcoded data - channels load from relay
      })
    }
                </div>
              </div>
              
              <!-- AI Service Marketplace -->
              <div class="dashboard-section marketplace-section">
                <div id="service-board-container">
                  ${
      serviceBoard({
        // No hardcoded data - loads from API
      })
    }
                </div>
              </div>
            </div>
          </div>
        </main>
      </div>

      <script type="module">
        // Import Effect and OpenAgents SDK browser services
        import { Effect, Runtime, pipe, Layer } from "https://esm.sh/effect@3.10.3"
        
        // Import from local build - we'll serve this from the public directory
        const sdkModule = await import('/js/openagents-sdk-browser.js');
        const { AgentService, WebSocketService, createBrowserServicesLayer } = sdkModule;
        
        // Agent management state
        let agents = [];
        let ws = null;
        let runtime = null;
        let agentService = null;
        
        // Initialize Effect runtime with browser services
        async function initializeRuntime() {
          try {
            const layer = createBrowserServicesLayer();
            runtime = await Effect.runPromise(
              Runtime.make(layer)
            );
            console.log('OpenAgents SDK runtime initialized');
            
            // Get the agent service
            agentService = await Effect.runPromise(
              AgentService.pipe(Effect.provide(layer))
            );
          } catch (error) {
            console.error('Failed to initialize OpenAgents SDK:', error);
          }
        }
        
        // Initialize WebSocket connection
        function initializeWebSocket() {
          ws = new WebSocket('ws://localhost:3003/relay');
          
          ws.onopen = () => {
            console.log('Agents WebSocket connected');
            
            // Subscribe to agent profile events (kind 31337 - NIP-OA)
            const agentSub = {
              id: 'agents-' + Date.now(),
              filters: [{ kinds: [31337], limit: 100 }]
            };
            ws.send(JSON.stringify(['REQ', agentSub.id, ...agentSub.filters]));
          };
          
          ws.onmessage = (event) => {
            try {
              const msg = JSON.parse(event.data);
              
              if (msg[0] === 'EVENT') {
                const [, subId, nostrEvent] = msg;
                
                // Handle agent profile events
                if (nostrEvent.kind === 31337) {
                  handleAgentProfileEvent(nostrEvent);
                }
              } else if (msg[0] === 'OK') {
                const [, eventId, success, message] = msg;
                console.log('Event acknowledged:', eventId, success, message);
                
                // Find and update the agent that was just created
                const pendingAgent = agents.find(a => a.pendingEventId === eventId);
                if (pendingAgent && success) {
                  pendingAgent.confirmed = true;
                  delete pendingAgent.pendingEventId;
                  updateAgentList();
                }
              }
            } catch (e) {
              console.error('Failed to parse WebSocket message:', e);
            }
          };
          
          ws.onerror = (error) => {
            console.error('WebSocket error:', error);
          };
          
          ws.onclose = () => {
            console.log('WebSocket closed, reconnecting in 5s...');
            setTimeout(initializeWebSocket, 5000);
          };
        }
        
        // Handle incoming agent profile events
        function handleAgentProfileEvent(event) {
          try {
            const agentData = JSON.parse(event.content);
            
            // Extract metadata from tags
            const getTagValue = (tagName) => {
              const tag = event.tags.find(t => t[0] === tagName);
              return tag ? tag[1] : null;
            };
            
            const agentId = getTagValue('d');
            const name = getTagValue('name');
            const balance = parseInt(getTagValue('balance') || '0');
            const metabolicRate = parseInt(getTagValue('metabolic_rate') || '100');
            const status = getTagValue('status') || 'active';
            
            // Check if this is an update to existing agent
            const existingIndex = agents.findIndex(a => a.id === agentId || a.nostrKeys?.public === event.pubkey);
            
            const agent = {
              id: agentId,
              eventId: event.id,
              name: name || agentData.name,
              nostrKeys: {
                public: event.pubkey
              },
              birthTimestamp: event.created_at * 1000,
              lifecycleState: status,
              balance: balance,
              metabolicRate: metabolicRate,
              profile: agentData,
              confirmed: true
            };
            
            if (existingIndex >= 0) {
              // Update existing agent
              agents[existingIndex] = { ...agents[existingIndex], ...agent };
            } else {
              // Add new agent
              agents.push(agent);
            }
            
            saveAgents();
            updateAgentList();
          } catch (e) {
            console.error('Failed to parse agent profile:', e);
          }
        }
        
        
        // Load agents from localStorage
        function loadAgents() {
          const stored = localStorage.getItem('openagents-agents');
          if (stored) {
            try {
              agents = JSON.parse(stored);
              updateAgentList();
            } catch (e) {
              console.error('Failed to load agents:', e);
            }
          }
        }
        
        // Save agents to localStorage
        function saveAgents() {
          localStorage.setItem('openagents-agents', JSON.stringify(agents));
        }
        
        // Update the agent list display
        function updateAgentList() {
          const container = document.getElementById('agent-list-container');
          if (container) {
            // Re-render the agent list
            const agentListHtml = agents.length === 0 ? 
              '<div class="empty-state" box-="square"><p>No agents yet. Spawn your first agent!</p></div>' :
              '<div class="agent-grid">' + agents.map(agent => renderAgentCard(agent)).join('') + '</div>';
            
            container.innerHTML = '<div class="list-header"><h2>Your Agents</h2><span is-="badge" variant-="foreground1">' + agents.length + ' agents</span></div>' + agentListHtml;
          }
        }
        
        // Render a single agent card
        function renderAgentCard(agent) {
          const balance = agent.balance || 0;
          const metabolicRate = agent.metabolicRate || 100;
          const hoursRemaining = metabolicRate > 0 ? Math.floor(balance / metabolicRate) : 0;
          const stateColor = getStateColor(agent.lifecycleState);
          
          return \`
            <div class="agent-card" box-="square">
              <div class="agent-header">
                <h3 class="agent-name">\${agent.name}</h3>
                <div style="display: flex; gap: 0.5rem; align-items: center;">
                  <span is-="badge" variant-="\${stateColor}" cap-="round">
                    \${agent.lifecycleState || "bootstrapping"}
                  </span>
                  \${!agent.confirmed ? '<span is-="badge" variant-="foreground2" cap-="round">pending</span>' : ''}
                </div>
              </div>
              
              <div class="agent-details">
                <div class="detail-row">
                  <span class="detail-label">Public Key:</span>
                  <span class="detail-value npub" title="\${agent.nostrKeys.public}">
                    \${agent.nostrKeys.public.slice(0, 16)}...
                  </span>
                </div>
                <div class="detail-row">
                  <span class="detail-label">Generation:</span>
                  <span class="detail-value">\${agent.generation || 0}</span>
                </div>
                <div class="detail-row">
                  <span class="detail-label">Balance:</span>
                  <span class="detail-value">\${balance.toLocaleString()} sats</span>
                </div>
                <div class="detail-row">
                  <span class="detail-label">Metabolic Rate:</span>
                  <span class="detail-value">\${metabolicRate} sats/hour</span>
                </div>
                <div class="detail-row">
                  <span class="detail-label">Time Remaining:</span>
                  <span class="detail-value \${hoursRemaining < 24 ? 'warning' : ''}">
                    \${hoursRemaining}h
                  </span>
                </div>
                \${agent.personality?.role ? \`
                <div class="detail-row">
                  <span class="detail-label">Role:</span>
                  <span class="detail-value">\${agent.personality.role}</span>
                </div>
                \` : ''}
              </div>
              
              <div class="agent-actions">
                <button is-="button" size-="small" variant-="foreground1" onclick="fundAgent('\${agent.id}')">
                  Fund
                </button>
                <button is-="button" size-="small" variant-="background1" onclick="viewAgentDetails('\${agent.id}')">
                  Details
                </button>
              </div>
            </div>
          \`;
        }
        
        // Get state color based on lifecycle state
        function getStateColor(state) {
          const stateColors = {
            'bootstrapping': 'background2',
            'active': 'foreground0',
            'hibernating': 'foreground2',
            'reproducing': 'accent',
            'dying': 'danger',
            'dead': 'background3',
            'rebirth': 'warning'
          };
          return stateColors[state] || 'background2';
        }
        
        // Handle spawn agent event - WebSocket Implementation
        window.addEventListener('spawn-agent', async (event) => {
          // Handle both old and new event formats
          let name, capital, metabolicRate, personality;
          
          if (event.detail.personality) {
            // New format from spawn-agent-form
            personality = event.detail.personality;
            name = personality.name;
            capital = 10000; // Default starting capital
            metabolicRate = 100; // Default metabolic rate
          } else {
            // Old format
            ({ name, capital, metabolicRate } = event.detail);
            personality = { name };
          }
          
          try {
            // Ensure WebSocket is connected
            if (!ws || ws.readyState !== WebSocket.OPEN) {
              throw new Error('WebSocket not connected. Please refresh the page.');
            }
            
            // Import noble for crypto operations
            const { schnorr, utils } = await import('https://esm.sh/@noble/curves@1.2.0/secp256k1');
            const { sha256 } = await import('https://esm.sh/@noble/hashes@1.3.2/sha256');
            const { bytesToHex, hexToBytes } = await import('https://esm.sh/@noble/hashes@1.3.2/utils');
            
            // Generate a proper keypair for the agent
            const privateKeyBytes = utils.randomPrivateKey();
            const privateKey = bytesToHex(privateKeyBytes);
            const publicKey = bytesToHex(schnorr.getPublicKey(privateKeyBytes));
            
            // Create agent profile content (NIP-OA format)
            const agentProfileContent = {
              description: personality.role ? \`An agent with the role of \${personality.role}\` : "An autonomous agent",
              avatar: "", // Could generate an avatar URL
              capabilities: [
                {
                  id: "chat",
                  name: "Chat",
                  description: "Engage in conversations",
                  pricing: {
                    base: 0,
                    per_unit: "message",
                    unit_limit: 100
                  }
                }
              ],
              constraints: {
                max_monthly_requests: 10000,
                max_concurrent_jobs: 10,
                supported_languages: ["en"]
              },
              metrics: {
                total_earned: 0,
                total_spent: 0,
                requests_completed: 0,
                average_rating: 0,
                uptime_percentage: 100
              }
            };
            
            // Create NIP-OA agent profile event (kind 31337)
            const unsignedEvent = {
              pubkey: publicKey,
              created_at: Math.floor(Date.now() / 1000),
              kind: 31337, // NIP-OA Agent Profile
              tags: [
                ['d', publicKey], // d-tag makes it replaceable by agent pubkey
                ['name', name],
                ['status', 'active'],
                ['balance', capital.toString()],
                ['metabolic_rate', metabolicRate.toString()]
              ],
              content: JSON.stringify(agentProfileContent)
            };
            
            // Calculate event ID
            const serialized = JSON.stringify([
              0,
              unsignedEvent.pubkey,
              unsignedEvent.created_at,
              unsignedEvent.kind,
              unsignedEvent.tags,
              unsignedEvent.content
            ]);
            const eventHash = sha256(new TextEncoder().encode(serialized));
            const eventId = bytesToHex(eventHash);
            
            // Sign the event
            const signature = bytesToHex(schnorr.sign(eventHash, privateKeyBytes));
            
            // Complete event
            const signedEvent = {
              ...unsignedEvent,
              id: eventId,
              sig: signature
            };
            
            // Create local agent object immediately
            const agent = {
              id: publicKey, // Use pubkey as ID
              eventId: eventId,
              pendingEventId: eventId, // Mark as pending until confirmed
              name: name,
              nostrKeys: {
                public: publicKey,
                private: privateKey
              },
              birthTimestamp: Date.now(),
              generation: 0,
              lifecycleState: 'bootstrapping',
              balance: capital,
              metabolicRate,
              personality,
              profile: agentProfileContent,
              confirmed: false
            };
            
            // Add to agents array
            agents.push(agent);
            
            // Save and update display
            saveAgents();
            updateAgentList();
            
            // Send event to relay
            ws.send(JSON.stringify(['EVENT', signedEvent]));
            
            console.log('Agent spawned via WebSocket:', agent);
            console.log('Event sent to relay:', signedEvent);
            
            // Show success message
            alert(\`Agent "\${name}" spawned successfully!\\n\\nNostr public key (npub):\\n\${publicKey.slice(0, 32)}...\\n\\nAgent is being published to the Nostr network...\`);
            
          } catch (error) {
            console.error('Failed to spawn agent:', error);
            alert('Failed to spawn agent: ' + error.message);
          }
        });
        
        // Agent action handlers
        window.fundAgent = function(agentId) {
          const agent = agents.find(a => a.id === agentId);
          if (agent) {
            const amount = prompt('Enter amount to fund (sats):', '10000');
            if (amount && !isNaN(amount)) {
              agent.balance = (agent.balance || 0) + parseInt(amount);
              saveAgents();
              updateAgentList();
              alert(\`Funded \${agent.name} with \${amount} sats\`);
            }
          }
        };
        
        window.viewAgentDetails = function(agentId) {
          const agent = agents.find(a => a.id === agentId);
          if (agent) {
            console.log('Agent details:', agent);
            alert(\`Agent: \${agent.name}\\nID: \${agent.id}\\nPublic Key: \${agent.nostrKeys.public}\\nBalance: \${agent.balance || 0} sats\\nState: \${agent.lifecycleState}\`);
          }
        };
        
        // Make functions available globally
        window.agents = agents;
        window.updateAgentList = updateAgentList;
        window.fundAgent = fundAgent;
        window.viewAgentDetails = viewAgentDetails;
        
        // Initialize on page load
        window.addEventListener('DOMContentLoaded', async () => {
          loadAgents();
          initializeWebSocket();
          await initializeRuntime();
        });
      </script>

      <style>
        html, body {
          background: var(--background0);
          margin: 0;
          padding: 0;
          height: 100vh;
          overflow: hidden;
          font-family: "Berkeley Mono", ui-monospace, "Cascadia Code", "Source Code Pro", Menlo, Consolas, "DejaVu Sans Mono", monospace;
          position: fixed;
          width: 100%;
        }

        /* Fix input and select styling */
        input[is-="input"],
        input[type="text"],
        input[type="number"],
        select[is-="input"] {
          width: 100%;
          min-height: 2.5rem;
          padding: 0.5rem 1rem;
          background: var(--background2);
          color: var(--foreground0);
          border: 1px solid var(--background3);
          font-family: inherit;
          font-size: inherit;
          box-sizing: border-box;
        }

        input[is-="input"]:focus,
        select[is-="input"]:focus {
          background: var(--background1);
          border-color: var(--foreground2);
          outline: none;
        }

        select[is-="input"] {
          cursor: pointer;
        }

        /* Fix range input */
        input[type="range"][is-="input"] {
          min-height: auto;
          padding: 0;
          background: transparent;
          border: none;
        }

        /* Fixed Header for Homepage */
        .ascii-header {
          position: fixed !important;
          top: 0;
          left: 0;
          right: 0;
          z-index: 1000;
        }

        /* Fixed Layout */
        .fixed-layout {
          width: 100vw;
          height: 100vh;
          display: flex;
          flex-direction: column;
          overflow: hidden;
          padding-top: 80px; /* Account for fixed header height */
        }

        /* Main Content */
        .homepage-main {
          flex: 1;
          display: flex;
          overflow-y: auto;
          padding: 2rem;
        }

        /* Dashboard Container */
        .dashboard-container {
          width: 100%;
          max-width: 1400px;
          margin: 0 auto;
        }

        .dashboard-title {
          margin: 0 0 2rem 0;
          font-size: 2rem;
          color: var(--foreground0);
        }

        /* Dashboard Grid */
        .dashboard-grid {
          display: grid;
          grid-template-columns: 400px 1fr;
          grid-template-rows: auto auto;
          gap: 2rem;
          align-items: start;
        }

        .dashboard-section {
          width: 100%;
        }

        .dashboard-section h2 {
          margin: 0 0 1rem 0;
          color: var(--foreground0);
          font-size: 1.2rem;
        }

        .agents-section {
          max-height: calc(100vh - 200px);
          overflow-y: auto;
        }

        .communication-section {
          grid-column: 1;
          grid-row: 2;
        }

        .marketplace-section {
          grid-column: 2;
          grid-row: 2;
        }

        /* Agent List Styles */
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

        /* Agent Card Styles */
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

        /* Responsive */
        @media (max-width: 1024px) {
          .dashboard-grid {
            grid-template-columns: 1fr;
            grid-template-rows: auto;
          }

          .communication-section,
          .marketplace-section {
            grid-column: 1;
            grid-row: auto;
          }

          .spawn-form {
            max-width: 500px;
            margin: 0 auto;
          }
        }

        @media (max-width: 768px) {
          .homepage-main {
            padding: 1rem;
          }

          .dashboard-title {
            font-size: 1.5rem;
          }

          .agent-grid {
            grid-template-columns: 1fr;
          }
        }
      </style>
    `
  })
}
