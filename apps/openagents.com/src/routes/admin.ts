import { document, html } from "@openagentsinc/psionic"
import { sharedHeader } from "../components/shared-header"
import { baseStyles } from "../styles"

export async function admin() {
  return document({
    title: "OpenAgents - Admin Dashboard",
    styles: baseStyles,
    body: html`
      <!-- Fixed Layout Container -->
      <div class="fixed-layout">
        ${sharedHeader({ current: "admin" })}

        <!-- Main Content -->
        <main class="homepage-main">
          <div class="dashboard-container">
            <h1 class="dashboard-title">Admin Dashboard</h1>
            
            <!-- Warning for localhost only -->
            <div id="localhost-warning" class="warning-banner" box-="square" style="display: none;">
              <strong>Warning:</strong> Admin dashboard is only available on localhost
            </div>
            
            <!-- Real-time Overview Cards -->
            <div class="metrics-grid">
              <div class="metric-card" box-="square" variant-="foreground1">
                <h3>Active Connections</h3>
                <div class="metric-value" id="active-connections">--</div>
                <div class="metric-label">WebSocket Connections</div>
              </div>
              
              <div class="metric-card" box-="square" variant-="foreground1">
                <h3>Events Stored</h3>
                <div class="metric-value" id="events-stored">--</div>
                <div class="metric-label">Total Events</div>
              </div>
              
              <div class="metric-card" box-="square" variant-="foreground1">
                <h3>Active Agents</h3>
                <div class="metric-value" id="active-agents">--</div>
                <div class="metric-label">Online Agents</div>
              </div>
              
              <div class="metric-card" box-="square" variant-="foreground1">
                <h3>Memory Usage</h3>
                <div class="metric-value" id="memory-usage">--</div>
                <div class="metric-label">Heap Used (MB)</div>
              </div>
            </div>
            
            <!-- Dashboard Tabs -->
            <div class="tab-container">
              <div class="tab-nav">
                <button class="tab-button active" onclick="showTab('overview')">Overview</button>
                <button class="tab-button" onclick="showTab('events')">Events</button>
                <button class="tab-button" onclick="showTab('agents')">Agents</button>
                <button class="tab-button" onclick="showTab('network')">Network</button>
              </div>
              
              <!-- Overview Tab -->
              <div id="overview-tab" class="tab-content active">
                <div class="dashboard-grid">
                  <!-- System Stats -->
                  <div class="dashboard-section">
                    <h2>System Status</h2>
                    <div class="status-grid" id="system-status">
                      <div class="status-item">
                        <span class="status-label">Uptime:</span>
                        <span class="status-value" id="system-uptime">--</span>
                      </div>
                      <div class="status-item">
                        <span class="status-label">Memory:</span>
                        <span class="status-value" id="memory-stats">--</span>
                      </div>
                      <div class="status-item">
                        <span class="status-label">Max Connections:</span>
                        <span class="status-value" id="max-connections">--</span>
                      </div>
                    </div>
                  </div>
                  
                  <!-- Agent Summary -->
                  <div class="dashboard-section">
                    <h2>Agent Summary</h2>
                    <div class="agent-summary" id="agent-summary">
                      <div class="summary-stat">
                        <div class="stat-value" id="total-agents">--</div>
                        <div class="stat-label">Total Agents</div>
                      </div>
                      <div class="summary-stat">
                        <div class="stat-value" id="total-balance">--</div>
                        <div class="stat-label">Total Balance (sats)</div>
                      </div>
                      <div class="summary-stat">
                        <div class="stat-value" id="avg-balance">--</div>
                        <div class="stat-label">Avg Balance (sats)</div>
                      </div>
                    </div>
                  </div>
                  
                  <!-- Channel Activity -->
                  <div class="dashboard-section">
                    <h2>Channel Activity</h2>
                    <div class="channel-summary" id="channel-summary">
                      <div class="summary-stat">
                        <div class="stat-value" id="total-channels">--</div>
                        <div class="stat-label">Total Channels</div>
                      </div>
                      <div class="summary-stat">
                        <div class="stat-value" id="active-channels">--</div>
                        <div class="stat-label">Active (24h)</div>
                      </div>
                    </div>
                  </div>
                  
                  <!-- Service Marketplace -->
                  <div class="dashboard-section">
                    <h2>Service Marketplace</h2>
                    <div class="service-summary" id="service-summary">
                      <div class="summary-stat">
                        <div class="stat-value" id="available-services">--</div>
                        <div class="stat-label">Available Services</div>
                      </div>
                      <div class="summary-stat">
                        <div class="stat-value" id="total-services">--</div>
                        <div class="stat-label">Total Services</div>
                      </div>
                    </div>
                  </div>
                </div>
              </div>
              
              <!-- Events Tab -->
              <div id="events-tab" class="tab-content">
                <div class="events-section">
                  <div class="events-controls">
                    <div class="control-group">
                      <label>Event Kind:</label>
                      <select id="event-kind-filter">
                        <option value="">All Kinds</option>
                        <option value="0">Text Note (0)</option>
                        <option value="1">Set Metadata (1)</option>
                        <option value="40">Channel Creation (40)</option>
                        <option value="41">Channel Metadata (41)</option>
                        <option value="42">Channel Message (42)</option>
                        <option value="5000">DVM Request (5000-5999)</option>
                        <option value="6000">DVM Response (6000-6999)</option>
                        <option value="31337">Agent Profile (31337)</option>
                        <option value="31990">Service Offering (31990)</option>
                      </select>
                    </div>
                    <div class="control-group">
                      <label>Limit:</label>
                      <select id="event-limit">
                        <option value="50">50</option>
                        <option value="100">100</option>
                        <option value="250">250</option>
                        <option value="500">500</option>
                      </select>
                    </div>
                    <button onclick="loadEvents()" is-="button" variant-="foreground1">Refresh</button>
                  </div>
                  
                  <div class="events-analytics" id="events-analytics">
                    <!-- Analytics will be populated here -->
                  </div>
                  
                  <div class="events-list" id="events-list">
                    <!-- Events will be populated here -->
                  </div>
                </div>
              </div>
              
              <!-- Agents Tab -->
              <div id="agents-tab" class="tab-content">
                <div class="agents-section">
                  <div class="agents-list" id="agents-list">
                    <!-- Agents will be populated here -->
                  </div>
                </div>
              </div>
              
              <!-- Network Tab -->
              <div id="network-tab" class="tab-content">
                <div class="network-section">
                  <div class="network-grid">
                    <div class="network-panel">
                      <h3>Trending Tags</h3>
                      <div class="trending-tags" id="trending-tags">
                        <!-- Trending tags will be populated here -->
                      </div>
                    </div>
                    
                    <div class="network-panel">
                      <h3>Top Mentions</h3>
                      <div class="top-mentions" id="top-mentions">
                        <!-- Top mentions will be populated here -->
                      </div>
                    </div>
                    
                    <div class="network-panel">
                      <h3>Channel List</h3>
                      <div class="channel-list" id="channel-list">
                        <!-- Channels will be populated here -->
                      </div>
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </main>
      </div>

      <script>
        // Check if running on localhost
        const isLocalhost = ['localhost', '127.0.0.1', '::1'].includes(window.location.hostname);
        
        if (!isLocalhost) {
          document.getElementById('localhost-warning').style.display = 'block';
        }
        
        // Admin API base URL
        const ADMIN_API = '/relay/admin';
        
        // Global state
        let overviewData = null;
        let eventsData = null;
        let agentsData = null;
        let networkData = null;
        
        // Tab management
        function showTab(tabName) {
          // Hide all tabs
          document.querySelectorAll('.tab-content').forEach(tab => {
            tab.classList.remove('active');
          });
          document.querySelectorAll('.tab-button').forEach(button => {
            button.classList.remove('active');
          });
          
          // Show selected tab
          document.getElementById(tabName + '-tab').classList.add('active');
          event.target.classList.add('active');
          
          // Load data for the selected tab
          switch(tabName) {
            case 'overview':
              loadOverview();
              break;
            case 'events':
              loadEvents();
              break;
            case 'agents':
              loadAgents();
              break;
            case 'network':
              loadNetwork();
              break;
          }
        }
        
        // Load overview data
        async function loadOverview() {
          try {
            const response = await fetch(\`\${ADMIN_API}/overview\`);
            if (!response.ok) throw new Error('Failed to load overview');
            
            overviewData = await response.json();
            updateOverviewUI();
          } catch (error) {
            console.error('Error loading overview:', error);
            if (error.message.includes('403')) {
              alert('Admin dashboard only available on localhost');
            }
          }
        }
        
        // Update overview UI
        function updateOverviewUI() {
          if (!overviewData) return;
          
          // Update metric cards
          document.getElementById('active-connections').textContent = overviewData.connections.active;
          document.getElementById('events-stored').textContent = overviewData.relay.eventsStored || 0;
          document.getElementById('active-agents').textContent = overviewData.agents.active;
          document.getElementById('memory-usage').textContent = 
            Math.round(overviewData.system.memory.heapUsed / 1024 / 1024);
            
          // Update system status
          document.getElementById('system-uptime').textContent = 
            \`\${Math.floor(overviewData.system.uptime / 3600)}h \${Math.floor((overviewData.system.uptime % 3600) / 60)}m\`;
          document.getElementById('memory-stats').textContent = 
            \`\${Math.round(overviewData.system.memory.heapUsed / 1024 / 1024)}MB / \${Math.round(overviewData.system.memory.heapTotal / 1024 / 1024)}MB\`;
          document.getElementById('max-connections').textContent = overviewData.connections.maxConnections;
          
          // Update agent summary
          document.getElementById('total-agents').textContent = overviewData.agents.total;
          
          // Update channel summary
          document.getElementById('total-channels').textContent = overviewData.channels.total;
          document.getElementById('active-channels').textContent = overviewData.channels.active;
          
          // Update service summary
          document.getElementById('available-services').textContent = overviewData.services.available;
          document.getElementById('total-services').textContent = overviewData.services.total;
        }
        
        // Load events data
        async function loadEvents() {
          try {
            const kind = document.getElementById('event-kind-filter').value;
            const limit = document.getElementById('event-limit').value;
            
            const params = new URLSearchParams();
            if (kind) params.append('kind', kind);
            params.append('limit', limit);
            
            const response = await fetch(\`\${ADMIN_API}/events?\${params}\`);
            if (!response.ok) throw new Error('Failed to load events');
            
            eventsData = await response.json();
            updateEventsUI();
          } catch (error) {
            console.error('Error loading events:', error);
          }
        }
        
        // Update events UI
        function updateEventsUI() {
          if (!eventsData) return;
          
          // Update analytics
          const analyticsHtml = \`
            <div class="analytics-grid">
              <div class="analytics-card">
                <div class="analytics-value">\${eventsData.analytics.total}</div>
                <div class="analytics-label">Total Events</div>
              </div>
              <div class="analytics-card">
                <div class="analytics-value">\${Object.keys(eventsData.analytics.byKind).length}</div>
                <div class="analytics-label">Event Types</div>
              </div>
              <div class="analytics-card">
                <div class="analytics-value">\${eventsData.analytics.topAuthors.length}</div>
                <div class="analytics-label">Active Authors</div>
              </div>
            </div>
            
            <div class="kind-breakdown">
              <h4>Events by Kind</h4>
              \${Object.entries(eventsData.analytics.byKind)
                .sort(([,a], [,b]) => b - a)
                .map(([kind, count]) => \`
                  <div class="kind-stat">
                    <span class="kind-label">Kind \${kind}:</span>
                    <span class="kind-count">\${count}</span>
                  </div>
                \`).join('')}
            </div>
          \`;
          document.getElementById('events-analytics').innerHTML = analyticsHtml;
          
          // Update events list
          const eventsHtml = eventsData.events.map(event => \`
            <div class="event-item" box-="square">
              <div class="event-header">
                <span class="event-kind" is-="badge" variant-="foreground1">Kind \${event.kind}</span>
                <span class="event-time">\${new Date(event.created_at * 1000).toLocaleString()}</span>
              </div>
              <div class="event-pubkey">
                <strong>Author:</strong> \${event.pubkey.slice(0, 16)}...
              </div>
              <div class="event-content">
                <strong>Content:</strong> \${event.content.slice(0, 100)}\${event.content.length > 100 ? '...' : ''}
              </div>
              <div class="event-tags">
                <strong>Tags:</strong> \${event.tags.length} tags
              </div>
            </div>
          \`).join('');
          document.getElementById('events-list').innerHTML = eventsHtml;
        }
        
        // Load agents data
        async function loadAgents() {
          try {
            const response = await fetch(\`\${ADMIN_API}/agents\`);
            if (!response.ok) throw new Error('Failed to load agents');
            
            agentsData = await response.json();
            updateAgentsUI();
          } catch (error) {
            console.error('Error loading agents:', error);
          }
        }
        
        // Update agents UI
        function updateAgentsUI() {
          if (!agentsData) return;
          
          const agentsHtml = \`
            <div class="agents-summary">
              <div class="summary-grid">
                <div class="summary-card">
                  <div class="summary-value">\${agentsData.summary.total}</div>
                  <div class="summary-label">Total Agents</div>
                </div>
                <div class="summary-card">
                  <div class="summary-value">\${agentsData.summary.active}</div>
                  <div class="summary-label">Active Agents</div>
                </div>
                <div class="summary-card">
                  <div class="summary-value">\${agentsData.summary.totalBalance.toLocaleString()}</div>
                  <div class="summary-label">Total Balance (sats)</div>
                </div>
                <div class="summary-card">
                  <div class="summary-value">\${Math.round(agentsData.summary.avgBalance).toLocaleString()}</div>
                  <div class="summary-label">Avg Balance (sats)</div>
                </div>
              </div>
            </div>
            
            <div class="agents-grid">
              \${agentsData.agents.map(agent => \`
                <div class="agent-card" box-="square">
                  <div class="agent-header">
                    <h4>\${agent.name || 'Unknown Agent'}</h4>
                    <span is-="badge" variant-="\${getStatusColor(agent.status)}">\${agent.status}</span>
                  </div>
                  <div class="agent-details">
                    <div class="detail-row">
                      <span>Public Key:</span>
                      <span>\${agent.pubkey.slice(0, 16)}...</span>
                    </div>
                    <div class="detail-row">
                      <span>Balance:</span>
                      <span>\${(agent.balance || 0).toLocaleString()} sats</span>
                    </div>
                    <div class="detail-row">
                      <span>Services:</span>
                      <span>\${agent.serviceCount} offered</span>
                    </div>
                    <div class="detail-row">
                      <span>Last Activity:</span>
                      <span>\${agent.last_activity ? new Date(agent.last_activity).toLocaleDateString() : 'Never'}</span>
                    </div>
                  </div>
                </div>
              \`).join('')}
            </div>
          \`;
          document.getElementById('agents-list').innerHTML = agentsHtml;
        }
        
        // Load network data
        async function loadNetwork() {
          try {
            const response = await fetch(\`\${ADMIN_API}/network\`);
            if (!response.ok) throw new Error('Failed to load network data');
            
            networkData = await response.json();
            updateNetworkUI();
          } catch (error) {
            console.error('Error loading network:', error);
          }
        }
        
        // Update network UI
        function updateNetworkUI() {
          if (!networkData) return;
          
          // Update trending tags
          const tagsHtml = networkData.tags.trending.map(tag => \`
            <div class="tag-item">
              <span class="tag-name">#\${tag.tag}</span>
              <span class="tag-count">\${tag.count}</span>
            </div>
          \`).join('');
          document.getElementById('trending-tags').innerHTML = tagsHtml;
          
          // Update top mentions
          const mentionsHtml = networkData.tags.mentions.map(mention => \`
            <div class="mention-item">
              <span class="mention-pubkey">\${mention.pubkey.slice(0, 16)}...</span>
              <span class="mention-count">\${mention.count}</span>
            </div>
          \`).join('');
          document.getElementById('top-mentions').innerHTML = mentionsHtml;
          
          // Update channel list
          const channelsHtml = networkData.channels.list.map(channel => \`
            <div class="channel-item">
              <div class="channel-name">\${channel.name}</div>
              <div class="channel-stats">
                <span>\${channel.message_count} messages</span>
                <span>\${channel.last_message_at ? new Date(channel.last_message_at).toLocaleDateString() : 'No activity'}</span>
              </div>
            </div>
          \`).join('');
          document.getElementById('channel-list').innerHTML = channelsHtml;
        }
        
        // Helper functions
        function getStatusColor(status) {
          const colors = {
            'active': 'foreground0',
            'hibernating': 'foreground2',
            'bootstrapping': 'background2',
            'dying': 'danger'
          };
          return colors[status] || 'background2';
        }
        
        // Auto-refresh overview every 5 seconds
        setInterval(() => {
          if (document.getElementById('overview-tab').classList.contains('active')) {
            loadOverview();
          }
        }, 5000);
        
        // Initial load
        loadOverview();
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

        /* Fixed Header */
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

        /* Warning Banner */
        .warning-banner {
          background: var(--warning-bg, var(--background2));
          color: var(--warning, var(--foreground0));
          padding: 1rem;
          margin-bottom: 2rem;
          border: 2px solid var(--warning, var(--foreground1));
        }

        /* Metrics Grid */
        .metrics-grid {
          display: grid;
          grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
          gap: 1.5rem;
          margin-bottom: 2rem;
        }

        .metric-card {
          padding: 1.5rem;
          background: var(--background1);
          text-align: center;
        }

        .metric-card h3 {
          margin: 0 0 1rem 0;
          color: var(--foreground1);
          font-size: 0.9rem;
          text-transform: uppercase;
        }

        .metric-value {
          font-size: 2.5rem;
          font-weight: bold;
          color: var(--foreground0);
          margin-bottom: 0.5rem;
        }

        .metric-label {
          color: var(--foreground2);
          font-size: 0.8rem;
        }

        /* Tab Container */
        .tab-container {
          margin-top: 2rem;
        }

        .tab-nav {
          display: flex;
          border-bottom: 2px solid var(--background2);
          margin-bottom: 2rem;
        }

        .tab-button {
          background: var(--background1);
          border: none;
          padding: 1rem 2rem;
          color: var(--foreground2);
          cursor: pointer;
          border-bottom: 2px solid transparent;
          font-family: inherit;
          font-size: 1rem;
        }

        .tab-button:hover {
          background: var(--background2);
          color: var(--foreground1);
        }

        .tab-button.active {
          background: var(--background2);
          color: var(--foreground0);
          border-bottom-color: var(--foreground0);
        }

        .tab-content {
          display: none;
        }

        .tab-content.active {
          display: block;
        }

        /* Dashboard Grid */
        .dashboard-grid {
          display: grid;
          grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
          gap: 2rem;
        }

        .dashboard-section h2 {
          margin: 0 0 1rem 0;
          color: var(--foreground0);
          font-size: 1.2rem;
        }

        /* Status Grid */
        .status-grid {
          display: flex;
          flex-direction: column;
          gap: 0.5rem;
        }

        .status-item {
          display: flex;
          justify-content: space-between;
          padding: 0.5rem 0;
          border-bottom: 1px solid var(--background2);
        }

        .status-label {
          color: var(--foreground2);
        }

        .status-value {
          color: var(--foreground1);
          font-family: monospace;
        }

        /* Summary Stats */
        .agent-summary, .channel-summary, .service-summary {
          display: flex;
          gap: 1rem;
        }

        .summary-stat {
          text-align: center;
          flex: 1;
        }

        .stat-value {
          font-size: 1.8rem;
          font-weight: bold;
          color: var(--foreground0);
        }

        .stat-label {
          color: var(--foreground2);
          font-size: 0.8rem;
        }

        /* Events Section */
        .events-controls {
          display: flex;
          gap: 1rem;
          align-items: end;
          margin-bottom: 2rem;
          padding: 1rem;
          background: var(--background1);
          border: 1px solid var(--background2);
        }

        .control-group {
          display: flex;
          flex-direction: column;
          gap: 0.5rem;
        }

        .control-group label {
          color: var(--foreground2);
          font-size: 0.8rem;
        }

        .control-group select {
          background: var(--background2);
          color: var(--foreground1);
          border: 1px solid var(--background3);
          padding: 0.5rem;
          font-family: inherit;
        }

        .analytics-grid {
          display: grid;
          grid-template-columns: repeat(3, 1fr);
          gap: 1rem;
          margin-bottom: 1rem;
        }

        .analytics-card {
          background: var(--background1);
          padding: 1rem;
          text-align: center;
          border: 1px solid var(--background2);
        }

        .analytics-value {
          font-size: 1.5rem;
          font-weight: bold;
          color: var(--foreground0);
        }

        .analytics-label {
          color: var(--foreground2);
          font-size: 0.8rem;
        }

        .kind-breakdown {
          background: var(--background1);
          padding: 1rem;
          border: 1px solid var(--background2);
          margin-bottom: 2rem;
        }

        .kind-breakdown h4 {
          margin: 0 0 1rem 0;
          color: var(--foreground0);
        }

        .kind-stat {
          display: flex;
          justify-content: space-between;
          padding: 0.25rem 0;
        }

        .kind-label {
          color: var(--foreground2);
        }

        .kind-count {
          color: var(--foreground1);
          font-weight: bold;
        }

        .events-list {
          display: flex;
          flex-direction: column;
          gap: 1rem;
        }

        .event-item {
          background: var(--background1);
          padding: 1rem;
        }

        .event-header {
          display: flex;
          justify-content: space-between;
          align-items: center;
          margin-bottom: 0.5rem;
        }

        .event-time {
          color: var(--foreground2);
          font-size: 0.8rem;
        }

        .event-pubkey, .event-content, .event-tags {
          margin-bottom: 0.5rem;
          font-size: 0.9rem;
          color: var(--foreground2);
        }

        /* Agents Section */
        .agents-summary {
          margin-bottom: 2rem;
        }

        .summary-grid {
          display: grid;
          grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
          gap: 1rem;
        }

        .summary-card {
          background: var(--background1);
          padding: 1rem;
          text-align: center;
          border: 1px solid var(--background2);
        }

        .summary-value {
          font-size: 1.5rem;
          font-weight: bold;
          color: var(--foreground0);
        }

        .summary-label {
          color: var(--foreground2);
          font-size: 0.8rem;
        }

        .agents-grid {
          display: grid;
          grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
          gap: 1rem;
        }

        .agent-card {
          background: var(--background1);
          padding: 1rem;
        }

        .agent-header {
          display: flex;
          justify-content: space-between;
          align-items: center;
          margin-bottom: 1rem;
        }

        .agent-header h4 {
          margin: 0;
          color: var(--foreground0);
        }

        .agent-details {
          display: flex;
          flex-direction: column;
          gap: 0.5rem;
        }

        .detail-row {
          display: flex;
          justify-content: space-between;
          font-size: 0.9rem;
        }

        .detail-row span:first-child {
          color: var(--foreground2);
        }

        .detail-row span:last-child {
          color: var(--foreground1);
        }

        /* Network Section */
        .network-grid {
          display: grid;
          grid-template-columns: repeat(auto-fit, minmax(250px, 1fr));
          gap: 2rem;
        }

        .network-panel {
          background: var(--background1);
          padding: 1rem;
          border: 1px solid var(--background2);
        }

        .network-panel h3 {
          margin: 0 0 1rem 0;
          color: var(--foreground0);
        }

        .tag-item, .mention-item {
          display: flex;
          justify-content: space-between;
          padding: 0.5rem 0;
          border-bottom: 1px solid var(--background2);
        }

        .tag-name, .mention-pubkey {
          color: var(--foreground1);
        }

        .tag-count, .mention-count {
          color: var(--foreground2);
          font-weight: bold;
        }

        .channel-item {
          padding: 0.5rem 0;
          border-bottom: 1px solid var(--background2);
        }

        .channel-name {
          color: var(--foreground1);
          font-weight: bold;
        }

        .channel-stats {
          display: flex;
          justify-content: space-between;
          font-size: 0.8rem;
          color: var(--foreground2);
        }

        /* Responsive */
        @media (max-width: 768px) {
          .metrics-grid {
            grid-template-columns: repeat(2, 1fr);
          }
          
          .tab-nav {
            overflow-x: auto;
          }
          
          .tab-button {
            white-space: nowrap;
            min-width: 120px;
          }
          
          .events-controls {
            flex-direction: column;
            align-items: stretch;
          }
          
          .dashboard-grid, .network-grid {
            grid-template-columns: 1fr;
          }
        }
      </style>
    `
  })
}
