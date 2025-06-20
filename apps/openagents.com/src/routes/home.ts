import { document, html } from "@openagentsinc/psionic"
import type { AgentIdentity } from "@openagentsinc/sdk"
import { agentList } from "../components/agent-list"
import { sharedHeader } from "../components/shared-header"
import { spawnAgentForm } from "../components/spawn-agent-form"
import { baseStyles } from "../styles"

export async function home() {
  // Generate some demo agents for initial display
  const demoAgents: Array<AgentIdentity> = []

  return document({
    title: "OpenAgents",
    styles: baseStyles,
    body: html`
      <!-- Fixed Layout Container -->
      <div class="fixed-layout">
        ${sharedHeader({ current: "home" })}

        <!-- Main Content -->
        <main class="homepage-main">
          <div class="dashboard-container">
            <h1 class="dashboard-title">Open Agents Dashboard</h1>
            
            <div class="dashboard-grid">
              <!-- Spawn Agent Form -->
              <div class="dashboard-section">
                ${spawnAgentForm()}
              </div>
              
              <!-- Agent List -->
              <div class="dashboard-section agents-section">
                <div id="agent-list-container">
                  ${agentList({ agents: demoAgents })}
                </div>
              </div>
            </div>
          </div>
        </main>
      </div>

      <script>
        // Agent management state
        let agents = [];
        
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
                <span is-="badge" variant-="\${stateColor}" cap-="round">
                  \${agent.lifecycleState || "bootstrapping"}
                </span>
              </div>
              
              <div class="agent-details">
                <div class="detail-row">
                  <span class="detail-label">ID:</span>
                  <span class="detail-value">\${agent.id}</span>
                </div>
                <div class="detail-row">
                  <span class="detail-label">Public Key:</span>
                  <span class="detail-value npub" title="\${agent.nostrKeys.public}">
                    \${agent.nostrKeys.public.slice(0, 16)}...
                  </span>
                </div>
                <div class="detail-row">
                  <span class="detail-label">Generation:</span>
                  <span class="detail-value">\${agent.generation}</span>
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
        
        // Handle spawn agent event
        window.addEventListener('spawn-agent', async (event) => {
          const { name, capital, metabolicRate } = event.detail;
          
          try {
            // Import SDK dynamically
            const { Agent } = await import('@openagentsinc/sdk');
            
            // Generate mnemonic and create agent
            const mnemonic = await Agent.generateMnemonic();
            const agent = await Agent.createFromMnemonic(mnemonic, {
              name: name,
              initial_capital: capital,
              stop_price: metabolicRate
            });
            
            // Store mnemonic separately (in real app, this should be encrypted)
            const agentData = {
              ...agent,
              mnemonic: mnemonic // Store for recovery
            };
            
            // Add to agents array
            agents.push(agentData);
            
            // Save and update display
            saveAgents();
            updateAgentList();
            
            console.log('Agent spawned:', agent);
            
            // Show success message
            alert(\`Agent "\${name}" spawned successfully!\\nPublic key: \${agent.nostrKeys.public}\`);
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
        
        // Load agents on page load
        loadAgents();
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
          gap: 2rem;
          align-items: start;
        }

        .dashboard-section {
          width: 100%;
        }

        .agents-section {
          max-height: calc(100vh - 200px);
          overflow-y: auto;
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
