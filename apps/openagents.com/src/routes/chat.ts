import { css, document, html, renderMarkdown } from "@openagentsinc/psionic"
import {
  chatClientScript,
  chatStyles as sharedChatStyles,
  renderChatMessage,
  renderThreadItem
} from "../lib/chat-utils"
import { baseStyles } from "../styles"

// Additional chat-specific styles
const chatStyles = css`
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
    --active-thread: #262626;
    --sidebar-border: rgba(255, 255, 255, 0.15);
  }

  body {
    background-color: var(--black) !important;
    color: var(--white) !important;
    font-family: "Berkeley Mono", "JetBrains Mono", ui-monospace, monospace !important;
  }

  /* Sidebar styling */
  .sidebar { transition: border-color 0.3s ease-in-out, width 0.3s ease-in-out; }
  .sidebar-open { width: 260px; border-right: 1px solid var(--sidebar-border); }
  .sidebar-closed { width: 0px; border-right: 1px solid rgba(0, 0, 0, 0); }
  .hmmm { transition: margin-left 0.3s ease-in-out; }

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
    border-top: 1px solid #2B2B2D;
    color: var(--text);
  }

  .sidebar-footer ol {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  ${sharedChatStyles}
`

export async function chat(ctx: { params: { id: string } }) {
  const conversationId = ctx.params.id

  // Import server-side only here to avoid bundling issues
  const { getConversationWithMessages, getConversations } = await import("../lib/chat-client")

  // Try to load the conversation and messages
  let conversation: any = null
  let messages: Array<any> = []
  let conversationExists = true

  try {
    const result = await getConversationWithMessages(conversationId)
    conversation = result.conversation
    messages = result.messages as Array<any>
  } catch {
    // Conversation not found
    conversationExists = false
  }

  // Load all conversations for sidebar
  let allConversations: Array<any> = []
  try {
    allConversations = await getConversations() as Array<any>
  } catch (error) {
    console.error("Failed to load conversations:", error)
  }

  // If conversation doesn't exist, show 404
  if (!conversationExists) {
    return document({
      title: "Chat Not Found - OpenAgents",
      styles: baseStyles + chatStyles,
      body: html`
        <div style="display: flex; height: 100vh; align-items: center; justify-content: center; background: black; color: white;">
          <div style="text-align: center;">
            <h1>Chat Not Found</h1>
            <p>The conversation "${conversationId}" could not be found.</p>
            <a href="/" style="color: white; text-decoration: underline;">← Back to Home</a>
          </div>
        </div>
      `
    })
  }

  // Render messages with markdown
  const renderedMessages = await Promise.all(
    messages.map(async (msg) => ({
      ...msg,
      rendered: await renderMarkdown(msg.content)
    }))
  )

  return document({
    title: `${conversation?.title || "Chat"} - OpenAgents`,
    styles: baseStyles + chatStyles,
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
          <div class="model-selector">llama-3.3-70b</div>
        </div>

        <!-- Sidebar -->
        <div id="sidebar" class="sidebar sidebar-open" style="position: fixed; left: 0; top: 0; height: 100vh; background: black; overflow: hidden;">
          <div style="width: 260px; height: 100%; display: flex; flex-direction: column;">
            <!-- New thread button area -->
            <div style="height: 54px; display: flex; align-items: center; justify-content: flex-end; padding: 0 16px;">
              <a href="/" style="background: none; border: none; color: white; cursor: pointer; padding: 6px; text-decoration: none;">
                <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <line x1="12" y1="5" x2="12" y2="19"></line>
                  <line x1="5" y1="12" x2="19" y2="12"></line>
                </svg>
              </a>
            </div>

            <!-- Thread list -->
            <div style="flex: 1; overflow-y: auto;">
              <div style="display: flex; flex-direction: column; gap: 8px; padding: 12px 4px;">
                <ol id="thread-list" style="list-style: none; margin: 0; padding: 0;">
                  ${
      allConversations.map((conv) =>
        html`
                    <li>${
          renderThreadItem({
            ...conv,
            active: conv.id === conversationId
          })
        }</li>
                  `
      ).join("")
    }
                </ol>
              </div>
            </div>

            <!-- Sidebar footer -->
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
              </ol>
            </div>
          </div>
        </div>

        <!-- Main content -->
        <div id="main" class="hmmm" style="flex: 1; display: flex; flex-direction: column; margin-left: 260px; transition: margin-left 0.3s ease-in-out;">
          <!-- Messages -->
          <div id="messages-container" style="flex: 1; overflow-y: auto; padding: 80px 20px 20px;">
            <div style="max-width: 800px; margin: 0 auto;">
              ${renderedMessages.map((message) => renderChatMessage(message)).join("")}
            </div>
          </div>

          <!-- Input area -->
          <div style="border-top: 1px solid var(--offblack); padding-top: 20px;">
            <div style="max-width: 800px; margin: 0 auto;">
              <div style="position: relative;">
                <textarea
                  id="chat-input"
                  class="chat-input"
                  placeholder="Continue the conversation..."
                  rows="1"
                  autocomplete="off"
                ></textarea>
                <button id="submit-button" class="submit-button">
                  <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                    <line x1="12" y1="19" x2="12" y2="5"></line>
                    <polyline points="5 12 12 5 19 12"></polyline>
                  </svg>
                </button>
              </div>
              <div style="padding: 12px 16px; display: flex; justify-content: space-between; align-items: center; font-size: 12px; color: var(--gray);">
                <span>Shift + Enter for new line</span>
                <span>⌘ Enter to send</span>
              </div>
            </div>
          </div>
        </div>
      </div>

      <script>
        // Set conversation ID for the client script
        window.CONVERSATION_ID = "${conversationId}";
        
        ${chatClientScript}
      </script>
    `
  })
}
