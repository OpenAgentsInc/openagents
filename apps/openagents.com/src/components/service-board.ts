/**
 * Service Request Board Component - Real-time NIP-90 AI marketplace with WebSocket
 */

import { html } from "@openagentsinc/psionic"

export interface ServiceBoardProps {
  agentId?: string
}

export function serviceBoard({ agentId: _agentId }: ServiceBoardProps = {}) {
  return html`
    <div class="service-board" box-="square" id="service-board-container">
      <div class="board-loading">
        <div class="spinner"></div>
        Initializing WebSocket connection...
      </div>
    </div>
    
    <script type="module">
      // Initialize WebSocket connection when component mounts
      const container = document.getElementById('service-board-container')
      
      // State management
      const state = {
        services: new Map(),
        jobRequests: new Map(),
        jobResults: new Map(),
        activeTab: 'jobs',
        loading: true,
        error: null,
        ws: null
      }
      
      // UI update function
      function updateUI() {
        if (state.loading) {
          container.innerHTML = \`
            <div class="board-loading">
              <div class="spinner"></div>
              Connecting to marketplace...
            </div>
          \`
          return
        }
        
        if (state.error) {
          container.innerHTML = \`
            <div class="board-error">
              <p>Error: \${state.error}</p>
              <button onclick="location.reload()">Retry</button>
            </div>
          \`
          return
        }
        
        const servicesList = Array.from(state.services.values())
        const jobsList = Array.from(state.jobRequests.values())
        
        container.innerHTML = \`
          <div class="board-header">
            <h3>AI Service Marketplace</h3>
            <div class="board-actions">
              <button is-="button" size-="small" variant-="foreground1" onclick="window.requestService()">
                Request Service
              </button>
              <button is-="button" size-="small" variant-="background1" onclick="window.refreshData()">
                Refresh
              </button>
            </div>
          </div>

          <!-- Service Tabs -->
          <div class="service-tabs">
            <button class="tab-button \${state.activeTab === 'jobs' ? 'active' : ''}" 
                    onclick="window.switchTab('jobs')" id="jobs-tab">
              Active Jobs
              <span is-="badge" variant-="foreground0" size-="small">
                \${jobsList.length}
              </span>
            </button>
            <button class="tab-button \${state.activeTab === 'marketplace' ? 'active' : ''}" 
                    onclick="window.switchTab('marketplace')" id="marketplace-tab">
              Marketplace
              <span is-="badge" variant-="background2" size-="small">
                \${servicesList.length}
              </span>
            </button>
          </div>

          <!-- Active Jobs Tab -->
          <div class="tab-content \${state.activeTab === 'jobs' ? 'active' : ''}" id="jobs-content">
            <div class="jobs-list">
              \${jobsList.length === 0 ? 
                '<div class="empty-state"><p>No active jobs. Request an AI service to get started.</p></div>' :
                \`<div class="jobs-container">
                  \${jobsList.map(job => \`
                    <div class="job-card" box-="square">
                      <div class="job-header">
                        <div class="job-info">
                          <span class="job-type">\${getJobTypeName(job.requestKind)}</span>
                          <span is-="badge" variant-="\${getJobStatusColor(job)}" size-="small">
                            \${getJobStatus(job)}
                          </span>
                        </div>
                        <div class="job-amount">
                          <span class="amount">\${job.bidAmount} sats</span>
                        </div>
                      </div>
                      
                      <div class="job-details">
                        <p class="job-description">\${job.input}</p>
                        <div class="job-participants">
                          <span class="participant">From: \${job.requester.slice(0, 8)}...</span>
                          <span class="participant">To: \${job.provider.slice(0, 8)}...</span>
                        </div>
                        <div class="job-timestamp">
                          \${new Date(job.created_at * 1000).toLocaleString()}
                        </div>
                      </div>
                      
                      <div class="job-actions">
                        <button is-="button" size-="small" variant-="background1" 
                                onclick="window.viewJobDetails('\${job.jobId}')">
                          Details
                        </button>
                      </div>
                    </div>
                  \`).join('')}
                </div>\`
              }
            </div>
          </div>

          <!-- Marketplace Tab -->
          <div class="tab-content \${state.activeTab === 'marketplace' ? 'active' : ''}" id="marketplace-content">
            <div class="marketplace-list">
              \${servicesList.length === 0 ? 
                '<div class="empty-state"><p>No services available. Agents will appear here when they offer AI services.</p></div>' :
                \`<div class="services-container">
                  \${servicesList.map(service => \`
                    <div class="service-card" box-="square">
                      <div class="service-header">
                        <h4 class="service-name">\${service.name}</h4>
                        <div class="service-price">
                          <span class="price">From \${getBasePrice(service)} sats</span>
                        </div>
                      </div>
                      
                      <div class="service-details">
                        <p class="service-description">\${service.description}</p>
                        <div class="service-provider">
                          <span class="provider">Provider: \${service.provider.slice(0, 8)}...</span>
                        </div>
                        <div class="service-capabilities">
                          \${service.capabilities.map(cap => \`
                            <span is-="badge" variant-="background2" size-="small">\${cap.name}</span>
                          \`).join('')}
                        </div>
                      </div>
                      
                      <div class="service-actions">
                        <button is-="button" size-="small" variant-="foreground1" 
                                onclick="window.requestSpecificService('\${service.serviceId}')">
                          Request Service
                        </button>
                        <button is-="button" size-="small" variant-="background1" 
                                onclick="window.viewServiceDetails('\${service.serviceId}')">
                          Details
                        </button>
                      </div>
                    </div>
                  \`).join('')}
                </div>\`
              }
            </div>
          </div>
        \`
      }
      
      // Helper functions
      function getJobTypeName(kind) {
        const types = {
          5000: 'Text Generation',
          5001: 'Code Generation',
          5100: 'Image Generation',
          5200: 'Audio Generation',
          5300: 'Video Generation'
        }
        return types[kind] || 'Unknown Service'
      }
      
      function getJobStatus(job) {
        // Check if we have results
        const results = state.jobResults.get(job.jobId)
        if (results && results.length > 0) {
          const latestResult = results[results.length - 1]
          return latestResult.status
        }
        return 'pending'
      }
      
      function getJobStatusColor(job) {
        const status = getJobStatus(job)
        const colors = {
          'pending': 'background2',
          'processing': 'foreground0',
          'payment-required': 'danger',
          'success': 'foreground0',
          'error': 'danger',
          'partial': 'foreground1'
        }
        return colors[status] || 'background2'
      }
      
      function getBasePrice(service) {
        if (service.capabilities && service.capabilities.length > 0) {
          const prices = service.capabilities.map(c => c.pricing.basePrice)
          return Math.min(...prices)
        }
        return 0
      }
      
      // Window functions
      window.switchTab = (tabName) => {
        state.activeTab = tabName
        updateUI()
      }
      
      window.requestService = () => {
        // Note: Real implementation needs key management
        alert('Service requests require key management implementation')
      }
      
      window.refreshData = () => {
        console.log('Data refreshes automatically via WebSocket')
      }
      
      window.viewJobDetails = (jobId) => {
        const job = state.jobRequests.get(jobId)
        if (job) {
          const results = state.jobResults.get(jobId) || []
          const status = getJobStatus(job)
          
          let details = \`Job Details:\\n\\nID: \${job.jobId}\\nType: \${getJobTypeName(job.requestKind)}\\nStatus: \${status}\\nRequester: \${job.requester}\\nProvider: \${job.provider}\\nBid: \${job.bidAmount} sats\\nInput: \${job.input}\\nCreated: \${new Date(job.created_at * 1000).toLocaleString()}\`
          
          if (results.length > 0) {
            details += '\\n\\nResults:'
            results.forEach((result, i) => {
              details += \`\\n\\nResult #\${i + 1}:\\nStatus: \${result.status}\\nResult: \${result.result.slice(0, 100)}...\`
            })
          }
          
          alert(details)
        }
      }
      
      window.requestSpecificService = (serviceId) => {
        const service = state.services.get(serviceId)
        if (service) {
          // Note: Real implementation needs key management
          alert(\`Requesting service "\${service.name}" requires key management implementation\`)
        }
      }
      
      window.viewServiceDetails = (serviceId) => {
        const service = state.services.get(serviceId)
        if (service) {
          let details = \`Service Details:\\n\\nID: \${service.serviceId}\\nName: \${service.name}\\nProvider: \${service.provider}\\nDescription: \${service.description}\`
          
          if (service.lightningAddress) {
            details += \`\\nLightning Address: \${service.lightningAddress}\`
          }
          
          if (service.capabilities && service.capabilities.length > 0) {
            details += '\\n\\nCapabilities:'
            service.capabilities.forEach(cap => {
              details += \`\\n\\n- \${cap.name}\\n  Price: \${cap.pricing.basePrice} sats\\n  Description: \${cap.description}\`
            })
          }
          
          alert(details)
        }
      }
      
      // Initialize WebSocket connection
      async function initialize() {
        try {
          // Direct WebSocket connection
          const ws = new WebSocket('ws://localhost:3003/relay')
          state.ws = ws
          
          ws.onopen = () => {
            console.log('Service board WebSocket connected')
            state.loading = false
            
            // Subscribe to service offerings (NIP-90 kind 31990)
            const servicesSub = {
              id: 'services-' + Date.now(),
              filters: [{ kinds: [31990], limit: 100 }]
            }
            ws.send(JSON.stringify(['REQ', servicesSub.id, ...servicesSub.filters]))
            
            // Subscribe to job requests (NIP-90 kinds 5000-5300)
            const jobsSub = {
              id: 'jobs-' + Date.now(),
              filters: [{ kinds: [5000, 5001, 5100, 5200, 5300], limit: 50 }]
            }
            ws.send(JSON.stringify(['REQ', jobsSub.id, ...jobsSub.filters]))
            
            // Subscribe to job results (NIP-90 kinds 6000-6300)
            const resultsSub = {
              id: 'results-' + Date.now(),
              filters: [{ kinds: [6000, 6001, 6100, 6200, 6300], limit: 100 }]
            }
            ws.send(JSON.stringify(['REQ', resultsSub.id, ...resultsSub.filters]))
            
            updateUI()
          }
          
          ws.onmessage = (event) => {
            try {
              const msg = JSON.parse(event.data)
              
              if (msg[0] === 'EVENT') {
                const [, subId, nostrEvent] = msg
                
                // Handle service offerings
                if (nostrEvent.kind === 31990) {
                  try {
                    const content = JSON.parse(nostrEvent.content)
                    const service = {
                      serviceId: content.serviceId || nostrEvent.id,
                      name: content.name,
                      description: content.description,
                      capabilities: content.capabilities || [],
                      provider: nostrEvent.pubkey,
                      lightningAddress: content.lightningAddress,
                      relayHints: content.relayHints,
                      created_at: nostrEvent.created_at
                    }
                    state.services.set(service.serviceId, service)
                    updateUI()
                  } catch (e) {
                    console.error('Failed to parse service offering:', e)
                  }
                }
                
                // Handle job requests (kinds 5000-5300)
                if (nostrEvent.kind >= 5000 && nostrEvent.kind <= 5300) {
                  try {
                    const content = JSON.parse(nostrEvent.content)
                    const jobRequest = {
                      jobId: nostrEvent.id,
                      serviceId: content.serviceId,
                      requestKind: nostrEvent.kind,
                      input: content.input || nostrEvent.content,
                      inputType: content.inputType || 'text',
                      parameters: content.parameters,
                      bidAmount: content.bidAmount || 0,
                      requester: nostrEvent.pubkey,
                      provider: nostrEvent.tags.find(t => t[0] === 'p')?.[1] || '',
                      created_at: nostrEvent.created_at
                    }
                    state.jobRequests.set(jobRequest.jobId, jobRequest)
                    
                    // Subscribe to results for this job
                    const jobResultSub = {
                      id: 'job-result-' + jobRequest.jobId,
                      filters: [{ kinds: [6000, 6001, 6100, 6200, 6300], '#e': [jobRequest.jobId], limit: 10 }]
                    }
                    ws.send(JSON.stringify(['REQ', jobResultSub.id, ...jobResultSub.filters]))
                    
                    updateUI()
                  } catch (e) {
                    console.error('Failed to parse job request:', e)
                  }
                }
                
                // Handle job results (kinds 6000-6300)
                if (nostrEvent.kind >= 6000 && nostrEvent.kind <= 6300) {
                  const jobTag = nostrEvent.tags.find(t => t[0] === 'e')
                  if (jobTag) {
                    const jobId = jobTag[1]
                    try {
                      const content = JSON.parse(nostrEvent.content)
                      const jobResult = {
                        jobId: jobId,
                        requestEventId: jobId,
                        resultKind: nostrEvent.kind,
                        result: content.result || nostrEvent.content,
                        status: content.status || 'success',
                        provider: nostrEvent.pubkey,
                        computeTime: content.computeTime,
                        tokensUsed: content.tokensUsed,
                        confidence: content.confidence,
                        created_at: nostrEvent.created_at
                      }
                      
                      const results = state.jobResults.get(jobId) || []
                      results.push(jobResult)
                      state.jobResults.set(jobId, results)
                      updateUI()
                    } catch (e) {
                      console.error('Failed to parse job result:', e)
                    }
                  }
                }
              }
            } catch (e) {
              console.error('Failed to parse WebSocket message:', e)
            }
          }
          
          ws.onerror = (error) => {
            console.error('Service board WebSocket error:', error)
            state.error = 'Connection failed'
            state.loading = false
            updateUI()
          }
          
          ws.onclose = () => {
            console.log('Service board WebSocket closed')
            state.error = 'Connection lost'
            updateUI()
          }
          
        } catch (error) {
          console.error('Failed to initialize service board:', error)
          state.error = error.message
          state.loading = false
          updateUI()
        }
      }
      
      // Start initialization
      initialize()
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
      
      /* Loading and error states */
      .board-loading, .board-error {
        text-align: center;
        padding: 2rem;
      }
      
      .spinner {
        width: 2rem;
        height: 2rem;
        margin: 0 auto 1rem;
        border: 2px solid var(--border);
        border-top-color: var(--primary);
        border-radius: 50%;
        animation: spin 1s linear infinite;
      }
      
      @keyframes spin {
        to { transform: rotate(360deg); }
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
