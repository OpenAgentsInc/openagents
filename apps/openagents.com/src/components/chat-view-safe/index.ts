import { css } from "@openagentsinc/psionic"
import { baseStyles } from "../../styles"

export interface ChatViewSafeProps {
  conversationId?: string
}

export async function createChatViewSafe({ conversationId }: ChatViewSafeProps) {
  // Import server-side only here to avoid bundling issues
  const { getConversationWithMessages, getConversations } = await import("../../lib/chat-client-convex")

  // Load all conversations for sidebar
  try {
    await getConversations() as Array<any>
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

  const title = conversation?.title || "Chat - OpenAgents"

  // Build HTML manually without template literals
  let html = `<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>${escapeHtml(title)}</title>
  <link rel="icon" type="image/x-icon" href="/favicon.ico">
  <style>${baseStyles}${css`
    body {
      background: #1a1a1a;
      color: #f5f5f5;
      font-family: monospace;
      padding: 20px;
      margin: 0;
    }
    .container {
      max-width: 800px;
      margin: 0 auto;
    }
    .message {
      margin: 20px 0;
    }
    .message-block {
      border-left: 4px solid #333;
      padding: 15px;
      background: #0a0a0a;
    }
    .message-block.user {
      border-left-color: #9ece6a;
    }
    .message-block.assistant {
      border-left-color: #7aa2f7;
    }
    .message-block.system {
      border-left-color: #bb9af7;
      background: #0f0f0f;
      font-size: 13px;
    }
    .message-content {
      white-space: pre-wrap;
      word-wrap: break-word;
      font-family: monospace;
      font-size: 14px;
      line-height: 1.6;
    }
  `}</style>
</head>
<body>
  <div class="container">
    <h1>${escapeHtml(title)}</h1>
    <p>Messages: ${messages.length}</p>
    <div id="messages">`

  // Add each message
  for (const msg of messages) {
    const role = msg.role || "system"
    const content = msg.content || "[no content]"
    const additionalClasses = getAdditionalClasses(msg.metadata)

    html += `
      <div class="message ${role}">
        <div class="message-block ${role}${additionalClasses}">
          <div class="message-content">${escapeHtml(content)}</div>
        </div>
      </div>
    `
  }

  html += `
    </div>
  </div>
</body>
</html>`

  return new Response(html, {
    headers: {
      "content-type": "text/html; charset=utf-8"
    }
  })
}

function escapeHtml(text: string): string {
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;")
}

function getAdditionalClasses(metadata: any): string {
  if (!metadata) return ""

  let classes = ""
  if (metadata.entryType === "tool_use") classes += " tool-use"
  if (metadata.entryType === "tool_result") classes += " tool-result"
  if (metadata.entryType === "summary") classes += " summary"

  return classes
}
