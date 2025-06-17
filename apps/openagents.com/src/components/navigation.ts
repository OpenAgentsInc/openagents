import { html } from "@openagentsinc/psionic"
import { themeSwitcher } from "./theme-switcher"

export function navigation({ current }: { current: string }) {
  const links = [
    { href: "/", label: "Home", key: "home" },
    { href: "/agents", label: "Agents", key: "agents" },
    { href: "/docs", label: "Docs", key: "docs" },
    { href: "/blog", label: "Blog", key: "blog" },
    { href: "/about", label: "About", key: "about" }
  ]

  return html`
    <div class="nav-container">
      <nav class="nav-links">
        ${
    links.map((link) => {
      const isActive = current === link.key
      return `<a href="${link.href}" class="webtui-button ${
        isActive ? "webtui-variant-foreground1" : "webtui-variant-background1"
      }">${link.label}</a>`
    }).join("")
  }
      </nav>
      ${themeSwitcher()}
    </div>
  `
}
