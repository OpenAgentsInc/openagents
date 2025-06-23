import { css, document, html } from "@openagentsinc/psionic"
import { chatClientScript, chatStyles as sharedChatStyles, renderThreadItem } from "../lib/chat-utils"
import { AVAILABLE_MODELS, DEFAULT_MODEL } from "../lib/models-config"
import { baseStyles } from "../styles"

// V1 exact styling
const v1Styles = css`
  /* V1 Color Palette from tailwind.config.js */
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
    --active-thread: #262626;
    --sidebar-border: rgba(255, 255, 255, 0.15);
  }

  /* Override any conflicting styles */
  body {
    background-color: var(--black) !important;
    color: var(--white) !important;
    font-family: "Berkeley Mono", "JetBrains Mono", ui-monospace, monospace !important;
  }

  /* V1 Sidebar styling */
  .sidebar {
    transition: border-color 0.3s ease-in-out, width 0.3s ease-in-out;
  }

  .sidebar-open {
    width: 260px;
    border-right: 1px solid var(--sidebar-border);
  }

  .sidebar-closed {
    width: 0px;
    border-right: 1px solid rgba(0, 0, 0, 0);
  }

  .hmmm {
    transition: margin-left 0.3s ease-in-out;
  }

  /* Model selector */
  .model-selector {
    background-color: var(--offblack);
    border: 1px solid var(--darkgray);
    border-radius: 6px;
    padding: 6px 12px;
    font-size: 13px;
    color: var(--lightgray);
  }

  /* Sidebar footer */
  .sidebar-footer {
    padding: 12px 0;
  }

  .sidebar-footer ol {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .sidebar-footer .footer-bottom {
    display: flex;
    flex-direction: row;
    gap: 4px;
    padding: 12px 12px 0;
    font-size: 12px;
    opacity: 0.75;
    color: var(--gray);
  }

  .sidebar-footer .footer-bottom a {
    color: inherit;
    text-decoration: none;
  }

  .sidebar-footer .footer-bottom a:hover {
    color: var(--white);
  }

  /* Messages remaining */
  .messages-remaining {
    padding: 12px 16px;
    display: flex;
    justify-content: center;
    align-items: center;
    font-size: 12px;
    color: var(--gray);
  }

  .messages-remaining span {
    font-weight: 500;
    color: var(--white);
    margin: 0 4px;
  }

  .messages-remaining a {
    margin-left: 4px;
    text-decoration: underline;
    cursor: pointer;
  }

  .messages-remaining a:hover {
    color: var(--white);
  }

  /* Send button */
  .send-button {
    position: absolute;
    right: 8px;
    bottom: 8px;
    background: none;
    border: none;
    color: var(--gray);
    cursor: pointer;
    padding: 4px;
    width: 32px;
    height: 32px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 4px;
    transition: all 0.2s;
  }

  .send-button:hover:not(:disabled) {
    color: var(--white);
    background-color: var(--offblack);
  }

  .send-button:disabled {
    opacity: 0.3;
    cursor: not-allowed;
  }

  .send-button svg {
    width: 16px;
    height: 16px;
  }

  ${sharedChatStyles}
  
  /* Model selector dropdown (same as chat) */
  .model-selector-container {
    position: relative;
  }
  
  .model-selector-button {
    background-color: var(--v1-offblack);
    border: 1px solid var(--v1-darkgray);
    border-radius: 6px;
    padding: 6px 12px;
    font-size: 13px;
    color: var(--v1-lightgray);
    cursor: pointer;
    display: flex;
    align-items: center;
    gap: 6px;
    transition: all 0.2s;
  }
  
  .model-selector-button:hover {
    background-color: var(--v1-darkgray);
    color: var(--v1-white);
  }
  
  .model-selector-dropdown {
    position: absolute;
    top: 100%;
    right: 0;
    margin-top: 4px;
    background-color: var(--v1-offblack);
    border: 1px solid var(--v1-darkgray);
    border-radius: 6px;
    min-width: 280px;
    max-height: 400px;
    overflow-y: auto;
    z-index: 100;
    display: none;
  }
  
  .model-selector-dropdown.open {
    display: block;
  }
  
  .model-group {
    padding: 8px 12px;
    font-size: 11px;
    color: var(--v1-gray);
    text-transform: uppercase;
    font-weight: 600;
    border-bottom: 1px solid var(--v1-darkgray);
  }
  
  .model-option {
    padding: 8px 12px;
    cursor: pointer;
    display: flex;
    flex-direction: column;
    gap: 2px;
    transition: background-color 0.2s;
  }
  
  .model-option:hover {
    background-color: var(--v1-darkgray);
  }
  
  .model-option.selected {
    background-color: var(--v1-active-thread);
  }
  
  .model-name {
    color: var(--v1-white);
    font-size: 13px;
  }
  
  .model-description {
    color: var(--v1-gray);
    font-size: 11px;
  }
  
  .api-key-notice {
    padding: 8px 12px;
    background-color: rgba(255, 193, 7, 0.1);
    border-top: 1px solid var(--v1-darkgray);
    font-size: 11px;
    color: var(--v1-lightgray);
  }
  
  .api-key-notice a {
    color: var(--v1-white);
    text-decoration: underline;
  }
`

export async function home() {
  // Import server-side only here to avoid bundling issues
  const { getConversations } = await import("../lib/chat-client")

  // Load all conversations for sidebar
  let allConversations: Array<any> = []
  try {
    allConversations = await getConversations() as Array<any>
  } catch (error) {
    console.error("Failed to load conversations:", error)
  }

  return document({
    title: "OpenAgents - Bitcoin-powered AI agents built with Effect",
    styles: baseStyles + v1Styles,
    body: html`
      <div style="display: flex; height: 100vh; overflow: hidden; background: black;">
        <!-- Header -->
        <div style="position: fixed; top: 0; left: 0; right: 0; height: 52px; display: flex; align-items: center; justify-content: space-between; padding: 0 20px; z-index: 20; background: black; border-bottom: 1px solid var(--darkgray);">
          <div style="display: flex; align-items: center; gap: 20px;">
            <button onclick="document.getElementById('sidebar').classList.toggle('sidebar-open'); document.getElementById('sidebar').classList.toggle('sidebar-closed'); document.getElementById('main').classList.toggle('hmmm')" style="background: none; border: none; color: white; cursor: pointer; padding: 4px;">
              <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <line x1="3" y1="12" x2="21" y2="12"></line>
                <line x1="3" y1="6" x2="21" y2="6"></line>
                <line x1="3" y1="18" x2="21" y2="18"></line>
              </svg>
            </button>
            <a href="/" style="color: white; text-decoration: none; font-size: 18px; font-weight: 600;">OpenAgents</a>
          </div>
          <div class="model-selector-container">
            <button id="model-selector-button" class="model-selector-button" onclick="toggleModelDropdown()">
              <span id="selected-model-name">Llama 3.3 70B</span>
              <svg width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M3 5L6 8L9 5"/>
              </svg>
            </button>
            <div id="model-selector-dropdown" class="model-selector-dropdown">
              <div class="model-group">Cloudflare (Free)</div>
              ${
      AVAILABLE_MODELS
        .filter((m) => m.provider === "cloudflare")
        .map((model) =>
          html`
                <div class="model-option" data-model-id="${model.id}" onclick="selectModel('${model.id}')">
                  <div class="model-name">${model.name}</div>
                  ${model.description ? html`<div class="model-description">${model.description}</div>` : ""}
                </div>
              `
        )
        .join("")
    }
              
              <div class="model-group">OpenRouter (API Key Required)</div>
              ${
      AVAILABLE_MODELS
        .filter((m) => m.provider === "openrouter")
        .map((model) =>
          html`
                <div class="model-option" data-model-id="${model.id}" onclick="selectModel('${model.id}')">
                  <div class="model-name">${model.name}</div>
                  ${model.description ? html`<div class="model-description">${model.description}</div>` : ""}
                </div>
              `
        )
        .join("")
    }
              
              <div class="api-key-notice" id="api-key-notice" style="display: none;">
                <a href="/settings">Add OpenRouter API key in Settings</a>
              </div>
            </div>
          </div>
        </div>

        <!-- Sidebar -->
        <div id="sidebar" class="sidebar sidebar-open" style="position: fixed; left: 0; top: 0; height: 100vh; background: black; overflow: hidden;">
          <div style="width: 260px; height: 100%; display: flex; flex-direction: column;">
            <!-- New thread button area - v1 style -->
            <div style="height: 54px; display: flex; align-items: center; justify-content: flex-end; padding: 0 16px;">
              <button onclick="window.location.reload()" style="background: none; border: none; color: white; cursor: pointer; padding: 6px;">
                <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <line x1="12" y1="5" x2="12" y2="19"></line>
                  <line x1="5" y1="12" x2="19" y2="12"></line>
                </svg>
              </button>
            </div>

            <!-- Thread list -->
            <div style="flex: 1; overflow-y: auto;">
              <div style="display: flex; flex-direction: column; gap: 8px; padding: 12px 4px;">
                <ol id="thread-list" style="list-style: none; margin: 0; padding: 0;">
                  ${
      allConversations.map((conv) =>
        html`
                    <li>${renderThreadItem(conv)}</li>
                  `
      ).join("")
    }
                </ol>
              </div>
            </div>

            <!-- Footer items (v1) -->
            <div class="sidebar-footer">
              <ol>
                <li>
                  <div class="relative z-[15]">
                    <div class="group relative rounded-lg active:opacity-90 px-3">
                      <a href="/store" class="hover:text-white flex flex-row items-center gap-2 py-1" style="display: flex; justify-content: space-between;">
                        <div class="select-none cursor-pointer relative overflow-hidden whitespace-nowrap">Agent Store</div>
                        <div class="text-xs text-gray opacity-50">Beta</div>
                      </a>
                    </div>
                  </div>
                </li>
                <li>
                  <div class="relative z-[15]">
                    <div class="group relative rounded-lg active:opacity-90 px-3">
                      <a href="/agents" class="flex items-center gap-2 py-1 hover:text-white">
                        <div class="select-none cursor-pointer relative grow overflow-hidden whitespace-nowrap">My Agents</div>
                      </a>
                    </div>
                  </div>
                </li>
                <li>
                  <div class="relative z-[15]">
                    <div class="group relative rounded-lg active:opacity-90 px-3">
                      <a href="/settings" class="flex items-center gap-2 py-1 hover:text-white">
                        <div class="select-none cursor-pointer relative grow overflow-hidden whitespace-nowrap">Settings</div>
                      </a>
                    </div>
                  </div>
                </li>
                <li>
                  <div class="relative z-[15]">
                    <div class="group relative rounded-lg active:opacity-90 px-3">
                      <a href="/import" class="flex items-center gap-2 py-1 hover:text-white">
                        <div class="select-none cursor-pointer relative grow overflow-hidden whitespace-nowrap">Import</div>
                      </a>
                    </div>
                  </div>
                </li>
                <li style="border-bottom: 1px solid #2B2B2D;margin-bottom: 8px;"></li>
                <li>
                  <div class="relative z-[15]">
                    <div class="group relative rounded-lg active:opacity-90 px-3">
                      <a href="/blog" class="flex items-center gap-2 py-1 hover:text-white">
                        <div class="select-none cursor-pointer relative grow overflow-hidden whitespace-nowrap">Blog</div>
                      </a>
                    </div>
                  </div>
                </li>
                <li>
                  <div class="relative z-[15]">
                    <div class="group relative rounded-lg active:opacity-90 px-3">
                      <a href="https://openagents.com/docs" class="hover:text-white flex flex-row items-center gap-2 py-1">
                        <div class="select-none cursor-pointer relative grow overflow-hidden whitespace-nowrap">Docs</div>
                      </a>
                    </div>
                  </div>
                </li>
                <li>
                  <div class="relative z-[15]">
                    <div class="group relative rounded-lg active:opacity-90 px-3">
                      <a href="https://stacker.news/~openagents" target="_blank" class="hover:text-white flex flex-row items-center gap-2 py-1" style="display: flex; justify-content: space-between;">
                        <div class="select-none cursor-pointer relative overflow-hidden whitespace-nowrap">Community</div>
                        <svg class="h-4 w-4 flex-shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                          <path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"></path>
                          <polyline points="15 3 21 3 21 9"></polyline>
                          <line x1="10" y1="14" x2="21" y2="3"></line>
                        </svg>
                      </a>
                    </div>
                  </div>
                </li>
                <li>
                  <div class="relative z-[15]">
                    <div class="group relative rounded-lg active:opacity-90 px-3">
                      <a href="https://github.com/OpenAgentsInc/openagents" target="_blank" class="hover:text-white flex flex-row items-center gap-2 py-1" style="display: flex; justify-content: space-between;">
                        <div class="select-none cursor-pointer relative overflow-hidden whitespace-nowrap">Source code</div>
                        <svg class="h-4 w-4 flex-shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                          <path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"></path>
                          <polyline points="15 3 21 3 21 9"></polyline>
                          <line x1="10" y1="14" x2="21" y2="3"></line>
                        </svg>
                      </a>
                    </div>
                  </div>
                </li>
                <li>
                  <div>
                    <div class="footer-bottom">
                      <a href="/terms">Terms</a>
                      <span>·</span>
                      <a href="/privacy">Privacy</a>
                    </div>
                  </div>
                </li>
              </ol>
            </div>
          </div>
        </div>

        <!-- Main content -->
        <div id="main" class="hmmm" style="flex: 1; display: flex; flex-direction: column; margin-left: 260px; transition: margin-left 0.3s ease-in-out;">
          <!-- Messages -->
          <div id="messages-container" style="flex: 1; overflow-y: auto; padding: 80px 20px 20px;">
            <div style="max-width: 800px; margin: 0 auto;">
              <!-- Messages will be dynamically added here -->
            </div>
          </div>

          <!-- Input area -->
          <div style="border-top: 1px solid var(--offblack); padding-top: 20px;">
            <div style="max-width: 800px; margin: 0 auto;">
              <div style="position: relative;">
                <textarea
                  id="chat-input"
                  class="chat-input"
                  placeholder="Message OpenAgents..."
                  rows="1"
                  autocomplete="off"
                  autofocus
                  style="outline: none;"
                ></textarea>
                <button id="submit-button" class="send-button submit-button" disabled>
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                    <line x1="22" y1="2" x2="11" y2="13"></line>
                    <polygon points="22 2 15 22 11 13 2 9 22 2"></polygon>
                  </svg>
                </button>
              </div>
              <div class="messages-remaining">
                You have <span>10</span> free responses remaining.
                <a>Sign up to get 10 messages every day.</a>
              </div>
            </div>
          </div>
        </div>
      </div>

      <script>
        // No conversation ID on home page
        window.CONVERSATION_ID = null;
        
        // Model selector functionality
        let selectedModel = localStorage.getItem('selectedModel') || '${DEFAULT_MODEL}';
        let openrouterApiKey = localStorage.getItem('openrouterApiKey') || '';
        
        // Update selected model display on load
        const modelConfig = ${JSON.stringify(AVAILABLE_MODELS)};
        const currentModel = modelConfig.find(m => m.id === selectedModel);
        if (currentModel) {
          document.getElementById('selected-model-name').textContent = currentModel.name;
        }
        
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
          if (model.provider === 'openrouter' && !openrouterApiKey) {
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
          }
        });
        
        // Use the shared chat client script
        ${chatClientScript}
      </script>
    `
  })
}
