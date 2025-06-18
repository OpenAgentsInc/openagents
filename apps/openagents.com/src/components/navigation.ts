import { html } from "@openagentsinc/psionic"

export function navigation({ current }: { current: string }) {
  const rightLinks = [
    { href: "/docs", label: "📄 Docs", key: "docs" },
    { href: "/components", label: "🧩 Examples", key: "components" },
    { href: "https://github.com/openagentsinc/openagents", label: "🔗 Github", key: "github", external: true }
  ]

  return html`
    <header class="webtui-header">
      <div class="header-brand">
        <a href="/" class="brand-link">
          <span class="brand-brackets">&lt;/&gt;</span>
          <span class="brand-text">OpenAgents</span>
        </a>
      </div>
      
      <nav class="header-nav">
        ${
    rightLinks.map((link) => {
      const isActive = current === link.key
      return `<a href="${link.href}" 
                     class="nav-link ${isActive ? "active" : ""}" 
                     ${link.external ? "target=\"_blank\" rel=\"noopener noreferrer\"" : ""}
                   >${link.label}</a>`
    }).join("")
  }
        
        <button class="search-button" aria-label="Search">
          🔍 Search
        </button>
      </nav>
    </header>

    <style>
      .webtui-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: 0.75rem 1.5rem;
        background: var(--background0);
        border-bottom: 1px solid var(--foreground2);
        font-family: "Berkeley Mono", ui-monospace, "Cascadia Code", "Source Code Pro", Menlo, Consolas, "DejaVu Sans Mono", monospace;
        font-size: 0.9rem;
        position: sticky;
        top: 0;
        z-index: 100;
      }

      .header-brand {
        flex-shrink: 0;
      }

      .brand-link {
        display: flex;
        align-items: center;
        gap: 0.5rem;
        text-decoration: none;
        color: var(--foreground1);
        font-weight: 600;
        transition: color 0.2s ease;
      }

      .brand-link:hover {
        color: var(--foreground0);
      }

      .brand-brackets {
        color: var(--foreground0);
        font-weight: 700;
      }

      .brand-text {
        color: var(--foreground1);
      }

      .header-nav {
        display: flex;
        align-items: center;
        gap: 1.5rem;
      }

      .nav-link {
        color: var(--foreground1);
        text-decoration: none;
        padding: 0.25rem 0.5rem;
        border-radius: 4px;
        transition: all 0.2s ease;
        border-bottom: 2px solid transparent;
      }

      .nav-link:hover,
      .nav-link.active {
        color: var(--foreground0);
        border-bottom-color: var(--foreground0);
      }

      .search-button {
        background: var(--background1);
        color: var(--foreground1);
        border: 1px solid var(--foreground2);
        border-radius: 4px;
        padding: 0.5rem 0.75rem;
        font-family: inherit;
        font-size: inherit;
        cursor: pointer;
        transition: all 0.2s ease;
      }

      .search-button:hover {
        background: var(--background2);
        border-color: var(--foreground1);
        color: var(--foreground0);
      }

      /* Responsive design */
      @media (max-width: 768px) {
        .webtui-header {
          padding: 0.5rem 1rem;
          font-size: 0.85rem;
        }

        .header-nav {
          gap: 1rem;
        }

        .nav-link {
          padding: 0.25rem;
        }

        .search-button {
          padding: 0.4rem 0.6rem;
        }
      }

      @media (max-width: 640px) {
        .header-nav {
          gap: 0.75rem;
        }
        
        .nav-link {
          font-size: 0.8rem;
        }
        
        .search-button {
          font-size: 0.8rem;
          padding: 0.3rem 0.5rem;
        }
      }
    </style>
  `
}
