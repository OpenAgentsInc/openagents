import { css, document, html } from "@openagentsinc/psionic"
import { chatStyles, renderChatMessage } from "../lib/chat-utils"
import { baseStyles } from "../styles"

export function testToolResultDisplay() {
  // Create a test message that matches the problematic one
  const toolResultMessage = {
    role: "user" as const,
    content: `[{"tool_use_id":"toolu_01VcDCTXpXehuZGSP5zRBvgp","type":"tool_result","content":"-2473446063908168524speed_up.mov\\n05.rbxl\\n1024.jpg\\n12-31-2023 Open Agents Proof of Cash.xlsx\\n12-31-2023 OpenAgents Balance Sheet (DRAFT - Preliminary).pdf\\n12-31-2023 OpenAgents Profit & Loss (DRAFT - Preliminary).pdf\\n1500x500a.jpeg\\n1934-7685-002.2025.issue-058-en.pdf\\n2024 BTC SENT to Contractors.xlsx\\n2024-03-13_Order-od_KrOvXIf5rv.pdf\\n2024-03-13_Order-od_RqrMCZp5ju.pdf\\n2024-04-29_OpenAgents-Inc._Order-od_HC1YbJMsOF.pdf\\n2024-monthly-statements\\n2024-monthly-statements.zip\\n20240903 Shell\\n20240903 Shell 2\\n20241209_1405_Submersible Approaches Thermal Plant_simple_compose_01jepgc6ycefqab0chf2fjdvkh.mp4\\n20241209_2125_Atlantean Warrior Transformation_simple_compose_01jeq9khjkf4wrrjf6jg22yr2j.mp4\\n2025-03-03_Bitcoin-Park_Order-od_vPUG59bvWL.pdf","is_error":false}]`,
    timestamp: Date.now()
  }

  // Also test a regular message
  const regularMessage = {
    role: "assistant" as const,
    content: "This is a regular assistant message for comparison.",
    timestamp: Date.now()
  }

  // Render both messages
  const toolResultHTML = renderChatMessage(toolResultMessage)
  const regularHTML = renderChatMessage(regularMessage)

  return document({
    title: "Tool Result Display Test",
    styles: baseStyles + css`${chatStyles}` + css`
      body {
        background: var(--black);
        color: var(--white);
        padding: 2rem;
        font-family: var(--font-family-mono);
      }
      
      .test-section {
        margin-bottom: 3rem;
      }
      
      .test-title {
        font-size: 1.5rem;
        font-weight: bold;
        margin-bottom: 1rem;
        color: #9ece6a;
      }
      
      .test-description {
        margin-bottom: 1rem;
        color: var(--gray);
      }
      
      .messages-container {
        max-width: 800px;
        margin: 0 auto;
      }
      
      .debug-info {
        background: var(--offblack);
        padding: 1rem;
        border-radius: 4px;
        margin-top: 1rem;
        font-size: 12px;
        white-space: pre-wrap;
        font-family: var(--font-family-mono);
      }
    `,
    body: html`
      <div class="test-section">
        <h1 class="test-title">Tool Result Display Test</h1>
        <p class="test-description">Testing how tool results are displayed with message ID: b71910df-a580-4308-8085-33557be098f4</p>
      </div>

      <div class="messages-container">
        <div class="test-section">
          <h2 class="test-title">Tool Result Message (User)</h2>
          <p class="test-description">This should display as a formatted tool result with collapsible content</p>
          ${toolResultHTML}
          
          <div class="debug-info">
            <strong>Debug Info:</strong>
            Content starts with '[{': ${toolResultMessage.content.startsWith('[{')}
            Content length: ${toolResultMessage.content.length} characters
            HTML contains 'tool-result-section': ${toolResultHTML.includes('tool-result-section')}
            HTML contains 'Tool Result' label: ${toolResultHTML.includes('Tool Result')}
          </div>
        </div>

        <div class="test-section">
          <h2 class="test-title">Regular Message (Assistant)</h2>
          <p class="test-description">This should display as a normal assistant message</p>
          ${regularHTML}
        </div>
      </div>

      <div class="test-section">
        <h2 class="test-title">CSS Classes Check</h2>
        <div class="debug-info">
          The following CSS classes should be defined and working:
          - .tool-result-section (border-left: 3px solid #9ece6a)
          - .tool-result-header (display: flex)
          - .tool-result-icon (color: #9ece6a)
          - .tool-result-label (color: #9ece6a, font-weight: 600)
          - .tool-result-content (font-size: 14px)
          - .tool-result-details (collapsible details element)
        </div>
      </div>
    `
  })
}