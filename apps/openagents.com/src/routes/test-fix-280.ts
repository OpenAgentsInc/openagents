import { document, html } from "@openagentsinc/psionic"

export async function testFix280() {
  const { getConversationWithMessages } = await import("../lib/chat-client-convex")
  
  const result = await getConversationWithMessages("claude-code-session-1750816776552")
  const allMessages = result.messages || []
  
  // Get messages 278-282
  const messages = allMessages.slice(278, 283)
  
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
      .message-content {
        background: #1a1a1a;
        padding: 10px;
        overflow-x: auto;
        font-size: 14px;
        line-height: 1.5;
      }
      /* CRITICAL: Ensure content doesn't break out */
      .safe-content {
        white-space: pre-wrap;
        word-wrap: break-word;
        overflow-wrap: break-word;
        max-width: 100%;
        display: block;
      }
      .truncated {
        color: #ff9999;
        font-style: italic;
      }
    </style>
    
    <h1>Fixed Messages 279-283</h1>
    
    ${messages.map((msg, index) => {
      const msgNum = index + 279
      const role = msg.role || "system"
      let content = msg.content || "[No content]"
      
      // SAFETY: Truncate extremely long content
      let isTruncated = false
      if (content.length > 2000) {
        content = content.substring(0, 2000)
        isTruncated = true
      }
      
      // Create a pre element to contain the content safely
      return html`
        <div class="message ${role}">
          <div class="message-header">
            <span class="message-role">${role.toUpperCase()}</span>
            <span>Message ${msgNum} (${msg.content ? msg.content.length : 0} chars)</span>
          </div>
          <div class="message-content">
            <pre class="safe-content">${content}</pre>
            ${isTruncated ? html`<div class="truncated">[TRUNCATED from ${msg.content.length} chars]</div>` : ''}
          </div>
        </div>
      `
    }).join('')}
  `
  
  return document({
    title: "Fixed Message Display",
    body: messageHtml
  })
}