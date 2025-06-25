import { document, html } from "@openagentsinc/psionic"

export async function debugChat280() {
  const { getConversationWithMessages } = await import("../lib/chat-client-convex")
  
  try {
    const result = await getConversationWithMessages("claude-code-session-1750816776552")
    const messages = result.messages || []
    
    // Find message around index 280
    const problemMessages = messages.slice(275, 285)
    
    return document({
      title: "Debug Chat Messages 275-285",
      body: html`
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
            border: 1px solid #333;
          }
          .index {
            color: #7aa2f7;
            font-weight: bold;
          }
          .content {
            margin-top: 10px;
            padding: 10px;
            background: #1a1a1a;
            border: 1px solid #444;
            white-space: pre-wrap;
            word-break: break-word;
          }
          .metadata {
            margin-top: 10px;
            padding: 10px;
            background: #111;
            font-size: 12px;
            color: #888;
          }
        </style>
        
        <h1>Debug Chat Messages 275-285</h1>
        <p>Total messages: ${messages.length}</p>
        
        ${problemMessages.map((msg, i) => {
          const index = i + 275
          return html`
            <div class="message">
              <div class="index">Message ${index}</div>
              <div>Role: ${msg.role}</div>
              <div>Content length: ${msg.content ? msg.content.length : 0}</div>
              <div>Metadata: ${JSON.stringify(msg.metadata)}</div>
              <div class="content">
                <strong>Content:</strong><br>
                <pre>${msg.content || "[empty]"}</pre>
              </div>
            </div>
          `
        }).join('')}
      `
    })
  } catch (error) {
    return document({
      title: "Error",
      body: html`<h1>Error: ${error instanceof Error ? error.message : 'Unknown error'}</h1>`
    })
  }
}