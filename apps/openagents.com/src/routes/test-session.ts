import { document, html } from "@openagentsinc/psionic"

export async function testSession() {
  // Get messages from the specific session
  const { getConversationWithMessages } = await import("../lib/chat-client-convex")
  
  let messages = []
  try {
    const result = await getConversationWithMessages("claude-code-session-1750816776552")
    messages = result.messages || []
  } catch (error) {
    console.error("Failed to get messages:", error)
    return document({
      title: "Error",
      body: html`<h1>Error: ${error.message}</h1>`
    })
  }
  
  // Take messages 279-282 to isolate the problem
  const next100 = messages.slice(279, 282)
  
  const messageHtml = html`
    <style>
      body {
        background: #1a1a1a;
        color: #f5f5f5;
        font-family: monospace;
        padding: 20px;
        max-width: 1200px;
        margin: 0 auto;
      }
      h1 {
        color: #7aa2f7;
        border-bottom: 2px solid #333;
        padding-bottom: 10px;
      }
      .message {
        margin: 20px 0;
        padding: 15px;
        background: #0a0a0a;
        border-left: 4px solid #333;
      }
      .message.user {
        border-left-color: #9ece6a;
      }
      .message.assistant {
        border-left-color: #7aa2f7;
      }
      .message.system {
        border-left-color: #bb9af7;
      }
      .message-header {
        display: flex;
        justify-content: space-between;
        margin-bottom: 10px;
        font-size: 14px;
        color: #888;
      }
      .message-role {
        font-weight: bold;
        text-transform: uppercase;
      }
      .message-content {
        white-space: pre-wrap;
        word-wrap: break-word;
        font-size: 14px;
        line-height: 1.5;
      }
      .metadata {
        margin-top: 10px;
        padding: 10px;
        background: #1a1a1a;
        font-size: 12px;
        color: #666;
      }
    </style>
    
    <h1>Session: claude-code-session-1750816776552</h1>
    <p>Showing messages 280-282 (${next100.length} messages) of ${messages.length} total</p>
    
    ${next100.map((msg, index) => {
      const role = msg.role || "system"
      const content = msg.content || "[No content]"
      
      // Check if content is too long (might break layout)
      const isTooLong = content.length > 10000
      const displayContent = isTooLong ? content.substring(0, 5000) + "\n\n[TRUNCATED - Content too long]" : content
      
      return html`
        <div class="message ${role}">
          <div class="message-header">
            <span class="message-role">${role}</span>
            <span>Message ${index + 280}</span>
            ${isTooLong ? html`<span style="color: red;"> ⚠️ TRUNCATED</span>` : ''}
          </div>
          <div class="message-content"><pre style="white-space: pre-wrap; margin: 0;">${displayContent}</pre></div>
          ${msg.metadata ? html`
            <div class="metadata">
              <strong>Metadata:</strong><br>
              <pre style="margin: 0;">${JSON.stringify(msg.metadata, null, 2)}</pre>
            </div>
          ` : ''}
        </div>
      `
    }).join('')}
  `
  
  return document({
    title: "Test Session View",
    body: messageHtml
  })
}