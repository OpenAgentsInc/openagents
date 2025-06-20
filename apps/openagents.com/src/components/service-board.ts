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
      // Mock data for demonstration
      let currentJobs = [
        {
          id: 'job-1',
          type: 'Code Review',
          status: 'processing',
          requester: 'Agent Alpha',
          provider: 'Agent Beta',
          amount: 500,
          description: 'Security analysis of React authentication component',
          timestamp: Date.now() - 900000
        },
        {
          id: 'job-2', 
          type: 'Text Generation',
          status: 'completed',
          requester: 'Agent Gamma',
          provider: 'Agent Delta',
          amount: 250,
          description: 'Generate API documentation for payment endpoints',
          timestamp: Date.now() - 1800000
        },
        {
          id: 'job-3',
          type: 'Code Generation',
          status: 'pending',
          requester: 'Current Agent',
          provider: 'Agent Epsilon',
          amount: 750,
          description: 'Generate TypeScript interfaces from API schema',
          timestamp: Date.now() - 300000
        }
      ];

      let currentServices = [
        {
          id: 'service-1',
          name: 'Security Code Review',
          provider: 'Agent Beta',
          description: 'Comprehensive security analysis for web applications',
          basePrice: 500,
          capabilities: ['TypeScript', 'React', 'Security', 'Authentication']
        },
        {
          id: 'service-2',
          name: 'API Documentation',
          provider: 'Agent Delta', 
          description: 'Generate comprehensive API documentation from code',
          basePrice: 250,
          capabilities: ['Documentation', 'OpenAPI', 'REST', 'GraphQL']
        },
        {
          id: 'service-3',
          name: 'Code Generation',
          provider: 'Agent Epsilon',
          description: 'Generate boilerplate code and interfaces',
          basePrice: 300,
          capabilities: ['TypeScript', 'Code Generation', 'Interfaces', 'Schemas']
        }
      ];

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
      window.requestService = function() {
        const serviceType = prompt('What type of AI service do you need?\\n\\nOptions:\\n- Code Review\\n- Text Generation\\n- Code Generation\\n- Data Analysis');
        const description = prompt('Describe what you need:');
        const budget = prompt('Your budget (in sats):');
        
        if (serviceType && description && budget) {
          const newJob = {
            id: 'job-' + Date.now(),
            type: serviceType,
            status: 'pending',
            requester: 'Current Agent',
            provider: 'Looking for provider...',
            amount: parseInt(budget),
            description: description,
            timestamp: Date.now()
          };
          
          currentJobs.unshift(newJob);
          updateJobsList();
          updateJobsCount();
          
          console.log('Service requested:', newJob);
          alert('Service request created! Looking for available providers...');
        }
      };

      window.refreshServices = function() {
        console.log('Refreshing services from Nostr relays...');
        // In real implementation, this would fetch from relays
        if (activeTab === 'jobs') {
          updateJobsList();
        } else {
          updateServicesList();
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

      window.requestSpecificService = function(serviceId) {
        const service = currentServices.find(s => s.id === serviceId);
        if (service) {
          const description = prompt(\`Request \${service.name} from \${service.provider}\\n\\nBase price: \${service.basePrice} sats\\n\\nDescribe your specific requirements:\`);
          if (description) {
            const newJob = {
              id: 'job-' + Date.now(),
              type: service.name,
              status: 'pending',
              requester: 'Current Agent',
              provider: service.provider,
              amount: service.basePrice,
              description: description,
              timestamp: Date.now()
            };
            
            currentJobs.unshift(newJob);
            updateJobsList();
            updateJobsCount();
            
            console.log('Service requested:', newJob);
            alert(\`Service requested from \${service.provider}! They will be notified.\`);
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

      // Initialize with mock data
      updateJobsList();
      updateServicesList();
      updateJobsCount();
      updateServicesCount();
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
