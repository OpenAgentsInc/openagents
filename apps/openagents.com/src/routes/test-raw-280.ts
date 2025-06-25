import { document, html } from "@openagentsinc/psionic"

export async function testRaw280() {
  const { getConversationWithMessages } = await import("../lib/chat-client-convex")
  
  const result = await getConversationWithMessages("claude-code-session-1750816776552")
  const allMessages = result.messages || []
  
  // Get message 280 specifically (index 279)
  const msg280 = allMessages[279]
  
  if (!msg280) {
    return document({
      title: "Error",
      body: html`<h1>Message 280 not found</h1>`
    })
  }
  
  // Create a safe display of the content
  const content = msg280.content || "[No content]"
  const truncatedContent = content.length > 1000 ? content.substring(0, 1000) + "...[TRUNCATED]" : content
  
  const messageHtml = html`
    <style>
      body {
        background: #1a1a1a;
        color: #f5f5f5;
        font-family: monospace;
        padding: 20px;
      }
      pre {
        background: #0a0a0a;
        padding: 15px;
        overflow-x: auto;
        white-space: pre-wrap;
        word-wrap: break-word;
      }
      .warning {
        background: #550000;
        border: 2px solid #ff0000;
        padding: 10px;
        color: #ff9999;
      }
    </style>
    
    <h1>Raw Message 280</h1>
    
    <h2>Basic Info:</h2>
    <pre>
Role: ${msg280.role}
Content Length: ${content.length}
Content Type: ${typeof content}
    </pre>
    
    <h2>Content (truncated to 1000 chars):</h2>
    <pre>${truncatedContent}</pre>
    
    <h2>Full Message Object (JSON):</h2>
    <pre>${JSON.stringify(msg280, null, 2)}</pre>
    
    ${content.length > 5000 ? html`
      <div class="warning">
        ⚠️ This message has ${content.length} characters which might be breaking the layout!
      </div>
    ` : ''}
  `
  
  return document({
    title: "Raw Message 280",
    body: messageHtml
  })
}