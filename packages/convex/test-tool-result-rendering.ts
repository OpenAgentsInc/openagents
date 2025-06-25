#!/usr/bin/env bun

/**
 * Test tool result rendering for message b71910df-a580-4308-8085-33557be098f4
 */

import { renderChatMessage } from "../../apps/openagents.com/src/lib/chat-utils"

// Simulate the message data
const toolResultMessage = {
  role: "user" as const,
  content: `[{"tool_use_id":"toolu_01VcDCTXpXehuZGSP5zRBvgp","type":"tool_result","content":"-2473446063908168524speed_up.mov\\n05.rbxl\\n1024.jpg\\n12-31-2023 Open Agents Proof of Cash.xlsx\\n12-31-2023 OpenAgents Balance Sheet (DRAFT - Preliminary).pdf\\n12-31-2023 OpenAgents Profit & Loss (DRAFT - Preliminary).pdf\\n1500x500a.jpeg\\n1934-7685-002.2025.issue-058-en.pdf\\n2024 BTC SENT to Contractors.xlsx","is_error":false}]`,
  timestamp: Date.now()
}

console.log("=== Testing Tool Result Rendering ===")

// Test 1: Without rendered field (should detect tool result)
console.log("\nTest 1: Without 'rendered' field")
const html1 = renderChatMessage(toolResultMessage)
console.log("HTML contains tool-result-section:", html1.includes("tool-result-section"))
console.log("HTML contains Tool Result label:", html1.includes("Tool Result"))
console.log("First 500 chars:", html1.substring(0, 500))

// Test 2: With rendered field (will use rendered content instead)
console.log("\n\nTest 2: With 'rendered' field")
const messageWithRendered = {
  ...toolResultMessage,
  rendered: "<p>This is pre-rendered markdown content</p>"
}
const html2 = renderChatMessage(messageWithRendered)
console.log("HTML contains tool-result-section:", html2.includes("tool-result-section"))
console.log("HTML contains pre-rendered content:", html2.includes("pre-rendered markdown"))
console.log("First 500 chars:", html2.substring(0, 500))

// Test 3: What the actual message looks like after markdown rendering
console.log("\n\nTest 3: After markdown rendering (simulating current behavior)")
const messageAfterMarkdown = {
  ...toolResultMessage,
  rendered: `<p>[{&quot;tool_use_id&quot;:&quot;toolu_01VcDCTXpXehuZGSP5zRBvgp&quot;,&quot;type&quot;:&quot;tool_result&quot;,&quot;content&quot;:&quot;-2473446063908168524speed_up.mov\\n05.rbxl\\n1024.jpg\\n12-31-2023 Open Agents Proof of Cash.xlsx\\n12-31-2023 OpenAgents Balance Sheet (DRAFT - Preliminary).pdf\\n12-31-2023 OpenAgents Profit &amp; Loss (DRAFT - Preliminary).pdf\\n1500x500a.jpeg\\n1934-7685-002.2025.issue-058-en.pdf\\n2024 BTC SENT to Contractors.xlsx&quot;,&quot;is_error&quot;:false}]</p>`
}
const html3 = renderChatMessage(messageAfterMarkdown)
console.log("HTML contains tool-result-section:", html3.includes("tool-result-section"))
console.log("HTML contains escaped JSON:", html3.includes("tool_use_id"))
console.log("First 500 chars:", html3.substring(0, 500))