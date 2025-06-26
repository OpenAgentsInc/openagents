/**
 * Claude Code Control Route
 * Provides UI for remote control of Claude Code instances
 */
import { document, html } from "@openagentsinc/psionic"
import { claudeCodeControl } from "../components/claude-code-control"
import { navigation } from "../components/navigation"
import { baseStyles } from "../styles"

export function GET() {
  const content = claudeCodeControl()

  return document({
    title: "Claude Code Control - OpenAgents",
    styles: baseStyles,
    body: html`
      ${navigation({ current: "" })}
      ${content}
    `
  })
}
