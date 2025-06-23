import { css, document, html, renderMarkdown } from "@openagentsinc/psionic"
import fs from "fs"
import path from "path"
import { baseStyles } from "../../styles"
import { chatClientScript, chatStyles, renderChatMessage } from "../../lib/chat-utils"
import { AVAILABLE_MODELS, DEFAULT_MODEL } from "../../lib/models-config"

// Read HTML and CSS files at runtime from the source directory
const chatViewHTML = fs.readFileSync(
  path.join(process.cwd(), "src", "components", "chat-view", "chat-view.html"),
  "utf-8"
)
const chatViewCSS = fs.readFileSync(
  path.join(process.cwd(), "src", "components", "chat-view", "chat-view.css"),
  "utf-8"
)

export interface ChatViewProps {
  conversationId?: string
}

export async function createChatView({ conversationId }: ChatViewProps) {
  // Import server-side only here to avoid bundling issues
  const { getConversations, getConversationWithMessages } = await import("../../lib/chat-client")

  // Load all conversations for sidebar
  let allConversations: Array<any> = []
  try {
    allConversations = await getConversations() as Array<any>
  } catch (error) {
    console.error("Failed to load conversations:", error)
  }

  // Load messages if conversationId is provided
  let messages: Array<any> = []
  let conversation: any = null
  if (conversationId) {
    try {
      const result = await getConversationWithMessages(conversationId)
      conversation = result.conversation
      messages = result.messages as Array<any>
    } catch (error) {
      console.error("Failed to load conversation:", error)
    }
  }

  // Render messages with markdown
  const renderedMessages = await Promise.all(
    messages.map(async (msg) => ({
      ...msg,
      rendered: await renderMarkdown(msg.content)
    }))
  )

  const title = conversation?.title || "Chat - OpenAgents"

  // Generate thread list HTML
  const threadListHTML = allConversations.length > 0 ? html`
    <div class="chat-group">
      <div class="group-header">
        <span class="group-label">Recent</span>
      </div>
      <ul class="chat-list">
        ${allConversations.map((conv) => html`
          <li class="chat-item ${conv.id === conversationId ? 'active' : ''}">
            <a href="/chat/${conv.id}">
              <span class="chat-title">${conv.title}</span>
            </a>
          </li>
        `).join('')}
      </ul>
    </div>
  ` : ''

  // Generate messages HTML
  const messagesHTML = renderedMessages.length > 0 ? 
    renderedMessages.map((message) => renderChatMessage(message)).join('') : ''

  // Generate model options HTML
  const modelOptionsHTML = html`
    <div class="model-group">Cloudflare (Free)</div>
    ${AVAILABLE_MODELS
      .filter((m) => m.provider === "cloudflare")
      .map((model) => html`
        <div class="model-option" data-model-id="${model.id}" onclick="selectModel('${model.id}')">
          <div class="model-name">${model.name}</div>
          ${model.description ? html`<div class="model-description">${model.description}</div>` : ""}
        </div>
      `)
      .join("")}
    
    <div class="model-group">OpenRouter (API Key Required)</div>
    <div id="openrouter-models">
      ${AVAILABLE_MODELS
        .filter((m) => m.provider === "openrouter")
        .map((model) => html`
          <div class="model-option openrouter-model" data-model-id="${model.id}" onclick="selectModel('${model.id}')">
            <div class="model-name">
              ${model.name}
              <svg class="lock-icon" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <rect x="5" y="11" width="14" height="11" rx="2" ry="2"/>
                <path d="M7 11V7a5 5 0 0 1 10 0v4"/>
              </svg>
            </div>
            ${model.description ? html`<div class="model-description">${model.description}</div>` : ""}
          </div>
        `)
        .join("")}
    </div>
    
    <div class="api-key-notice" id="api-key-notice" style="display: none;">
      <a href="/settings">Add OpenRouter API key in Settings</a>
    </div>
  `

  // Replace placeholders in HTML
  const processedHTML = chatViewHTML
    .replace('<!-- Thread groups will be inserted here -->', threadListHTML)
    .replace('<!-- Messages will be dynamically added here -->', messagesHTML)
    .replace('<!-- Model options will be populated dynamically -->', modelOptionsHTML)

  return document({
    title,
    styles: baseStyles + css`${chatViewCSS}` + css`${chatStyles}`,
    body: html`
      ${processedHTML}
      
      <script>
        // Set conversation ID
        window.CONVERSATION_ID = ${conversationId ? `"${conversationId}"` : 'null'};
        
        // Model selector functionality
        let selectedModel = localStorage.getItem('selectedModel') || '${DEFAULT_MODEL}';
        let openrouterApiKey = localStorage.getItem('openrouterApiKey') || '';
        let hasServerKey = false;
        
        // Update selected model display on load
        const modelConfig = ${JSON.stringify(AVAILABLE_MODELS)};
        const currentModel = modelConfig.find(m => m.id === selectedModel);
        if (currentModel) {
          document.getElementById('selected-model-name').textContent = currentModel.name;
        }
        
        // Check server configuration
        async function checkConfig() {
          try {
            const response = await fetch('/api/config');
            const config = await response.json();
            hasServerKey = config.hasOpenRouterKey;
            
            // Update UI based on API key availability
            const hasAnyKey = openrouterApiKey || hasServerKey;
            document.querySelectorAll('.openrouter-model').forEach(option => {
              if (!hasAnyKey) {
                option.classList.add('locked');
              } else {
                option.classList.remove('locked');
                const lockIcon = option.querySelector('.lock-icon');
                if (lockIcon) lockIcon.style.display = 'none';
              }
            });
          } catch (error) {
            console.error('Failed to check config:', error);
          }
        }
        
        // Check config on load
        checkConfig();
        
        function toggleModelDropdown() {
          const dropdown = document.getElementById('model-selector-dropdown');
          dropdown.classList.toggle('open');
          
          // Close dropdown when clicking outside
          if (dropdown.classList.contains('open')) {
            setTimeout(() => {
              document.addEventListener('click', closeDropdownOnClickOutside);
            }, 0);
          }
        }
        
        function closeDropdownOnClickOutside(event) {
          const container = document.querySelector('.model-selector-container');
          if (!container.contains(event.target)) {
            document.getElementById('model-selector-dropdown').classList.remove('open');
            document.removeEventListener('click', closeDropdownOnClickOutside);
          }
        }
        
        function selectModel(modelId) {
          const model = modelConfig.find(m => m.id === modelId);
          if (!model) return;
          
          // Check if OpenRouter API key is needed
          if (model.provider === 'openrouter' && !openrouterApiKey && !hasServerKey) {
            document.getElementById('api-key-notice').style.display = 'block';
            return;
          }
          
          // Update selection
          selectedModel = modelId;
          localStorage.setItem('selectedModel', modelId);
          
          // Update UI
          document.getElementById('selected-model-name').textContent = model.name;
          document.getElementById('model-selector-dropdown').classList.remove('open');
          document.removeEventListener('click', closeDropdownOnClickOutside);
          
          // Update selected state
          document.querySelectorAll('.model-option').forEach(option => {
            option.classList.toggle('selected', option.dataset.modelId === modelId);
          });
        }
        
        // Mark current selection on load
        document.addEventListener('DOMContentLoaded', () => {
          document.querySelectorAll('.model-option').forEach(option => {
            option.classList.toggle('selected', option.dataset.modelId === selectedModel);
          });
        });
        
        // Make selectedModel available globally
        window.SELECTED_MODEL = selectedModel;
        window.getSelectedModel = () => {
          const model = modelConfig.find(m => m.id === selectedModel);
          return { id: selectedModel, provider: model?.provider || 'cloudflare' };
        };
        
        // Check for API key updates
        window.addEventListener('storage', (e) => {
          if (e.key === 'openrouterApiKey') {
            openrouterApiKey = e.newValue || '';
            // Hide API key notice if key was added
            if (openrouterApiKey) {
              document.getElementById('api-key-notice').style.display = 'none';
            }
            // Re-check config to update UI
            checkConfig();
          }
        });
        
        // Use the shared chat client script
        ${chatClientScript}
      </script>
    `
  })
}
