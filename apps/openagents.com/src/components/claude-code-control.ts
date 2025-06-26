/**
 * Claude Code Control UI Component
 * Provides interface for controlling remote Claude Code instances
 */
import { html } from "@openagentsinc/psionic"

// Types are defined inline in the JavaScript code

export function claudeCodeControl() {
  return html`
    <div class="claude-code-control">
      <h2>Claude Code Remote Control</h2>
      
      <div class="machines-section">
        <h3>Available Machines</h3>
        <div id="machines-list" class="machines-list">
          <div class="loading">Connecting to Claude Code server...</div>
        </div>
      </div>
      
      <div class="sessions-section">
        <h3>Active Sessions</h3>
        <div id="sessions-list" class="sessions-list">
          <div class="empty">No active sessions</div>
        </div>
      </div>
      
      <div class="control-section">
        <h3>Start New Session</h3>
        <form id="start-session-form">
          <div class="form-group">
            <label for="machine-select">Machine:</label>
            <select id="machine-select" name="machineId" required>
              <option value="">Select a machine...</option>
            </select>
          </div>
          
          <div class="form-group">
            <label for="project-path">Project Path:</label>
            <input 
              type="text" 
              id="project-path" 
              name="projectPath" 
              placeholder="/path/to/project"
              required
            />
          </div>
          
          <button type="submit">Start Session</button>
        </form>
      </div>
      
      <div class="prompt-section" style="display: none;">
        <h3>Send Prompt</h3>
        <form id="send-prompt-form">
          <div class="form-group">
            <label for="session-select">Session:</label>
            <select id="session-select" name="sessionId" required>
              <option value="">Select a session...</option>
            </select>
          </div>
          
          <div class="form-group">
            <label for="prompt-text">Prompt:</label>
            <textarea 
              id="prompt-text" 
              name="prompt" 
              rows="4"
              placeholder="Enter your prompt for Claude Code..."
              required
            ></textarea>
          </div>
          
          <button type="submit">Send Prompt</button>
        </form>
      </div>
      
      <div class="responses-section">
        <h3>Responses</h3>
        <div id="responses-list" class="responses-list">
          <div class="empty">No responses yet</div>
        </div>
      </div>
    </div>
    
    <style>
      .claude-code-control {
        max-width: 1200px;
        margin: 0 auto;
        padding: 2rem;
        font-family: 'Berkeley Mono', monospace;
      }
      
      .claude-code-control h2 {
        color: var(--text);
        margin-bottom: 2rem;
      }
      
      .claude-code-control h3 {
        color: var(--darkgray);
        margin-bottom: 1rem;
        font-size: 1.1rem;
      }
      
      .machines-section,
      .sessions-section,
      .control-section,
      .prompt-section,
      .responses-section {
        background: var(--offblack);
        border: 1px solid var(--border);
        border-radius: 0.5rem;
        padding: 1.5rem;
        margin-bottom: 2rem;
      }
      
      .machines-list,
      .sessions-list {
        display: grid;
        gap: 1rem;
      }
      
      .machine-card,
      .session-card {
        background: var(--black);
        border: 1px solid var(--border);
        border-radius: 0.25rem;
        padding: 1rem;
        display: flex;
        justify-content: space-between;
        align-items: center;
      }
      
      .machine-card.online {
        border-color: #10b981;
      }
      
      .machine-card.offline {
        border-color: #ef4444;
        opacity: 0.6;
      }
      
      .machine-info h4,
      .session-info h4 {
        margin: 0 0 0.5rem 0;
        color: var(--text);
      }
      
      .machine-info .details,
      .session-info .details {
        font-size: 0.875rem;
        color: var(--darkgray);
      }
      
      .status-badge {
        padding: 0.25rem 0.75rem;
        border-radius: 9999px;
        font-size: 0.75rem;
        font-weight: 500;
      }
      
      .status-badge.online {
        background: #10b98133;
        color: #10b981;
      }
      
      .status-badge.offline {
        background: #ef444433;
        color: #ef4444;
      }
      
      .form-group {
        margin-bottom: 1rem;
      }
      
      .form-group label {
        display: block;
        margin-bottom: 0.5rem;
        color: var(--darkgray);
        font-size: 0.875rem;
      }
      
      .form-group input,
      .form-group select,
      .form-group textarea {
        width: 100%;
        padding: 0.5rem;
        background: var(--black);
        border: 1px solid var(--border);
        border-radius: 0.25rem;
        color: var(--text);
        font-family: inherit;
      }
      
      .form-group input:focus,
      .form-group select:focus,
      .form-group textarea:focus {
        outline: none;
        border-color: var(--primary);
      }
      
      button {
        background: var(--primary);
        color: var(--black);
        border: none;
        border-radius: 0.25rem;
        padding: 0.5rem 1rem;
        font-weight: 500;
        cursor: pointer;
        font-family: inherit;
      }
      
      button:hover {
        opacity: 0.9;
      }
      
      button:disabled {
        opacity: 0.5;
        cursor: not-allowed;
      }
      
      .responses-list {
        max-height: 400px;
        overflow-y: auto;
        font-family: monospace;
        font-size: 0.875rem;
      }
      
      .response-item {
        background: var(--black);
        border: 1px solid var(--border);
        border-radius: 0.25rem;
        padding: 1rem;
        margin-bottom: 0.5rem;
      }
      
      .response-item .timestamp {
        color: var(--darkgray);
        font-size: 0.75rem;
        margin-bottom: 0.5rem;
      }
      
      .response-item .content {
        white-space: pre-wrap;
        word-break: break-word;
      }
      
      .loading,
      .empty {
        text-align: center;
        color: var(--darkgray);
        padding: 2rem;
      }
      
      .error {
        color: #ef4444;
        padding: 1rem;
        background: #ef444411;
        border: 1px solid #ef444433;
        border-radius: 0.25rem;
        margin-bottom: 1rem;
      }
    </style>
    
    <script type="module">
      // WebSocket connection
      let ws = null;
      let machines = new Map();
      let sessions = new Map();
      
      // Initialize WebSocket connection
      function initWebSocket() {
        const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
        const wsUrl = protocol + '//' + window.location.host + '/claude-code/client';
        
        ws = new WebSocket(wsUrl);
        
        ws.onopen = () => {
          console.log('Connected to Claude Code server');
          // Request initial machine list
          ws.send(JSON.stringify({
            type: 'query',
            query: { type: 'machines' }
          }));
        };
        
        ws.onmessage = (event) => {
          const message = JSON.parse(event.data);
          handleServerMessage(message);
        };
        
        ws.onerror = (error) => {
          console.error('WebSocket error:', error);
          showError('Connection error. Please refresh the page.');
        };
        
        ws.onclose = () => {
          console.log('Disconnected from Claude Code server');
          setTimeout(initWebSocket, 5000); // Reconnect after 5 seconds
        };
      }
      
      // Handle messages from server
      function handleServerMessage(message) {
        switch (message.type) {
          case 'machines':
            updateMachines(message.machines);
            break;
            
          case 'sessions':
            updateSessions(message.sessions);
            break;
            
          case 'machine_status':
            updateMachineStatus(message.machineId, message.status);
            break;
            
          case 'response':
            displayResponse(message.response);
            break;
            
          case 'error':
            showError(message.error);
            break;
        }
      }
      
      // Update machines list
      function updateMachines(machinesList) {
        machines.clear();
        machinesList.forEach(m => machines.set(m.machineId, m));
        
        const machinesDiv = document.getElementById('machines-list');
        const machineSelect = document.getElementById('machine-select');
        
        if (machinesList.length === 0) {
          machinesDiv.innerHTML = '<div class="empty">No machines connected</div>';
          machineSelect.innerHTML = '<option value="">No machines available</option>';
          return;
        }
        
        // Update UI
        machinesDiv.innerHTML = machinesList.map(machine => \`
          <div class="machine-card \${machine.status}">
            <div class="machine-info">
              <h4>\${machine.hostname}</h4>
              <div class="details">
                Claude \${machine.claudeVersion} • \${machine.activeSessions.length} sessions
              </div>
            </div>
            <span class="status-badge \${machine.status}">\${machine.status}</span>
          </div>
        \`).join('');
        
        // Update select
        machineSelect.innerHTML = '<option value="">Select a machine...</option>' +
          machinesList
            .filter(m => m.status === 'online')
            .map(m => \`<option value="\${m.machineId}">\${m.hostname}</option>\`)
            .join('');
      }
      
      // Update sessions list
      function updateSessions(sessionsList) {
        sessions.clear();
        sessionsList.forEach(s => sessions.set(s.sessionId, s));
        
        const sessionsDiv = document.getElementById('sessions-list');
        const sessionSelect = document.getElementById('session-select');
        const promptSection = document.querySelector('.prompt-section');
        
        if (sessionsList.length === 0) {
          sessionsDiv.innerHTML = '<div class="empty">No active sessions</div>';
          promptSection.style.display = 'none';
          return;
        }
        
        // Update UI
        sessionsDiv.innerHTML = sessionsList.map(session => \`
          <div class="session-card">
            <div class="session-info">
              <h4>\${session.projectName || session.projectPath}</h4>
              <div class="details">
                \${session.status} • \${session.messageCount} messages
              </div>
            </div>
            <button onclick="endSession('\${session.sessionId}', '\${session.machineId}')">
              End Session
            </button>
          </div>
        \`).join('');
        
        // Update select
        sessionSelect.innerHTML = '<option value="">Select a session...</option>' +
          sessionsList
            .filter(s => s.status === 'active')
            .map(s => \`<option value="\${s.sessionId}">\${s.projectName || s.sessionId}</option>\`)
            .join('');
            
        promptSection.style.display = 'block';
      }
      
      // Display response
      function displayResponse(response) {
        const responsesDiv = document.getElementById('responses-list');
        const timestamp = new Date(response.timestamp).toLocaleTimeString();
        
        const responseHtml = \`
          <div class="response-item">
            <div class="timestamp">\${timestamp} - \${response.type}</div>
            <div class="content">\${response.data.content || JSON.stringify(response.data, null, 2)}</div>
          </div>
        \`;
        
        if (responsesDiv.querySelector('.empty')) {
          responsesDiv.innerHTML = responseHtml;
        } else {
          responsesDiv.insertAdjacentHTML('afterbegin', responseHtml);
        }
      }
      
      // Show error
      function showError(message) {
        const errorDiv = document.createElement('div');
        errorDiv.className = 'error';
        errorDiv.textContent = message;
        document.querySelector('.claude-code-control').insertBefore(
          errorDiv,
          document.querySelector('.machines-section')
        );
        
        setTimeout(() => errorDiv.remove(), 5000);
      }
      
      // Form handlers
      document.getElementById('start-session-form').addEventListener('submit', async (e) => {
        e.preventDefault();
        const formData = new FormData(e.target);
        
        ws.send(JSON.stringify({
          type: 'command',
          command: {
            commandId: 'cmd_' + Date.now(),
            type: 'start_session',
            machineId: formData.get('machineId'),
            userId: 'user_' + Date.now(),
            timestamp: new Date(),
            data: {
              projectPath: formData.get('projectPath')
            }
          }
        }));
        
        e.target.reset();
      });
      
      document.getElementById('send-prompt-form').addEventListener('submit', async (e) => {
        e.preventDefault();
        const formData = new FormData(e.target);
        const session = sessions.get(formData.get('sessionId'));
        
        if (!session) {
          showError('Session not found');
          return;
        }
        
        ws.send(JSON.stringify({
          type: 'command',
          command: {
            commandId: 'cmd_' + Date.now(),
            type: 'send_prompt',
            machineId: session.machineId,
            sessionId: session.sessionId,
            userId: session.userId,
            timestamp: new Date(),
            data: {
              prompt: formData.get('prompt')
            }
          }
        }));
        
        e.target.reset();
      });
      
      // End session
      window.endSession = (sessionId, machineId) => {
        const session = sessions.get(sessionId);
        if (!session) return;
        
        ws.send(JSON.stringify({
          type: 'command',
          command: {
            commandId: 'cmd_' + Date.now(),
            type: 'end_session',
            machineId: machineId,
            sessionId: sessionId,
            userId: session.userId,
            timestamp: new Date(),
            data: {}
          }
        }));
      };
      
      // Initialize
      initWebSocket();
    </script>
  `
}
