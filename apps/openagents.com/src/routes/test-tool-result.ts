import { document, html } from "@openagentsinc/psionic"
import { renderChatMessage } from "../lib/chat-utils"

export function testToolResult() {
  // Test message similar to what we're seeing
  const testMessage = {
    id: "test-message",
    role: "user" as const,
    content: `📤 Tool Result: -2473446063908168524speed_up.mov
05.rbxl
1024.jpg
12-31-2023 Open Agents Proof of Capability.pdf
12-31-2023 OpenAgents Balance Sheet (DRAFT - Preliminary).pdf
2024 Churn Analysis.xlsx
2025 Calendars.ai
2025+Calendars.pdf
2025-06-05_07-35-50.mkv
2025-06-13_11-51-28.mkv
2025-06-13_11-51-38.mkv
2025-06-13_12-03-48.mkv
231108_Ledger Balance.xlsx
32Bitty_Kin_Pic.png
5starlogo.png
AI Agent.png
ARQAI-MCP-2025.pptx
ASquaredAgency.png
AboutMe-OG.png
AgentCamp - Google Chrome 2025-05-28 08-33-45.mp4`,
    timestamp: Date.now(),
    metadata: {
      entryType: "user",
      hasEmbeddedTool: false
    }
  }

  const renderedMessage = renderChatMessage(testMessage)

  return document({
    title: "Test Tool Result",
    body: html`
      <style>
        body { 
          background: #1a1a1a; 
          color: #f5f5f5; 
          font-family: monospace; 
          padding: 2rem;
        }
        .message { margin: 1rem 0; }
        .message-block { padding: 1rem; background: #262626; border-radius: 8px; }
        .tool-result-section {
          background: rgba(34, 197, 94, 0.05);
          border: 1px solid rgba(34, 197, 94, 0.2);
          border-radius: 6px;
          padding: 0.75rem;
        }
        .tool-result-header {
          display: flex;
          align-items: center;
          gap: 0.5rem;
          margin-bottom: 0.5rem;
          color: #9ece6a;
        }
        .file-listing {
          background: rgba(158, 206, 106, 0.05);
          border: 1px solid rgba(158, 206, 106, 0.2);
          border-radius: 6px;
          padding: 0.75rem;
        }
        details {
          margin-top: 0.5rem;
        }
        summary {
          cursor: pointer;
          color: #9ece6a;
        }
        pre {
          background: #0a0a0a;
          padding: 0.5rem;
          border-radius: 4px;
          overflow-x: auto;
        }
      </style>
      
      <h1>Tool Result Rendering Test</h1>
      
      <h2>Raw Content:</h2>
      <pre>${testMessage.content}</pre>
      
      <h2>Rendered Output:</h2>
      ${renderedMessage}
      
      <h2>Detection Info:</h2>
      <pre>
Content starts with "📤 Tool Result:": ${testMessage.content.startsWith("📤 Tool Result:")}
Content length: ${testMessage.content.length}
Lines count: ${testMessage.content.split("\n").length}
      </pre>
    `
  })
}
