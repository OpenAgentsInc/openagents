import { document, html } from "@openagentsinc/psionic"
import { renderChatMessage } from "../lib/chat-utils"

export async function testFindBreak() {
  const { getConversationWithMessages } = await import("../lib/chat-client-convex")
  
  const result = await getConversationWithMessages("claude-code-session-1750816776552")
  const messages = result.messages || []
  
  // Binary search to find where it breaks
  let testRanges = [
    { start: 0, end: 50, label: "0-50" },
    { start: 50, end: 100, label: "50-100" },
    { start: 100, end: 150, label: "100-150" },
    { start: 150, end: 200, label: "150-200" },
    { start: 200, end: 250, label: "200-250" },
    { start: 250, end: 300, label: "250-300" },
    { start: 300, end: 350, label: "300-350" },
  ]
  
  const results = []
  
  for (const range of testRanges) {
    try {
      const subset = messages.slice(range.start, range.end)
      const rendered = subset.map(m => renderChatMessage(m)).join('')
      results.push({
        range: range.label,
        success: true,
        messageCount: subset.length
      })
    } catch (error) {
      results.push({
        range: range.label,
        success: false,
        error: error instanceof Error ? error.message : 'Unknown error'
      })
    }
  }
  
  return document({
    title: "Find Break Point",
    body: html`
      <style>
        body {
          background: #1a1a1a;
          color: #f5f5f5;
          font-family: monospace;
          padding: 20px;
        }
        .result {
          margin: 10px 0;
          padding: 10px;
          background: #0a0a0a;
          border: 1px solid #333;
        }
        .success { border-color: #9ece6a; }
        .failure { border-color: #f7768e; }
      </style>
      
      <h1>Finding Break Point</h1>
      <p>Total messages: ${messages.length}</p>
      
      ${results.map(r => html`
        <div class="result ${r.success ? 'success' : 'failure'}">
          <strong>${r.range}:</strong> 
          ${r.success ? `✓ Success (${r.messageCount} messages)` : `✗ Failed: ${r.error}`}
        </div>
      `).join('')}
    `
  })
}