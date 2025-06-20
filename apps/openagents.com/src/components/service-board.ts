/**
 * Service Request Board Component - NIP-90 AI job requests and responses
 */

import { html } from "@openagentsinc/psionic"

export interface ServiceBoardProps {
  agentId?: string
  activeJobs?: Array<{
    id: string
    type: string
    status: "pending" | "processing" | "completed" | "failed"
    requester: string
    provider: string
    amount: number
    description: string
    timestamp: number
  }>
  availableServices?: Array<{
    id: string
    name: string
    provider: string
    description: string
    basePrice: number
    capabilities: Array<string>
  }>
}

export function serviceBoard({
  activeJobs = [],
  agentId: _agentId,
  availableServices = []
}: ServiceBoardProps = {}) {
  return html`
    <div class="service-board" box-="square">
      <div class="board-header">
        <h3>AI Service Marketplace</h3>
        <div class="board-actions">
          <button is-="button" size-="small" variant-="foreground1" onclick="requestService()">
            Request Service
          </button>
          <button is-="button" size-="small" variant-="background1" onclick="refreshServices()">
            Refresh
          </button>
        </div>
      </div>

      <!-- Service Tabs -->
      <div class="service-tabs">
        <button class="tab-button active" onclick="switchTab('jobs')" id="jobs-tab">
          Active Jobs
          <span is-="badge" variant-="foreground0" size-="small" id="jobs-count">
            ${activeJobs.length}
          </span>
        </button>
        <button class="tab-button" onclick="switchTab('marketplace')" id="marketplace-tab">
          Marketplace
          <span is-="badge" variant-="background2" size-="small" id="services-count">
            ${availableServices.length}
          </span>
        </button>
      </div>

      <!-- Active Jobs Tab -->
      <div class="tab-content active" id="jobs-content">
        <div class="jobs-list">
          ${
    activeJobs.length === 0 ?
      html`<div class="empty-state">
              <p>No active jobs. Request an AI service to get started.</p>
            </div>` :
      html`<div class="jobs-container" id="jobs-container">
              ${
        activeJobs.map((job) =>
          html`
                <div class="job-card" box-="square">
                  <div class="job-header">
                    <div class="job-info">
                      <span class="job-type">${job.type}</span>
                      <span is-="badge" variant-="${getStatusColor(job.status)}" size-="small">
                        ${job.status}
                      </span>
                    </div>
                    <div class="job-amount">
                      <span class="amount">${job.amount} sats</span>
                    </div>
                  </div>
                  
                  <div class="job-details">
                    <p class="job-description">${job.description}</p>
                    <div class="job-participants">
                      <span class="participant">From: ${job.requester}</span>
                      <span class="participant">To: ${job.provider}</span>
                    </div>
                    <div class="job-timestamp">
                      ${new Date(job.timestamp).toLocaleString()}
                    </div>
                  </div>
                  
                  <div class="job-actions">
                    <button is-="button" size-="small" variant-="background1" 
                            onclick="viewJobDetails('${job.id}')">
                      Details
                    </button>
                    ${
            job.status === "pending" ?
              html`
                      <button is-="button" size-="small" variant-="danger" 
                              onclick="cancelJob('${job.id}')">
                        Cancel
                      </button>
                    ` :
              ""
          }
                  </div>
                </div>
              `
        ).join("")
      }
            </div>`
  }
        </div>
      </div>

      <!-- Marketplace Tab -->
      <div class="tab-content" id="marketplace-content">
        <div class="marketplace-list">
          ${
    availableServices.length === 0 ?
      html`<div class="empty-state">
              <p>No services available. Agents will appear here when they offer AI services.</p>
            </div>` :
      html`<div class="services-container" id="services-container">
              ${
        availableServices.map((service) =>
          html`
                <div class="service-card" box-="square">
                  <div class="service-header">
                    <h4 class="service-name">${service.name}</h4>
                    <div class="service-price">
                      <span class="price">${service.basePrice} sats</span>
                    </div>
                  </div>
                  
                  <div class="service-details">
                    <p class="service-description">${service.description}</p>
                    <div class="service-provider">
                      <span class="provider">Provider: ${service.provider}</span>
                    </div>
                    <div class="service-capabilities">
                      ${
            service.capabilities.map((cap) =>
              html`
                        <span is-="badge" variant-="background2" size-="small">${cap}</span>
                      `
            ).join("")
          }
                    </div>
                  </div>
                  
                  <div class="service-actions">
                    <button is-="button" size-="small" variant-="foreground1" 
                            onclick="requestSpecificService('${service.id}')">
                      Request Service
                    </button>
                    <button is-="button" size-="small" variant-="background1" 
                            onclick="viewServiceDetails('${service.id}')">
                      Details
                    </button>
                  </div>
                </div>
              `
        ).join("")
      }
            </div>`
  }
        </div>
      </div>
    </div>

    <script>
      // Real data from API - No more mocks!
      let currentJobs = [];
      let currentServices = [];

      // Load real services from API
      async function loadServices() {
        try {
          const response = await fetch('/api/services');
          const data = await response.json();
          
          if (data.services) {
            currentServices = data.services;
            updateServicesList();
            updateServicesCount();
            console.log('Loaded real services from API:', currentServices);
          }
        } catch (error) {
          console.error('Failed to load services:', error);
          // Fallback to prevent UI breaking
          currentServices = [
            {
              id: 'service-fallback',
              name: 'Service Loading Failed',
              provider: 'System',
              description: 'Please check API connectivity',
              basePrice: 0,
              capabilities: ['Error']
            }
          ];
        }
      }

      // Load real jobs from API
      async function loadJobs() {
        try {
          const response = await fetch('/api/jobs');
          const data = await response.json();
          
          if (data.jobs) {
            currentJobs = data.jobs;
            updateJobsList();
            updateJobsCount();
            console.log('Loaded real jobs from API:', currentJobs);
          }
        } catch (error) {
          console.error('Failed to load jobs:', error);
          // Fallback to prevent UI breaking
          currentJobs = [];
        }
      }

      // Save jobs via API (not needed for now - jobs are managed by NIP-90 events)
      function saveJobs() {
        // Jobs are now managed through NIP-90 events and stored in the relay
        // This function is kept for compatibility but does nothing
        console.log('Jobs are managed through NIP-90 events - no local storage needed');
      }

      let activeTab = 'jobs';

      // Tab management
      window.switchTab = function(tabName) {
        activeTab = tabName;
        
        // Update tab buttons
        document.querySelectorAll('.tab-button').forEach(btn => btn.classList.remove('active'));
        document.getElementById(tabName + '-tab').classList.add('active');
        
        // Update tab content
        document.querySelectorAll('.tab-content').forEach(content => content.classList.remove('active'));
        document.getElementById(tabName + '-content').classList.add('active');
      };

      // Service management functions
      window.requestService = async function() {
        const serviceType = prompt('What type of AI service do you need?\\n\\nOptions:\\n- Code Review\\n- Text Generation\\n- Code Generation\\n- Data Analysis');
        const description = prompt('Describe what you need:');
        const budget = prompt('Your budget (in sats):');
        
        if (serviceType && description && budget) {
          try {
            const response = await fetch('/api/services', {
              method: 'POST',
              headers: { 'Content-Type': 'application/json' },
              body: JSON.stringify({
                serviceType,
                description,
                budget: parseInt(budget)
              })
            });
            
            const result = await response.json();
            
            if (result.success && result.job) {
              currentJobs.unshift(result.job);
              updateJobsList();
              updateJobsCount();
              
              console.log('Real service requested via NIP-90:', result.job);
              alert('Service request created via NIP-90! Looking for available providers...');
            } else {
              throw new Error(result.error || 'Failed to create service request');
            }
          } catch (error) {
            console.error('Service request failed:', error);
            alert('Failed to create service request. Please try again.');
          }
        }
      };

      window.refreshServices = async function() {
        console.log('Refreshing services from Nostr relays...');
        try {
          if (activeTab === 'jobs') {
            await loadJobs();
          } else {
            await loadServices();
          }
        } catch (error) {
          console.error('Failed to refresh data:', error);
          alert('Failed to refresh data. Please check your connection.');
        }
      };

      window.viewJobDetails = function(jobId) {
        const job = currentJobs.find(j => j.id === jobId);
        if (job) {
          alert(\`Job Details:\\n\\nType: \${job.type}\\nStatus: \${job.status}\\nRequester: \${job.requester}\\nProvider: \${job.provider}\\nAmount: \${job.amount} sats\\nDescription: \${job.description}\\nCreated: \${new Date(job.timestamp).toLocaleString()}\`);
        }
      };

      window.cancelJob = function(jobId) {
        if (confirm('Are you sure you want to cancel this job?')) {
          const jobIndex = currentJobs.findIndex(j => j.id === jobId);
          if (jobIndex !== -1) {
            currentJobs.splice(jobIndex, 1);
            updateJobsList();
            updateJobsCount();
            console.log('Job cancelled:', jobId);
          }
        }
      };

      window.requestSpecificService = async function(serviceId) {
        const service = currentServices.find(s => s.id === serviceId);
        if (service) {
          const description = prompt(\`Request \${service.name} from \${service.provider}\\n\\nBase price: \${service.basePrice} sats\\n\\nDescribe your specific requirements:\`);
          if (description) {
            try {
              const response = await fetch('/api/services', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({
                  serviceType: service.name,
                  description,
                  budget: service.basePrice,
                  targetAgentPubkey: service.agent_pubkey
                })
              });
              
              const result = await response.json();
              
              if (result.success && result.job) {
                currentJobs.unshift(result.job);
                updateJobsList();
                updateJobsCount();
                
                console.log('Real targeted service requested via NIP-90:', result.job);
                alert(\`Service requested from \${service.provider} via NIP-90! They will be notified.\`);
              } else {
                throw new Error(result.error || 'Failed to create service request');
              }
            } catch (error) {
              console.error('Targeted service request failed:', error);
              alert('Failed to request service. Please try again.');
            }
          }
        }
      };

      window.viewServiceDetails = function(serviceId) {
        const service = currentServices.find(s => s.id === serviceId);
        if (service) {
          alert(\`Service Details:\\n\\nName: \${service.name}\\nProvider: \${service.provider}\\nBase Price: \${service.basePrice} sats\\nDescription: \${service.description}\\nCapabilities: \${service.capabilities.join(', ')}\`);
        }
      };

      function getStatusColor(status) {
        const colors = {
          'pending': 'background2',
          'processing': 'foreground0', 
          'completed': 'foreground0',
          'failed': 'danger'
        };
        return colors[status] || 'background2';
      }

      function updateJobsList() {
        const container = document.getElementById('jobs-container');
        if (container && currentJobs.length > 0) {
          container.innerHTML = currentJobs.map(job => \`
            <div class="job-card" box-="square">
              <div class="job-header">
                <div class="job-info">
                  <span class="job-type">\${job.type}</span>
                  <span is-="badge" variant-="\${getStatusColor(job.status)}" size-="small">
                    \${job.status}
                  </span>
                </div>
                <div class="job-amount">
                  <span class="amount">\${job.amount} sats</span>
                </div>
              </div>
              
              <div class="job-details">
                <p class="job-description">\${job.description}</p>
                <div class="job-participants">
                  <span class="participant">From: \${job.requester}</span>
                  <span class="participant">To: \${job.provider}</span>
                </div>
                <div class="job-timestamp">
                  \${new Date(job.timestamp).toLocaleString()}
                </div>
              </div>
              
              <div class="job-actions">
                <button is-="button" size-="small" variant-="background1" 
                        onclick="viewJobDetails('\${job.id}')">
                  Details
                </button>
                \${job.status === 'pending' ? \`
                  <button is-="button" size-="small" variant-="danger" 
                          onclick="cancelJob('\${job.id}')">
                    Cancel
                  </button>
                \` : ''}
              </div>
            </div>
          \`).join('');
        }
      }

      function updateServicesList() {
        const container = document.getElementById('services-container');
        if (container && currentServices.length > 0) {
          container.innerHTML = currentServices.map(service => \`
            <div class="service-card" box-="square">
              <div class="service-header">
                <h4 class="service-name">\${service.name}</h4>
                <div class="service-price">
                  <span class="price">\${service.basePrice} sats</span>
                </div>
              </div>
              
              <div class="service-details">
                <p class="service-description">\${service.description}</p>
                <div class="service-provider">
                  <span class="provider">Provider: \${service.provider}</span>
                </div>
                <div class="service-capabilities">
                  \${service.capabilities.map(cap => \`
                    <span is-="badge" variant-="background2" size-="small">\${cap}</span>
                  \`).join('')}
                </div>
              </div>
              
              <div class="service-actions">
                <button is-="button" size-="small" variant-="foreground1" 
                        onclick="requestSpecificService('\${service.id}')">
                  Request Service
                </button>
                <button is-="button" size-="small" variant-="background1" 
                        onclick="viewServiceDetails('\${service.id}')">
                  Details
                </button>
              </div>
            </div>
          \`).join('');
        }
      }

      function updateJobsCount() {
        const countBadge = document.getElementById('jobs-count');
        if (countBadge) {
          countBadge.textContent = currentJobs.length;
        }
      }

      function updateServicesCount() {
        const countBadge = document.getElementById('services-count');
        if (countBadge) {
          countBadge.textContent = currentServices.length;
        }
      }

      // Initialize with real data from APIs
      document.addEventListener('DOMContentLoaded', async function() {
        console.log('Initializing service board with real data...');
        try {
          // Load real services and jobs from APIs
          await Promise.all([
            loadServices(),
            loadJobs()
          ]);
          console.log('Service board initialized with real data');
        } catch (error) {
          console.error('Failed to initialize service board:', error);
          // Fallback to empty state
          updateJobsList();
          updateServicesList();
          updateJobsCount();
          updateServicesCount();
        }
      });
      
      // Also initialize immediately if DOM is already loaded
      if (document.readyState === 'loading') {
        // DOM is still loading, event listener will handle it
      } else {
        // DOM is already loaded, initialize now
        setTimeout(async () => {
          try {
            await Promise.all([
              loadServices(),
              loadJobs()
            ]);
            console.log('Service board initialized with real data (immediate)');
          } catch (error) {
            console.error('Failed to initialize service board (immediate):', error);
          }
        }, 0);
      }
    </script>

    <style>
      .service-board {
        background: var(--background1);
        padding: 1.5rem;
        height: 600px;
        display: flex;
        flex-direction: column;
      }

      .board-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        margin-bottom: 1rem;
        padding-bottom: 1rem;
        border-bottom: 1px solid var(--background3);
      }

      .board-header h3 {
        margin: 0;
        color: var(--foreground0);
      }

      .board-actions {
        display: flex;
        gap: 0.5rem;
      }

      /* Service Tabs */
      .service-tabs {
        display: flex;
        gap: 0.5rem;
        margin-bottom: 1rem;
      }

      .tab-button {
        padding: 0.5rem 1rem;
        background: var(--background2);
        border: 1px solid var(--background3);
        color: var(--foreground1);
        cursor: pointer;
        transition: all 0.2s ease;
        display: flex;
        align-items: center;
        gap: 0.5rem;
        font-family: inherit;
      }

      .tab-button:hover {
        background: var(--background3);
      }

      .tab-button.active {
        background: var(--foreground0);
        color: var(--background0);
      }

      /* Tab Content */
      .tab-content {
        flex: 1;
        display: none;
        overflow-y: auto;
      }

      .tab-content.active {
        display: block;
      }

      /* Jobs List */
      .jobs-container, .services-container {
        display: flex;
        flex-direction: column;
        gap: 1rem;
      }

      .job-card, .service-card {
        background: var(--background0);
        padding: 1rem;
        border-left: 3px solid var(--background3);
      }

      .job-header, .service-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        margin-bottom: 0.75rem;
      }

      .job-info {
        display: flex;
        align-items: center;
        gap: 0.75rem;
      }

      .job-type, .service-name {
        font-weight: 600;
        color: var(--foreground0);
      }

      .service-name {
        margin: 0;
        font-size: 1rem;
      }

      .job-amount, .service-price {
        display: flex;
        flex-direction: column;
        align-items: flex-end;
      }

      .amount, .price {
        font-weight: 600;
        color: var(--foreground0);
        font-size: 0.9rem;
      }

      .job-details, .service-details {
        margin-bottom: 1rem;
      }

      .job-description, .service-description {
        color: var(--foreground1);
        margin: 0 0 0.5rem 0;
        line-height: 1.4;
      }

      .job-participants, .service-provider {
        display: flex;
        gap: 1rem;
        margin-bottom: 0.5rem;
      }

      .participant, .provider {
        font-size: 0.85rem;
        color: var(--foreground2);
      }

      .job-timestamp {
        font-size: 0.8rem;
        color: var(--foreground2);
      }

      .service-capabilities {
        display: flex;
        flex-wrap: wrap;
        gap: 0.25rem;
        margin-top: 0.5rem;
      }

      .job-actions, .service-actions {
        display: flex;
        gap: 0.5rem;
        justify-content: flex-end;
      }

      /* Empty states */
      .empty-state {
        flex: 1;
        display: flex;
        align-items: center;
        justify-content: center;
        color: var(--foreground2);
        text-align: center;
        padding: 2rem;
      }

      /* Responsive */
      @media (max-width: 768px) {
        .service-board {
          height: 500px;
        }

        .service-tabs {
          flex-direction: column;
        }

        .job-header, .service-header {
          flex-direction: column;
          align-items: flex-start;
          gap: 0.5rem;
        }

        .job-participants {
          flex-direction: column;
          gap: 0.25rem;
        }

        .job-actions, .service-actions {
          justify-content: flex-start;
          flex-wrap: wrap;
        }
      }
    </style>
  `
}

function getStatusColor(status: string): string {
  const colors = {
    "pending": "background2",
    "processing": "foreground0",
    "completed": "foreground0",
    "failed": "danger"
  }
  return colors[status as keyof typeof colors] || "background2"
}
