import { css, document, html } from "@openagentsinc/psionic"
import { baseStyles } from "../styles"

const settingsStyles = css`
  /* V1 Color Palette */
  :root {
    --text: #D7D8E5;
    --offblack: #1e1e1e;
    --darkgray: #3D3D40;
    --gray: #8B8585;
    --lightgray: #A7A7A7;
    --white: #fff;
    --black: #000000;
    --input-border: #3D3E42;
    --placeholder: #777A81;
    --success: #10b981;
    --warning: #f59e0b;
    --error: #ef4444;
  }

  body {
    background-color: var(--black) !important;
    color: var(--white) !important;
    font-family: "Berkeley Mono", "JetBrains Mono", ui-monospace, monospace !important;
  }

  .settings-container {
    max-width: 800px;
    margin: 0 auto;
    padding: 2rem;
    padding-top: 5rem;
  }

  .settings-header {
    text-align: center;
    margin-bottom: 3rem;
  }

  .settings-title {
    font-size: 2.5rem;
    color: var(--white);
    margin-bottom: 1rem;
  }

  .settings-subtitle {
    font-size: 1.1rem;
    color: var(--gray);
  }

  .settings-nav {
    display: flex;
    gap: 0.5rem;
    margin-bottom: 2rem;
    background: var(--offblack);
    border-radius: 8px;
    padding: 0.5rem;
  }

  .nav-tab {
    flex: 1;
    padding: 0.75rem 1rem;
    background: transparent;
    color: var(--gray);
    border: none;
    border-radius: 6px;
    cursor: pointer;
    font-family: inherit;
    font-size: 0.875rem;
    transition: all 0.2s ease;
  }

  .nav-tab.active {
    background: var(--white);
    color: var(--black);
  }

  .nav-tab:hover:not(.active) {
    background: var(--darkgray);
    color: var(--white);
  }

  .settings-section {
    background: var(--offblack);
    border: 1px solid var(--darkgray);
    border-radius: 12px;
    padding: 2rem;
    margin-bottom: 2rem;
  }

  .section-header {
    margin-bottom: 1.5rem;
    padding-bottom: 1rem;
    border-bottom: 1px solid var(--darkgray);
  }

  .section-title {
    font-size: 1.2rem;
    color: var(--white);
    margin: 0 0 0.5rem 0;
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .section-description {
    color: var(--gray);
    font-size: 0.875rem;
    margin: 0;
    line-height: 1.5;
  }

  .form-group {
    margin-bottom: 1.5rem;
  }

  .form-group:last-child {
    margin-bottom: 0;
  }

  .form-label {
    display: block;
    color: var(--white);
    font-weight: 600;
    margin-bottom: 0.5rem;
    font-size: 0.875rem;
  }

  .form-input {
    width: 100%;
    padding: 0.75rem 1rem;
    background: var(--black);
    border: 2px solid var(--darkgray);
    border-radius: 6px;
    color: var(--white);
    font-family: inherit;
    font-size: 0.875rem;
    transition: border-color 0.3s ease;
    box-sizing: border-box;
  }

  .form-input:focus {
    outline: none;
    border-color: var(--white);
  }

  .form-input::placeholder {
    color: var(--placeholder);
  }

  .form-textarea {
    resize: vertical;
    min-height: 100px;
  }

  .form-select {
    cursor: pointer;
    appearance: none;
    background-image: url("data:image/svg+xml;charset=utf-8,%3Csvg xmlns='http://www.w3.org/2000/svg' fill='none' viewBox='0 0 20 20'%3E%3Cpath stroke='%236b7280' stroke-linecap='round' stroke-linejoin='round' stroke-width='1.5' d='M6 8l4 4 4-4'/%3E%3C/svg%3E");
    background-position: right 0.5rem center;
    background-repeat: no-repeat;
    background-size: 1.5em 1.5em;
    padding-right: 2.5rem;
  }

  .form-row {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 1rem;
  }

  .checkbox-group {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin-bottom: 0.5rem;
  }

  .checkbox-group input[type="checkbox"] {
    width: auto;
    margin: 0;
  }

  .help-text {
    color: var(--gray);
    font-size: 0.75rem;
    margin-top: 0.25rem;
    line-height: 1.4;
  }

  .btn {
    padding: 0.75rem 1.5rem;
    border-radius: 6px;
    font-family: inherit;
    font-size: 0.875rem;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.2s ease;
    text-decoration: none;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border: none;
    gap: 0.5rem;
  }

  .btn-primary {
    background: var(--white);
    color: var(--black);
  }

  .btn-primary:hover:not(:disabled) {
    background: var(--lightgray);
  }

  .btn-primary:disabled {
    background: var(--darkgray);
    color: var(--gray);
    cursor: not-allowed;
  }

  .btn-secondary {
    background: transparent;
    color: var(--white);
    border: 2px solid var(--darkgray);
  }

  .btn-secondary:hover {
    background: var(--darkgray);
  }

  .btn-danger {
    background: var(--error);
    color: var(--white);
  }

  .btn-danger:hover {
    background: #dc2626;
  }

  .wallet-info {
    background: var(--black);
    border: 1px solid var(--darkgray);
    border-radius: 8px;
    padding: 1.5rem;
    margin-bottom: 1.5rem;
  }

  .wallet-balance {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 1rem;
  }

  .balance-label {
    color: var(--gray);
    font-size: 0.875rem;
  }

  .balance-value {
    color: var(--white);
    font-size: 1.5rem;
    font-weight: 600;
  }

  .wallet-address {
    background: var(--offblack);
    border: 1px solid var(--darkgray);
    border-radius: 6px;
    padding: 0.75rem;
    font-family: "Berkeley Mono", monospace;
    font-size: 0.75rem;
    color: var(--text);
    word-break: break-all;
    margin-bottom: 1rem;
  }

  .wallet-actions {
    display: flex;
    gap: 1rem;
  }

  .success-message {
    background: rgba(16, 185, 129, 0.1);
    border: 1px solid var(--success);
    border-radius: 6px;
    padding: 1rem;
    color: var(--success);
    margin-bottom: 1rem;
  }

  .danger-zone {
    border: 1px solid var(--error);
    border-radius: 8px;
    padding: 1.5rem;
    background: rgba(239, 68, 68, 0.05);
  }

  .danger-zone h4 {
    color: var(--error);
    margin: 0 0 1rem 0;
    font-size: 1rem;
  }

  .tab-content {
    display: none;
  }

  .tab-content.active {
    display: block;
  }

  /* Responsive */
  @media (max-width: 768px) {
    .settings-container {
      padding: 1rem;
      padding-top: 4rem;
    }

    .settings-section {
      padding: 1.5rem;
    }

    .form-row {
      grid-template-columns: 1fr;
    }

    .wallet-actions {
      flex-direction: column;
    }

    .settings-nav {
      flex-direction: column;
    }

    .settings-title {
      font-size: 2rem;
    }
  }
`

export async function settings() {
  return document({
    title: "Settings - OpenAgents",
    styles: baseStyles + settingsStyles,
    body: html`
      <!-- Fixed Header -->
      <div style="position: fixed; top: 0; left: 0; right: 0; height: 52px; display: flex; align-items: center; justify-content: space-between; padding: 0 20px; z-index: 20; background: black; border-bottom: 1px solid var(--darkgray);">
        <div style="display: flex; align-items: center; gap: 20px;">
          <a href="/" style="color: white; text-decoration: none; font-size: 18px; font-weight: 600;">OpenAgents</a>
          <nav style="display: flex; gap: 20px;">
            <a href="/agents" style="color: var(--gray); text-decoration: none; font-size: 14px;">My Agents</a>
            <a href="/store" style="color: var(--gray); text-decoration: none; font-size: 14px;">Store</a>
            <a href="/create" style="color: var(--gray); text-decoration: none; font-size: 14px;">Create</a>
          </nav>
        </div>
        <a href="/settings" style="color: var(--white); text-decoration: none; font-size: 14px;">Settings</a>
      </div>

      <div class="settings-container">
        <!-- Header -->
        <div class="settings-header">
          <h1 class="settings-title">⚙️ Settings</h1>
          <p class="settings-subtitle">Manage your account, preferences, and wallet</p>
        </div>

        <!-- Navigation Tabs -->
        <div class="settings-nav">
          <button class="nav-tab active" onclick="showTab('profile')">Profile</button>
          <button class="nav-tab" onclick="showTab('chat')">Chat</button>
          <button class="nav-tab" onclick="showTab('wallet')">Wallet</button>
          <button class="nav-tab" onclick="showTab('account')">Account</button>
        </div>

        <!-- Profile Settings -->
        <div id="profile-tab" class="tab-content active">
          <div class="settings-section">
            <div class="section-header">
              <h3 class="section-title">
                👤 Profile Information
              </h3>
              <p class="section-description">
                Update your public profile information that other users will see.
              </p>
            </div>

            <form id="profile-form">
              <div class="form-group">
                <label class="form-label" for="username">Username</label>
                <input type="text" id="username" class="form-input" value="your_username" placeholder="Enter username">
                <div class="help-text">Your unique identifier on the platform. This cannot be changed.</div>
              </div>

              <div class="form-group">
                <label class="form-label" for="display-name">Display Name</label>
                <input type="text" id="display-name" class="form-input" value="Your Name" placeholder="Enter display name">
                <div class="help-text">The name that appears on your profile and agents.</div>
              </div>

              <div class="form-group">
                <label class="form-label" for="bio">Bio</label>
                <textarea id="bio" class="form-input form-textarea" placeholder="Tell others about yourself...">AI enthusiast and agent creator building the future of autonomous systems.</textarea>
                <div class="help-text">A brief description about yourself and your interests.</div>
              </div>

              <button type="submit" class="btn btn-primary">Save Profile</button>
            </form>
          </div>
        </div>

        <!-- Chat Settings -->
        <div id="chat-tab" class="tab-content">
          <div class="settings-section">
            <div class="section-header">
              <h3 class="section-title">
                💬 Chat Preferences
              </h3>
              <p class="section-description">
                Configure your default chat settings and AI model preferences.
              </p>
            </div>

            <form id="chat-form">
              <div class="form-group">
                <label class="form-label" for="default-model">Default AI Model</label>
                <select id="default-model" class="form-input form-select">
                  <option value="llama-3.3-70b" selected>Llama 3.3 70B (Recommended)</option>
                  <option value="llama-3.1-8b">Llama 3.1 8B (Faster)</option>
                  <option value="llama-3.2-1b">Llama 3.2 1B (Cheapest)</option>
                </select>
                <div class="help-text">The AI model to use by default for new conversations.</div>
              </div>

              <div class="form-group">
                <label class="form-label" for="system-prompt">Personal System Prompt</label>
                <textarea id="system-prompt" class="form-input form-textarea" placeholder="You are a helpful assistant that...">You are a helpful assistant focused on Bitcoin, Lightning Network, and autonomous agents. Always provide accurate information and help users understand complex concepts.</textarea>
                <div class="help-text">Custom instructions that will be added to all your conversations.</div>
              </div>

              <div class="form-group">
                <div class="checkbox-group">
                  <input type="checkbox" id="save-history" checked>
                  <label class="form-label" for="save-history">Save chat history</label>
                </div>
                <div class="help-text">Keep a history of your conversations for future reference.</div>
              </div>

              <div class="form-group">
                <div class="checkbox-group">
                  <input type="checkbox" id="auto-title" checked>
                  <label class="form-label" for="auto-title">Auto-generate chat titles</label>
                </div>
                <div class="help-text">Automatically create descriptive titles for your chat threads.</div>
              </div>

              <button type="submit" class="btn btn-primary">Save Chat Settings</button>
            </form>
          </div>
          
          <!-- API Keys Section -->
          <div class="settings-section">
            <div class="section-header">
              <h3 class="section-title">
                🔑 API Keys
              </h3>
              <p class="section-description">
                Add API keys to use additional AI providers like OpenRouter for Claude and GPT models.
              </p>
            </div>

            <form id="api-keys-form">
              <div class="form-group">
                <label class="form-label" for="openrouter-key">OpenRouter API Key</label>
                <div style="display: flex; gap: 0.5rem;">
                  <input 
                    type="password" 
                    id="openrouter-key" 
                    class="form-input" 
                    placeholder="sk-or-v1-..." 
                    style="flex: 1;"
                  >
                  <button type="button" id="toggle-key-visibility" class="btn btn-secondary" style="padding: 0.75rem;">
                    <svg width="20" height="20" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="2">
                      <path d="M1 10s3-6 9-6 9 6 9 6-3 6-9 6-9-6-9-6z"/>
                      <circle cx="10" cy="10" r="3"/>
                    </svg>
                  </button>
                </div>
                <div class="help-text">
                  Get your API key from <a href="https://openrouter.ai/keys" target="_blank" style="color: var(--white); text-decoration: underline;">openrouter.ai/keys</a>. 
                  This enables access to Claude, GPT-4, and other premium models.
                </div>
              </div>
              
              <div id="api-key-status" style="display: none; margin-bottom: 1rem; padding: 1rem; border-radius: 8px; font-size: 0.875rem;">
                <!-- Status message will be shown here -->
              </div>

              <button type="submit" class="btn btn-primary">Save API Key</button>
            </form>
          </div>
        </div>

        <!-- Wallet Settings -->
        <div id="wallet-tab" class="tab-content">
          <div class="settings-section">
            <div class="section-header">
              <h3 class="section-title">
                ⚡ Bitcoin Wallet
              </h3>
              <p class="section-description">
                Manage your Bitcoin balance and Lightning Network integration.
              </p>
            </div>

            <!-- Wallet Info -->
            <div class="wallet-info">
              <div class="wallet-balance">
                <span class="balance-label">Available Balance</span>
                <span class="balance-value">25,847 sats</span>
              </div>
              
              <div class="form-group">
                <label class="form-label">Lightning Address</label>
                <div class="wallet-address">your_username@openagents.com</div>
                <div class="help-text">Your Lightning address for receiving payments.</div>
              </div>

              <div class="wallet-actions">
                <button class="btn btn-primary" onclick="depositFunds()">
                  💰 Deposit Funds
                </button>
                <button class="btn btn-secondary" onclick="withdrawFunds()">
                  📤 Withdraw
                </button>
              </div>
            </div>

            <form id="wallet-form">
              <div class="form-group">
                <div class="checkbox-group">
                  <input type="checkbox" id="auto-fund" checked>
                  <label class="form-label" for="auto-fund">Auto-fund from Lightning address</label>
                </div>
                <div class="help-text">Automatically add received payments to your chat balance.</div>
              </div>

              <div class="form-group">
                <label class="form-label" for="spending-limit">Daily Spending Limit (sats)</label>
                <input type="number" id="spending-limit" class="form-input" value="1000" min="0">
                <div class="help-text">Maximum amount you can spend on chats per day.</div>
              </div>

              <button type="submit" class="btn btn-primary">Save Wallet Settings</button>
            </form>
          </div>
        </div>

        <!-- Account Settings -->
        <div id="account-tab" class="tab-content">
          <div class="settings-section">
            <div class="section-header">
              <h3 class="section-title">
                🔐 Account Security
              </h3>
              <p class="section-description">
                Manage your account security and authentication methods.
              </p>
            </div>

            <form id="account-form">
              <div class="form-group">
                <label class="form-label" for="email">Email Address</label>
                <input type="email" id="email" class="form-input" value="your@email.com" placeholder="Enter email">
                <div class="help-text">Used for important account notifications.</div>
              </div>

              <div class="form-group">
                <label class="form-label" for="nostr-pubkey">Nostr Public Key</label>
                <input type="text" id="nostr-pubkey" class="form-input" value="npub1..." placeholder="Enter Nostr public key" readonly>
                <div class="help-text">Your Nostr identity for decentralized authentication.</div>
              </div>

              <div class="form-group">
                <div class="checkbox-group">
                  <input type="checkbox" id="email-notifications">
                  <label class="form-label" for="email-notifications">Email notifications</label>
                </div>
                <div class="help-text">Receive important updates via email.</div>
              </div>

              <button type="submit" class="btn btn-primary">Save Account Settings</button>
            </form>

            <!-- Danger Zone -->
            <div class="danger-zone">
              <h4>⚠️ Danger Zone</h4>
              <p style="color: var(--gray); margin-bottom: 1rem;">These actions cannot be undone.</p>
              
              <div style="display: flex; gap: 1rem; flex-wrap: wrap;">
                <button class="btn btn-danger" onclick="exportData()">
                  📦 Export Data
                </button>
                <button class="btn btn-danger" onclick="deleteAccount()">
                  🗑️ Delete Account
                </button>
              </div>
            </div>
          </div>
        </div>
      </div>

      <script>
        // Tab switching
        function showTab(tabName) {
          // Hide all tab contents
          document.querySelectorAll('.tab-content').forEach(tab => {
            tab.classList.remove('active')
          })
          
          // Remove active class from all tabs
          document.querySelectorAll('.nav-tab').forEach(tab => {
            tab.classList.remove('active')
          })
          
          // Show selected tab
          document.getElementById(tabName + '-tab').classList.add('active')
          
          // Add active class to clicked tab
          event.target.classList.add('active')
        }

        // Form submission handlers
        document.getElementById('profile-form').addEventListener('submit', (e) => {
          e.preventDefault()
          showSuccessMessage('Profile updated successfully!')
        })

        document.getElementById('chat-form').addEventListener('submit', (e) => {
          e.preventDefault()
          showSuccessMessage('Chat settings saved!')
        })

        document.getElementById('wallet-form').addEventListener('submit', (e) => {
          e.preventDefault()
          showSuccessMessage('Wallet settings updated!')
        })

        document.getElementById('account-form').addEventListener('submit', (e) => {
          e.preventDefault()
          showSuccessMessage('Account settings saved!')
        })

        // Success message helper
        function showSuccessMessage(message) {
          // Remove existing success messages
          document.querySelectorAll('.success-message').forEach(msg => msg.remove())
          
          // Create new success message
          const successDiv = document.createElement('div')
          successDiv.className = 'success-message'
          successDiv.textContent = message
          
          // Insert at top of active tab
          const activeTab = document.querySelector('.tab-content.active')
          const firstSection = activeTab.querySelector('.settings-section')
          firstSection.insertBefore(successDiv, firstSection.firstChild)
          
          // Remove after 3 seconds
          setTimeout(() => {
            successDiv.remove()
          }, 3000)
        }

        // Wallet actions
        function depositFunds() {
          const amount = prompt('Enter amount to deposit (sats):', '10000')
          if (amount && !isNaN(amount)) {
            // In real implementation, this would show a Lightning invoice
            alert(\`Deposit request for \${amount} sats created. You would see a Lightning invoice here.\`)
          }
        }

        function withdrawFunds() {
          const amount = prompt('Enter amount to withdraw (sats):', '1000')
          if (amount && !isNaN(amount)) {
            const address = prompt('Enter Lightning invoice or address:')
            if (address) {
              alert(\`Withdrawal of \${amount} sats to \${address.slice(0, 20)}... initiated.\`)
            }
          }
        }

        // Account actions
        function exportData() {
          if (confirm('Export all your data? This will create a download with your profile, chats, and agents.')) {
            alert('Data export started. You will receive a download link via email.')
          }
        }

        function deleteAccount() {
          const confirmation = prompt('Type "DELETE" to confirm account deletion:')
          if (confirmation === 'DELETE') {
            alert('Account deletion initiated. All data will be permanently removed within 30 days.')
          }
        }
        
        // API Key Management
        document.addEventListener('DOMContentLoaded', () => {
          // Load existing API key
          const savedKey = localStorage.getItem('openrouterApiKey')
          if (savedKey) {
            document.getElementById('openrouter-key').value = savedKey
          }
          
          // Toggle key visibility
          document.getElementById('toggle-key-visibility').addEventListener('click', () => {
            const input = document.getElementById('openrouter-key')
            const isPassword = input.type === 'password'
            input.type = isPassword ? 'text' : 'password'
            
            // Update icon
            const button = document.getElementById('toggle-key-visibility')
            if (isPassword) {
              button.innerHTML = '<svg width="20" height="20" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="2"><path d="M17.94 17.94A10.07 10.07 0 0112 20c-7 0-11-8-11-8a18.45 18.45 0 015.06-5.94"/><path d="M1 1l18 18"/></svg>'
            } else {
              button.innerHTML = '<svg width="20" height="20" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="2"><path d="M1 10s3-6 9-6 9 6 9 6-3 6-9 6-9-6-9-6z"/><circle cx="10" cy="10" r="3"/></svg>'
            }
          })
          
          // Handle API key form submission
          document.getElementById('api-keys-form').addEventListener('submit', async (e) => {
            e.preventDefault()
            
            const keyInput = document.getElementById('openrouter-key')
            const apiKey = keyInput.value.trim()
            const statusDiv = document.getElementById('api-key-status')
            
            if (!apiKey) {
              // Clear the key
              localStorage.removeItem('openrouterApiKey')
              statusDiv.style.display = 'block'
              statusDiv.style.backgroundColor = 'var(--darkgray)'
              statusDiv.style.color = 'var(--white)'
              statusDiv.textContent = 'API key removed successfully'
              
              setTimeout(() => {
                statusDiv.style.display = 'none'
              }, 3000)
              return
            }
            
            // Save the key
            localStorage.setItem('openrouterApiKey', apiKey)
            
            // Test the key
            statusDiv.style.display = 'block'
            statusDiv.style.backgroundColor = 'var(--darkgray)'
            statusDiv.style.color = 'var(--lightgray)'
            statusDiv.textContent = 'Testing API key...'
            
            try {
              const response = await fetch('/api/openrouter/status', {
                headers: {
                  'x-api-key': apiKey
                }
              })
              
              if (response.ok) {
                statusDiv.style.backgroundColor = 'rgba(16, 185, 129, 0.1)'
                statusDiv.style.color = 'var(--success)'
                statusDiv.textContent = '✓ API key validated successfully! You can now use OpenRouter models.'
              } else {
                statusDiv.style.backgroundColor = 'rgba(239, 68, 68, 0.1)'
                statusDiv.style.color = 'var(--error)'
                statusDiv.textContent = '✗ Invalid API key. Please check your key and try again.'
                localStorage.removeItem('openrouterApiKey')
              }
            } catch (error) {
              statusDiv.style.backgroundColor = 'rgba(239, 68, 68, 0.1)'
              statusDiv.style.color = 'var(--error)'
              statusDiv.textContent = '✗ Failed to validate API key. Please try again.'
              localStorage.removeItem('openrouterApiKey')
            }
            
            setTimeout(() => {
              statusDiv.style.display = 'none'
            }, 5000)
          })
        })
      </script>
    `
  })
}
