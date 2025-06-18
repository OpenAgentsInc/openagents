import { html } from "@openagentsinc/psionic"

export function docsMenu(currentSlug?: string) {
  return html`
    <div class="docs-menu">
      <div class="docs-menu-section">
        <div class="menu-section-title">Getting Started</div>
        <a href="/docs/getting-started" class="docs-menu-link ${
          currentSlug === "getting-started" ? "active" : ""
        }">
          Quick Start Guide
        </a>
        <a href="/docs/sdk-reference" class="docs-menu-link ${
          currentSlug === "sdk-reference" ? "active" : ""
        }">
          SDK Reference
        </a>
      </div>
      
      <div class="docs-menu-section">
        <div class="menu-section-title">Framework</div>
        <a href="/docs/psionic" class="docs-menu-link ${
          currentSlug === "psionic" ? "active" : ""
        }">
          Psionic Web Framework
        </a>
        <a href="/docs/architecture" class="docs-menu-link ${
          currentSlug === "architecture" ? "active" : ""
        }">
          Architecture Overview
        </a>
      </div>
      
      <div class="docs-menu-section">
        <div class="menu-section-title">Contributing</div>
        <a href="/docs/development" class="docs-menu-link ${
          currentSlug === "development" ? "active" : ""
        }">
          Development Guide
        </a>
        <a href="/docs/roadmap" class="docs-menu-link ${
          currentSlug === "roadmap" ? "active" : ""
        }">
          Roadmap
        </a>
      </div>
      
      <div class="docs-menu-section">
        <div class="menu-section-title">Support</div>
        <a href="/docs/troubleshooting" class="docs-menu-link ${
          currentSlug === "troubleshooting" ? "active" : ""
        }">
          Troubleshooting
        </a>
      </div>
    </div>
  `
}