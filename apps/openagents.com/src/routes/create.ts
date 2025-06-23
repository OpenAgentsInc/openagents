import { css, document, html } from "@openagentsinc/psionic"
import { baseStyles } from "../styles"

const createStyles = css`
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

  .create-container {
    max-width: 800px;
    margin: 0 auto;
    padding: 2rem;
    padding-top: 5rem;
  }

  .create-header {
    text-align: center;
    margin-bottom: 3rem;
  }

  .create-title {
    font-size: 2.5rem;
    color: var(--white);
    margin-bottom: 1rem;
  }

  .create-subtitle {
    font-size: 1.1rem;
    color: var(--gray);
    margin-bottom: 2rem;
  }

  .form-container {
    background: var(--offblack);
    border: 1px solid var(--darkgray);
    border-radius: 12px;
    padding: 2.5rem;
  }

  .form-section {
    margin-bottom: 2rem;
  }

  .form-section:last-child {
    margin-bottom: 0;
  }

  .section-title {
    font-size: 1.2rem;
    color: var(--white);
    margin-bottom: 0.5rem;
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .section-description {
    color: var(--gray);
    font-size: 0.875rem;
    margin-bottom: 1.5rem;
    line-height: 1.5;
  }

  .form-group {
    margin-bottom: 1.5rem;
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
    font-family: inherit;
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

  .image-upload {
    display: flex;
    align-items: center;
    gap: 1rem;
  }

  .image-preview {
    width: 64px;
    height: 64px;
    background: var(--darkgray);
    border-radius: 8px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 2rem;
    border: 2px solid var(--darkgray);
  }

  .image-upload-btn {
    background: var(--darkgray);
    color: var(--white);
    border: none;
    padding: 0.5rem 1rem;
    border-radius: 6px;
    cursor: pointer;
    font-family: inherit;
    font-size: 0.875rem;
    transition: background-color 0.3s ease;
  }

  .image-upload-btn:hover {
    background: var(--gray);
  }

  .pricing-display {
    background: var(--black);
    border: 1px solid var(--darkgray);
    border-radius: 6px;
    padding: 1rem;
    margin-top: 1rem;
  }

  .pricing-item {
    display: flex;
    justify-content: space-between;
    margin-bottom: 0.5rem;
    font-size: 0.875rem;
  }

  .pricing-item:last-child {
    margin-bottom: 0;
    font-weight: 600;
    color: var(--white);
    border-top: 1px solid var(--darkgray);
    padding-top: 0.5rem;
    margin-top: 0.5rem;
  }

  .pricing-label {
    color: var(--gray);
  }

  .pricing-value {
    color: var(--white);
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

  .form-actions {
    display: flex;
    gap: 1rem;
    justify-content: flex-end;
    margin-top: 2rem;
    padding-top: 2rem;
    border-top: 1px solid var(--darkgray);
  }

  .preview-section {
    background: var(--black);
    border: 1px solid var(--darkgray);
    border-radius: 8px;
    padding: 1.5rem;
    margin-top: 1rem;
  }

  .preview-title {
    color: var(--white);
    font-size: 0.875rem;
    font-weight: 600;
    margin-bottom: 1rem;
  }

  .agent-preview {
    background: var(--offblack);
    border: 1px solid var(--darkgray);
    border-radius: 8px;
    padding: 1.5rem;
  }

  .preview-header {
    display: flex;
    align-items: center;
    gap: 1rem;
    margin-bottom: 1rem;
  }

  .preview-avatar {
    width: 40px;
    height: 40px;
    background: var(--darkgray);
    border-radius: 6px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 1.25rem;
  }

  .preview-info h4 {
    margin: 0;
    color: var(--white);
    font-size: 1rem;
  }

  .preview-info p {
    margin: 0;
    color: var(--gray);
    font-size: 0.75rem;
  }

  .preview-description {
    color: var(--text);
    line-height: 1.5;
    margin-bottom: 1rem;
  }

  .preview-greeting {
    background: var(--black);
    border-left: 3px solid var(--white);
    padding: 0.75rem;
    color: var(--text);
    font-style: italic;
    border-radius: 0 6px 6px 0;
  }

  .help-text {
    color: var(--gray);
    font-size: 0.75rem;
    margin-top: 0.25rem;
    line-height: 1.4;
  }

  .success-message {
    background: rgba(16, 185, 129, 0.1);
    border: 1px solid var(--success);
    border-radius: 6px;
    padding: 1rem;
    color: var(--success);
    margin-bottom: 1rem;
  }

  /* Responsive */
  @media (max-width: 768px) {
    .create-container {
      padding: 1rem;
      padding-top: 4rem;
    }

    .form-container {
      padding: 1.5rem;
    }

    .form-row {
      grid-template-columns: 1fr;
    }

    .form-actions {
      flex-direction: column;
    }

    .create-title {
      font-size: 2rem;
    }
  }
`

export async function create() {
  return document({
    title: "Create Agent - OpenAgents",
    styles: baseStyles + createStyles,
    body: html`
      <!-- Fixed Header -->
      <div style="position: fixed; top: 0; left: 0; right: 0; height: 52px; display: flex; align-items: center; justify-content: space-between; padding: 0 20px; z-index: 20; background: black; border-bottom: 1px solid var(--darkgray);">
        <div style="display: flex; align-items: center; gap: 20px;">
          <a href="/" style="color: white; text-decoration: none; font-size: 18px; font-weight: 600;">OpenAgents</a>
          <nav style="display: flex; gap: 20px;">
            <a href="/agents" style="color: var(--gray); text-decoration: none; font-size: 14px;">My Agents</a>
            <a href="/store" style="color: var(--gray); text-decoration: none; font-size: 14px;">Store</a>
            <a href="/create" style="color: var(--white); text-decoration: none; font-size: 14px;">Create</a>
          </nav>
        </div>
        <a href="/settings" style="color: var(--gray); text-decoration: none; font-size: 14px;">Settings</a>
      </div>

      <div class="create-container">
        <!-- Header -->
        <div class="create-header">
          <h1 class="create-title">🤖 Create New Agent</h1>
          <p class="create-subtitle">Build an autonomous AI agent that can earn Bitcoin by helping users</p>
        </div>

        <!-- Success Message (hidden by default) -->
        <div id="success-message" class="success-message" style="display: none;">
          <strong>Agent created successfully!</strong> Your agent is now available in your dashboard and will appear in the public store once approved.
        </div>

        <!-- Creation Form -->
        <form class="form-container" id="agent-form">
          <!-- Basic Information -->
          <div class="form-section">
            <h3 class="section-title">
              📝 Basic Information
            </h3>
            <p class="section-description">
              Set up your agent's identity and public profile information.
            </p>

            <div class="form-group">
              <label class="form-label" for="agent-name">Agent Name *</label>
              <input type="text" id="agent-name" class="form-input" placeholder="Bitcoin Analyst" required>
              <div class="help-text">Choose a clear, descriptive name that tells users what your agent does.</div>
            </div>

            <div class="form-group">
              <label class="form-label" for="agent-description">Description *</label>
              <textarea id="agent-description" class="form-input form-textarea" placeholder="Expert analyst providing Bitcoin market insights, technical analysis, and trading strategies." required></textarea>
              <div class="help-text">Explain what your agent does and what value it provides to users.</div>
            </div>

            <div class="form-group">
              <label class="form-label" for="agent-greeting">Greeting Message *</label>
              <textarea id="agent-greeting" class="form-input form-textarea" placeholder="Hi! I'm a Bitcoin market analyst ready to help you understand crypto markets and make informed decisions." required></textarea>
              <div class="help-text">The first message users will see when they start chatting with your agent.</div>
            </div>

            <div class="form-group">
              <label class="form-label">Profile Image</label>
              <div class="image-upload">
                <div class="image-preview" id="image-preview">🤖</div>
                <div>
                  <button type="button" class="image-upload-btn" onclick="selectEmoji()">Choose Emoji</button>
                  <div class="help-text">Select an emoji to represent your agent</div>
                </div>
              </div>
            </div>
          </div>

          <!-- Instructions & Behavior -->
          <div class="form-section">
            <h3 class="section-title">
              🧠 Instructions & Behavior
            </h3>
            <p class="section-description">
              Define how your agent should behave and respond to users.
            </p>

            <div class="form-group">
              <label class="form-label" for="agent-prompt">System Prompt *</label>
              <textarea id="agent-prompt" class="form-input form-textarea" style="min-height: 120px;" placeholder="You are a Bitcoin market analyst with deep expertise in cryptocurrency markets, technical analysis, and trading strategies. Always provide data-driven insights and help users make informed decisions. Stay up-to-date with market trends and explain complex concepts in simple terms." required></textarea>
              <div class="help-text">Detailed instructions that define your agent's personality, expertise, and how it should respond.</div>
            </div>
          </div>

          <!-- Model & Pricing -->
          <div class="form-section">
            <h3 class="section-title">
              ⚙️ Model & Pricing
            </h3>
            <p class="section-description">
              Configure which AI model to use and how much to charge per message.
            </p>

            <div class="form-row">
              <div class="form-group">
                <label class="form-label" for="agent-model">AI Model *</label>
                <select id="agent-model" class="form-input form-select" required>
                  <option value="llama-3.3-70b">Llama 3.3 70B (Recommended)</option>
                  <option value="llama-3.1-8b">Llama 3.1 8B (Faster)</option>
                  <option value="llama-3.2-1b">Llama 3.2 1B (Cheapest)</option>
                </select>
              </div>

              <div class="form-group">
                <label class="form-label" for="agent-pricing">Price per Message (sats) *</label>
                <input type="number" id="agent-pricing" class="form-input" value="100" min="1" max="10000" required>
              </div>
            </div>

            <div class="pricing-display">
              <div class="pricing-item">
                <span class="pricing-label">Model Cost:</span>
                <span class="pricing-value" id="model-cost">~50 sats</span>
              </div>
              <div class="pricing-item">
                <span class="pricing-label">Platform Fee (10%):</span>
                <span class="pricing-value" id="platform-fee">~10 sats</span>
              </div>
              <div class="pricing-item">
                <span class="pricing-label">Your Earnings:</span>
                <span class="pricing-value" id="creator-earnings">~40 sats</span>
              </div>
            </div>
          </div>

          <!-- Privacy Settings -->
          <div class="form-section">
            <h3 class="section-title">
              🔒 Privacy & Availability
            </h3>
            <p class="section-description">
              Control who can use your agent and how it appears in the store.
            </p>

            <div class="form-group">
              <label class="form-label">
                <input type="checkbox" id="agent-public" checked style="margin-right: 0.5rem;">
                Make agent publicly available in the store
              </label>
              <div class="help-text">When enabled, anyone can find and chat with your agent.</div>
            </div>
          </div>

          <!-- Live Preview -->
          <div class="form-section">
            <h3 class="section-title">
              👁️ Preview
            </h3>
            <p class="section-description">
              See how your agent will appear to users in the store.
            </p>

            <div class="preview-section">
              <div class="preview-title">Store Card Preview</div>
              <div class="agent-preview" id="agent-preview">
                <div class="preview-header">
                  <div class="preview-avatar" id="preview-avatar">🤖</div>
                  <div class="preview-info">
                    <h4 id="preview-name">Your Agent Name</h4>
                    <p>by @you</p>
                  </div>
                </div>
                <div class="preview-description" id="preview-description">
                  Enter a description to see how it looks...
                </div>
                <div class="preview-greeting" id="preview-greeting">
                  Enter a greeting message...
                </div>
              </div>
            </div>
          </div>

          <!-- Form Actions -->
          <div class="form-actions">
            <a href="/agents" class="btn btn-secondary">Cancel</a>
            <button type="submit" class="btn btn-primary" id="create-btn">
              🚀 Create Agent
            </button>
          </div>
        </form>
      </div>

      <script>
        // Form elements
        const form = document.getElementById('agent-form')
        const nameInput = document.getElementById('agent-name')
        const descriptionInput = document.getElementById('agent-description')
        const greetingInput = document.getElementById('agent-greeting')
        const promptInput = document.getElementById('agent-prompt')
        const modelSelect = document.getElementById('agent-model')
        const pricingInput = document.getElementById('agent-pricing')
        const publicCheckbox = document.getElementById('agent-public')

        // Preview elements
        const previewAvatar = document.getElementById('preview-avatar')
        const previewName = document.getElementById('preview-name')
        const previewDescription = document.getElementById('preview-description')
        const previewGreeting = document.getElementById('preview-greeting')

        // Pricing elements
        const modelCostElement = document.getElementById('model-cost')
        const platformFeeElement = document.getElementById('platform-fee')
        const creatorEarningsElement = document.getElementById('creator-earnings')

        // Current emoji selection
        let selectedEmoji = '🤖'

        // Model costs (in sats per message)
        const modelCosts = {
          'llama-3.3-70b': 50,
          'llama-3.1-8b': 20,
          'llama-3.2-1b': 5
        }

        // Update preview in real-time
        function updatePreview() {
          previewName.textContent = nameInput.value || 'Your Agent Name'
          previewDescription.textContent = descriptionInput.value || 'Enter a description to see how it looks...'
          previewGreeting.textContent = greetingInput.value || 'Enter a greeting message...'
          previewAvatar.textContent = selectedEmoji
        }

        // Update pricing calculation
        function updatePricing() {
          const basePrice = parseInt(pricingInput.value) || 100
          const modelCost = modelCosts[modelSelect.value] || 50
          const platformFee = Math.floor(basePrice * 0.1)
          const creatorEarnings = basePrice - modelCost - platformFee

          modelCostElement.textContent = \`~\${modelCost} sats\`
          platformFeeElement.textContent = \`~\${platformFee} sats\`
          creatorEarningsElement.textContent = \`~\${Math.max(0, creatorEarnings)} sats\`
        }

        // Emoji selection
        const emojis = ['🤖', '💻', '⚡', '🟠', '🟣', '🔥', '⭐', '💎', '🚀', '🦀', '🐍', '☕', '📊', '💰', '🎯', '🧪', '🔬', '🎨', '📝', '🛠️']
        
        function selectEmoji() {
          const emoji = prompt('Choose an emoji for your agent:\\n\\n' + emojis.join(' ') + '\\n\\nEnter an emoji:')
          if (emoji && emoji.trim()) {
            selectedEmoji = emoji.trim()
            document.getElementById('image-preview').textContent = selectedEmoji
            updatePreview()
          }
        }

        // Event listeners
        nameInput.addEventListener('input', updatePreview)
        descriptionInput.addEventListener('input', updatePreview)
        greetingInput.addEventListener('input', updatePreview)
        pricingInput.addEventListener('input', updatePricing)
        modelSelect.addEventListener('change', updatePricing)

        // Form submission
        form.addEventListener('submit', async (e) => {
          e.preventDefault()
          
          const createBtn = document.getElementById('create-btn')
          const originalText = createBtn.innerHTML
          
          // Show loading state
          createBtn.disabled = true
          createBtn.innerHTML = '⏳ Creating...'
          
          // Simulate API call
          setTimeout(() => {
            // In real implementation, this would call the API
            console.log('Agent created:', {
              name: nameInput.value,
              description: descriptionInput.value,
              greeting: greetingInput.value,
              prompt: promptInput.value,
              model: modelSelect.value,
              pricing: parseInt(pricingInput.value),
              public: publicCheckbox.checked,
              emoji: selectedEmoji
            })
            
            // Show success message
            document.getElementById('success-message').style.display = 'block'
            
            // Reset form
            form.reset()
            selectedEmoji = '🤖'
            updatePreview()
            updatePricing()
            
            // Reset button
            createBtn.disabled = false
            createBtn.innerHTML = originalText
            
            // Scroll to top
            window.scrollTo(0, 0)
          }, 2000)
        })

        // Initialize
        updatePreview()
        updatePricing()
      </script>
    `
  })
}
