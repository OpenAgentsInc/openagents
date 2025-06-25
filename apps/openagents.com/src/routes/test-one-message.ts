import { document, html } from "@openagentsinc/psionic"

export async function testOneMessage() {
  // Get REAL messages from Convex
  const { getConversationWithMessages } = await import("../lib/chat-client-convex")
  
  let messages = []
  try {
    // Get a random conversation
    const result = await getConversationWithMessages("claude-code-session-1750816776552")
    const allMessages = result.messages || []
    
    // Pick 3 random messages
    const shuffled = [...allMessages].sort(() => Math.random() - 0.5)
    messages = shuffled.slice(0, 3)
  } catch (error) {
    console.error("Failed to get messages:", error)
    messages = [
      { role: "user", content: "Failed to load messages from database" },
      { role: "assistant", content: `Error: ${error.message}` },
      { role: "system", content: "Check the database connection" }
    ]
  }
  
  // Render JUST THIS ONE MESSAGE - NO HTML WRAPPER!
  const messageHtml = html`
    <style>
      body {
        background: #1a1a1a;
        color: #f5f5f5;
        font-family: monospace;
        padding: 20px;
      }
      .message {
        border: 3px solid #f7768e;
        padding: 20px;
        margin: 20px 0;
        background: #0a0a0a;
      }
      pre {
        background: #2a2a2a;
        padding: 10px;
        overflow: auto;
        white-space: pre-wrap;
      }
    </style>
    
    <h1>THREE MESSAGES IN A ROW</h1>
    
    ${messages.map(msg => {
      const role = msg.role || msg.entry_type || "unknown"
      const content = msg.content || "[No content]"
      
      if (role === "user") {
        return html`
          <div class="message" style="border-color: #9ece6a;">
            <h2>👤 User</h2>
            <pre style="white-space: pre-wrap;">${content}</pre>
          </div>
        `
      } else if (role === "assistant") {
        return html`
          <div class="message" style="border-color: #7aa2f7;">
            <h2>🤖 Assistant</h2>
            <pre style="white-space: pre-wrap;">${content}</pre>
          </div>
        `
      } else {
        return html`
          <div class="message" style="border-color: #bb9af7;">
            <h2>📋 ${role}</h2>
            <pre style="white-space: pre-wrap;">${content}</pre>
            ${msg.metadata ? html`<pre>${JSON.stringify(msg.metadata, null, 2)}</pre>` : ''}
          </div>
        `
      }
    }).join('')}
  `
  
  return document({
    title: "Test One Message",
    body: messageHtml
  })
}