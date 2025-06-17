import { html } from "@openagentsinc/psionic"

export function navigation({ current }: { current: string }) {
  return html`
    <nav>
      <a href="/" class="${current === "home" ? "active" : ""}">Home</a>
      <a href="/agents" class="${current === "agents" ? "active" : ""}">Agents</a>
      <a href="/docs" class="${current === "docs" ? "active" : ""}">Docs</a>
      <a href="/blog" class="${current === "blog" ? "active" : ""}">Blog</a>
      <a href="/about" class="${current === "about" ? "active" : ""}">About</a>
    </nav>
  `
}
