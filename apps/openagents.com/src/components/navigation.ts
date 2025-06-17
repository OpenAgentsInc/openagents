import { html } from "@openagentsinc/psionic"

export function navigation(currentPath: string) {
  return html`
    <nav>
      <a href="/" class="${currentPath === "/" ? "active" : ""}">Home</a>
      <a href="/agents" class="${currentPath === "/agents" ? "active" : ""}">Agents</a>
      <a href="/docs" class="${currentPath === "/docs" ? "active" : ""}">Docs</a>
      <a href="/about" class="${currentPath === "/about" ? "active" : ""}">About</a>
    </nav>
  `
}
