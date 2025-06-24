/**
 * Chat functionality module
 * Handles chat UI interactions, message streaming, and conversation management
 */

import { ProseMirrorEditor } from "./prosemirror-setup"

interface ChatClient {
  createConversation: (title: string) => Promise<string>
  addMessage: (conversationId: string, role: string, content: string) => Promise<void>
  getConversations: () => Promise<Array<any>>
  updateConversationTitle: (conversationId: string, title: string) => Promise<void>
}

// Global state
let isGenerating = false
let currentStreamReader: ReadableStreamDefaultReader<Uint8Array> | null = null
let currentConversationId: string | null = null
let proseMirrorEditor: ProseMirrorEditor | null = null

// Create chat client
const chatClient: ChatClient = {
  createConversation: async (title: string) => {
    const response = await fetch("/api/conversations", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ title })
    })
    const data = await response.json()
    return data.id
  },

  addMessage: async (conversationId: string, role: string, content: string) => {
    await fetch(`/api/conversations/${conversationId}/messages`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ role, content })
    })
  },

  getConversations: async () => {
    const response = await fetch("/api/conversations")
    return response.json()
  },

  updateConversationTitle: async (conversationId: string, title: string) => {
    await fetch(`/api/conversations/${conversationId}`, {
      method: "PATCH",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ title })
    })
  }
}

// Utility functions
function escapeHtml(text: string): string {
  const div = document.createElement("div")
  div.textContent = text
  // Replace newlines with <br> tags after escaping
  return div.innerHTML.replace(/\n/g, "<br>")
}

export async function loadConversations() {
  try {
    const conversations = await chatClient.getConversations()
    const threadContainer = document.getElementById("thread-list-container")

    if (!threadContainer || conversations.length === 0) return

    const recentHtml = `
      <div class="mt-2">
        <div class="px-3 py-1 mb-0.5">
          <span class="text-xs font-medium text-[rgba(255,255,255,0.5)] uppercase">Recent</span>
        </div>
        <ul class="flex flex-col gap-0.5">
          ${
      conversations.map((conv) => `
            <li>
              <a href="/chat/${conv.id}" class="block px-3 py-1.5 text-sm rounded-md transition-colors ${
        conv.id === currentConversationId
          ? "bg-[rgba(255,255,255,0.1)] text-[#D7D8E5]"
          : "text-[rgba(255,255,255,0.7)] hover:bg-[rgba(255,255,255,0.05)] hover:text-[#D7D8E5]"
      }">
                <span>${escapeHtml(conv.title)}</span>
              </a>
            </li>
          `).join("")
    }
        </ul>
      </div>
    `

    threadContainer.innerHTML = recentHtml
  } catch (error) {
    console.error("Failed to load conversations:", error)
  }
}

export async function sendMessage(message: string) {
  console.log("=== sendMessage Debug ===")
  console.log("Message received (with escapes):", JSON.stringify(message))
  console.log("Message received (raw):", message)
  console.log("Message length:", message.length)

  if (isGenerating || !message.trim()) return

  const submitButton = document.getElementById("submit-button") as HTMLButtonElement

  if (!proseMirrorEditor || !submitButton) return

  // Update UI state
  isGenerating = true
  submitButton.disabled = true
  proseMirrorEditor.setDisabled(true)

  // Create conversation if needed
  if (!currentConversationId) {
    try {
      const title = message.slice(0, 50) + (message.length > 50 ? "..." : "")
      currentConversationId = await chatClient.createConversation(title)

      // Update URL without reload
      window.history.replaceState({}, "", `/chat/${currentConversationId}`)

      // Reload conversations to show new one
      await loadConversations()
    } catch (error) {
      console.error("Failed to create conversation:", error)
      resetInputState()
      return
    }
  }

  // Add user message to UI
  addMessageToUI("user", message)

  // Save user message
  try {
    await chatClient.addMessage(currentConversationId, "user", message)
  } catch (error) {
    console.error("Failed to save message:", error)
  }

  // Stream assistant response
  await streamAssistantResponse(message)
}

function addMessageToUI(role: "user" | "assistant", content: string) {
  const messagesContainer = document.getElementById("messages-container")
  const messagesDiv = messagesContainer?.querySelector("div")

  if (!messagesDiv) return

  const messageHtml = `
    <div class="message">
      <div class="message-block ${role}">
        <div class="message-body">${role === "user" ? escapeHtml(content) : content}</div>
      </div>
    </div>
  `

  messagesDiv.insertAdjacentHTML("beforeend", messageHtml)
  if (messagesContainer) {
    messagesContainer.scrollTop = messagesContainer.scrollHeight
  }
}

async function streamAssistantResponse(message: string) {
  // Get selected model
  const modelData = (window as any).getSelectedModel?.() || { id: "llama-4-scout-17b", provider: "cloudflare" }

  // Prepare request body
  const requestBody: any = {
    message,
    conversationId: currentConversationId,
    model: modelData.id
  }

  // Add API key if needed
  if (modelData.provider === "openrouter") {
    const apiKey = localStorage.getItem("openrouterApiKey")
    if (apiKey) {
      requestBody.openrouterApiKey = apiKey
    }
  }

  try {
    const endpoint = modelData.provider === "cloudflare" ? "/api/cloudflare/chat" : "/api/openrouter/chat"
    const response = await fetch(endpoint, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(requestBody)
    })

    if (!response.ok) {
      throw new Error(`HTTP error! status: ${response.status}`)
    }

    // Create assistant message container
    addMessageToUI("assistant", "<div class=\"dot-flashing\"></div>")

    const messagesContainer = document.getElementById("messages-container")
    const allMessages = messagesContainer?.querySelectorAll(".message")
    const assistantMessage = allMessages?.[allMessages.length - 1]
    const messageBody = assistantMessage?.querySelector(".message-body")

    if (!response.body || !messageBody) {
      throw new Error("No response body or message container")
    }

    // Stream the response
    const reader = response.body.getReader()
    currentStreamReader = reader
    const decoder = new TextDecoder()
    let buffer = ""
    let fullContent = ""

    while (true) {
      const { done, value } = await reader.read()

      if (done) break

      buffer += decoder.decode(value, { stream: true })
      const lines = buffer.split("\n")
      buffer = lines.pop() || ""

      for (const line of lines) {
        if (line.startsWith("data: ")) {
          const data = line.slice(6)
          if (data === "[DONE]") continue

          try {
            const parsed = JSON.parse(data)
            // Handle OpenAI-compatible SSE format
            const content = parsed.choices?.[0]?.delta?.content
            if (content) {
              fullContent += content
              // Convert markdown to HTML for display
              const htmlContent = await convertMarkdown(fullContent)
              messageBody.innerHTML = htmlContent
              messagesContainer!.scrollTop = messagesContainer!.scrollHeight
            }
          } catch (e) {
            console.error("Failed to parse SSE data:", e)
          }
        }
      }
    }

    // Save assistant message
    if (fullContent && currentConversationId) {
      try {
        await chatClient.addMessage(currentConversationId, "assistant", fullContent)
      } catch (error) {
        console.error("Failed to save assistant message:", error)
      }
    }
  } catch (error) {
    console.error("Chat error:", error)
    const messagesContainer = document.getElementById("messages-container")
    const allMessages = messagesContainer?.querySelectorAll(".message")
    const lastMessage = allMessages?.[allMessages.length - 1]
    const messageBody = lastMessage?.querySelector(".message-body")
    if (messageBody) {
      messageBody.innerHTML = "<div class=\"error\">Failed to get response. Please try again.</div>"
    }
  } finally {
    currentStreamReader = null
    resetInputState()
  }
}

async function convertMarkdown(markdown: string): Promise<string> {
  // For now, just escape HTML and convert basic markdown
  // In production, you might want to use a proper markdown parser
  let html = escapeHtml(markdown)

  // Convert code blocks
  html = html.replace(/```(\w+)?\n([\s\S]*?)```/g, (_, lang, code) => {
    return `<pre><code class="language-${lang || "plaintext"}">${code.trim()}</code></pre>`
  })

  // Convert inline code
  html = html.replace(/`([^`]+)`/g, "<code>$1</code>")

  // Convert bold
  html = html.replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>")

  // Convert italic
  html = html.replace(/\*([^*]+)\*/g, "<em>$1</em>")

  // Convert line breaks
  html = html.replace(/\n/g, "<br>")

  return html
}

function resetInputState() {
  const submitButton = document.getElementById("submit-button") as HTMLButtonElement

  if (proseMirrorEditor && submitButton) {
    isGenerating = false
    submitButton.disabled = false
    proseMirrorEditor.setDisabled(false)
    proseMirrorEditor.focus()
  }
}

// Initialize chat when module loads
export function initializeChat() {
  // Set conversation ID from window if available
  currentConversationId = (window as any).CONVERSATION_ID || null

  // Load conversations
  loadConversations()

  // Set up form submission
  const form = document.querySelector(".chat-form") as HTMLFormElement
  if (form) {
    form.addEventListener("submit", async (e) => {
      e.preventDefault()
      if (proseMirrorEditor) {
        const text = proseMirrorEditor.getText()
        if (text) {
          await sendMessage(text)
        }
      }
    })
  }

  // Set up ProseMirror editor
  const inputContainer = document.getElementById("chat-input")
  if (inputContainer) {
    proseMirrorEditor = new ProseMirrorEditor({
      mount: inputContainer,
      onSubmit: async (text) => {
        await sendMessage(text)
      },
      placeholder: "Message OpenAgents..."
    })

    // Set up submit button state monitoring
    const submitButton = document.getElementById("submit-button") as HTMLButtonElement
    if (submitButton) {
      // Monitor for changes to update submit button state
      const view = proseMirrorEditor.getView()
      const originalDispatchTransaction = view.dispatch.bind(view)
      view.dispatch = (tr) => {
        originalDispatchTransaction(tr)
        const hasText = view.state.doc.textContent.trim().length > 0
        submitButton.disabled = !hasText || isGenerating
      }
    }

    // Focus the editor
    proseMirrorEditor.focus()
  }

  // Stop generation on button click
  const stopButton = document.getElementById("stop-button")
  if (stopButton) {
    stopButton.addEventListener("click", () => {
      if (currentStreamReader) {
        currentStreamReader.cancel()
        currentStreamReader = null
        resetInputState()
      }
    })
  }
} // Export necessary functions for global access

;(window as any).sendMessage = sendMessage

// Functions are already exported above
