import { document, html, renderMarkdownWithMetadata } from "@openagentsinc/psionic"
import type { RouteHandler } from "@openagentsinc/psionic"
import fs from "fs/promises"
import path from "path"
import { fileURLToPath } from "url"
import { docsMenu } from "../components/docs-menu"
import { sharedHeader } from "../components/shared-header"
import { baseStyles } from "../styles"

// Get the directory of the current module
const __dirname = path.dirname(fileURLToPath(import.meta.url))
// Navigate from src/routes to content/docs
const DOCS_DIR = path.resolve(__dirname, "..", "..", "content", "docs")

// Main docs index page
export function docs() {
  return document({
    title: "Documentation - OpenAgents",
    styles: baseStyles,
    body: html`
      <!-- Fixed Layout Container -->
      <div class="fixed-layout">
        ${sharedHeader({ current: "docs" })}
        
        <!-- Main Content -->
        <main class="docs-main">
          <!-- Docs Header -->
          <div class="docs-header" box-="square" shear-="bottom">
            <div class="docs-header-content">
              <div class="docs-title">
                <div class="docs-main-title">§ Documentation</div>
                <p>Learn to build Bitcoin-powered AI agents</p>
              </div>
            </div>
          </div>

          <!-- Docs Content -->
          <div class="docs-layout">
            <div class="docs-sidebar">
              <div class="docs-menu">
                <div class="docs-menu-section">
                  <div class="menu-section-title">Getting Started</div>
                  <a href="/docs/getting-started" class="docs-menu-link active">
                    Quick Start Guide
                  </a>
                </div>
                
                <div class="docs-menu-section">
                  <div class="menu-section-title">Core Concepts</div>
                  <a href="/docs/agent-lifecycle" class="docs-menu-link">
                    Agent Lifecycle
                  </a>
                  <a href="/docs/economics" class="docs-menu-link">
                    Economic Model
                  </a>
                  <a href="/docs/bitcoin-integration" class="docs-menu-link">
                    Bitcoin Integration
                  </a>
                </div>
                
                <div class="docs-menu-section">
                  <div class="menu-section-title">API Reference</div>
                  <a href="/docs/api-reference" class="docs-menu-link">
                    Complete API Docs
                  </a>
                  <a href="/docs/sdk-examples" class="docs-menu-link">
                    SDK Examples
                  </a>
                </div>
                
                <div class="docs-menu-section">
                  <div class="menu-section-title">Support</div>
                  <a href="/docs/troubleshooting" class="docs-menu-link">
                    Troubleshooting
                  </a>
                  <a href="/docs/faq" class="docs-menu-link">
                    FAQ
                  </a>
                </div>
              </div>
            </div>

            <div class="docs-content-area">
              <div class="docs-content">
                <div class="welcome-section">
                  <div class="section-title">» Welcome to OpenAgents</div>
                  <p class="lead">
                    Build autonomous AI agents that earn Bitcoin by providing valuable services. Each agent must sustain itself economically, ensuring alignment with user needs.
                  </p>
                  
                  <div class="quick-links">
                    <a href="/docs/getting-started" class="quick-link-card" box-="square">
                      <div class="card-title">» Getting Started</div>
                      <p>Create your first AI agent with the SDK</p>
                    </a>
                    
                    <a href="/docs/sdk-reference" class="quick-link-card" box-="square">
                      <div class="card-title">¤ SDK Reference</div>
                      <p>Complete API documentation</p>
                    </a>
                    
                    <a href="/docs/psionic" class="quick-link-card" box-="square">
                      <div class="card-title">° Psionic Framework</div>
                      <p>Build web applications with our framework</p>
                    </a>
                  </div>
                </div>
                
                <div class="features-section">
                  <div class="section-title">※ Key Features</div>
                  <div class="features-grid">
                    <div class="feature-item" box-="square">
                      <div class="feature-title">◊ Intelligent Agents</div>
                      <p>AI agents that learn and adapt to provide better services over time</p>
                    </div>
                    
                    <div class="feature-item" box-="square">
                      <div class="feature-title">¤ Bitcoin Economics</div>
                      <p>Agents must earn Bitcoin to survive, naturally aligning with human needs</p>
                    </div>
                    
                    <div class="feature-item" box-="square">
                      <div class="feature-title">§ Open Protocols</div>
                      <p>Built on Nostr for identity and Lightning for instant micropayments</p>
                    </div>
                    
                    <div class="feature-item" box-="square">
                      <div class="feature-title">° Real-time Streaming</div>
                      <p>Streaming inference with live token generation and cost tracking</p>
                    </div>
                  </div>
                </div>
                
                <div class="code-example">
                  <div class="section-title">¤ Quick Example</div>
                  <pre is-="pre" box-="square"><code># Install the SDK
pnpm add @openagentsinc/sdk

# Create your first agent
import { Agent, Inference } from '@openagentsinc/sdk'

const agent = Agent.create({
  name: "Universal Translator",
  capabilities: ["translation"],
  pricing: { per_request: 100 } // 100 sats per translation
})

// Make the agent earn Bitcoin
const translation = await Inference.infer({
  system: "Translate to Spanish",
  messages: [{ role: "user", content: "Hello world" }],
  model: "llama3.2"
})

console.log(translation.content) // "Hola mundo"</code></pre>
                </div>
              </div>
            </div>
          </div>
        </main>
      </div>

      <style>
        body {
          background: var(--background0);
          margin: 0;
          padding: 0;
          min-height: 100vh;
          font-family: "Berkeley Mono", ui-monospace, "Cascadia Code", "Source Code Pro", Menlo, Consolas, "DejaVu Sans Mono", monospace;
        }

        /* Fixed Layout */
        .fixed-layout {
          width: 100vw;
          height: 100vh;
          display: flex;
          flex-direction: column;
          overflow: hidden;
        }

        /* Main Content */
        .docs-main {
          flex: 1;
          overflow-y: auto;
          padding: 2rem;
        }

        /* Override WebTUI heading styles to remove # symbols */
        .docs-main-title::before,
        .section-title::before,
        .card-title::before,
        .feature-title::before,
        .menu-section-title::before,
        h1::before,
        h2::before,
        h3::before,
        h4::before,
        h5::before,
        h6::before {
          content: "" !important;
        }

        /* Docs Header */
        .docs-header {
          padding: 2rem;
          background: var(--background1);
          margin-bottom: 2rem;
        }

        .docs-header-content {
          display: flex;
          align-items: center;
          justify-content: center;
          max-width: 1200px;
          margin: 0 auto;
        }

        .docs-main-title {
          margin: 0 0 0.5rem 0;
          color: var(--foreground0);
          font-size: 2rem;
          font-weight: 700;
        }

        .docs-title p {
          margin: 0;
          color: var(--foreground1);
          font-size: 0.9rem;
        }


        /* Docs Layout */
        .docs-layout {
          display: flex;
          gap: 2rem;
          max-width: 1200px;
          margin: 0 auto;
        }

        .docs-sidebar {
          width: 280px;
          min-width: 280px;
          background: var(--background1);
          padding: 2rem;
          height: fit-content;
        }

        .docs-menu-section {
          margin-bottom: 2rem;
        }

        .menu-section-title {
          margin: 0 0 1rem 0;
          color: var(--foreground0);
          font-size: 0.75rem;
          font-weight: 600;
          text-transform: uppercase;
          letter-spacing: 0.05em;
        }

        .docs-menu-link {
          display: block;
          padding: 0.5rem;
          margin-bottom: 0.25rem;
          color: var(--foreground1);
          text-decoration: none;
          font-size: 0.875rem;
          border-radius: 4px;
          transition: all 0.2s;
        }

        .docs-menu-link:hover,
        .docs-menu-link.active {
          background: var(--background2);
          color: var(--foreground0);
        }

        /* Content Area */
        .docs-content-area {
          flex: 1;
          min-width: 0;
        }

        .docs-content {
          max-width: 800px;
        }

        .welcome-section {
          margin-bottom: 4rem;
        }

        .section-title {
          margin: 0 0 2rem 0;
          color: var(--foreground0);
          font-size: 1.5rem;
          font-weight: 600;
        }

        .lead {
          font-size: 1.1rem;
          line-height: 1.7;
          color: var(--foreground1);
          margin-bottom: 3rem;
        }

        .quick-links {
          display: grid;
          grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
          gap: 1.5rem;
          margin-bottom: 4rem;
        }

        .quick-link-card {
          display: block;
          padding: 2rem;
          text-decoration: none;
          color: inherit;
          transition: all 0.2s;
          background: var(--background1);
        }

        .quick-link-card:hover {
          background: var(--background2);
          transform: translateY(-2px);
        }

        .card-title {
          margin: 0 0 1rem 0;
          color: var(--foreground0);
          font-size: 1.1rem;
          font-weight: 600;
        }

        .quick-link-card p {
          margin: 0;
          color: var(--foreground1);
          font-size: 0.9rem;
          line-height: 1.5;
        }

        .features-section {
          margin-bottom: 4rem;
        }

        .features-grid {
          display: grid;
          grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
          gap: 1.5rem;
        }

        .feature-item {
          padding: 2rem;
          background: var(--background1);
        }

        .feature-title {
          margin: 0 0 1rem 0;
          color: var(--foreground0);
          font-size: 1rem;
          font-weight: 600;
        }

        .feature-item p {
          margin: 0;
          color: var(--foreground1);
          font-size: 0.9rem;
          line-height: 1.5;
        }

        .code-example {
          margin-bottom: 4rem;
        }

        pre[is-="pre"] {
          font-family: inherit;
          font-size: 0.875rem;
          line-height: 1.6;
          padding: 2rem !important;
        }

        /* Kill all HR styles and rebuild */
        hr {
          all: unset !important;
          display: block !important;
          width: 100% !important;
          height: 0 !important;
          border: 0 !important;
          border-top: 1px solid var(--foreground1) !important;
          margin: 2rem 0 !important;
          padding: 0 !important;
          background: transparent !important;
          color: transparent !important;
          opacity: 0.3 !important;
        }

        /* Mobile responsiveness */
        @media (max-width: 768px) {
          .docs-main {
            padding: 1rem;
          }

          .docs-header {
            padding: 1.5rem;
          }

          .docs-header-content {
            flex-direction: column;
            align-items: center;
            gap: 1rem;
          }

          .docs-layout {
            flex-direction: column;
            gap: 1rem;
          }

          .docs-sidebar {
            width: 100%;
            min-width: 100%;
          }

          .quick-links,
          .features-grid {
            grid-template-columns: 1fr;
          }
        }
      </style>
    `
  })
}

// Individual documentation page handler
export const docPage: RouteHandler = async (context) => {
  const slug = context.params?.slug as string

  if (!slug) {
    return docs() // Return index if no slug
  }

  try {
    const filePath = path.join(DOCS_DIR, `${slug}.md`)
    const content = await fs.readFile(filePath, "utf-8")
    const result = renderMarkdownWithMetadata(content)

    return document({
      title: `${result.metadata.title} - OpenAgents Documentation`,
      styles: baseStyles,
      body: html`
        <!-- Fixed Layout Container -->
        <div class="fixed-layout">
          ${sharedHeader({ current: "docs" })}
          
          <!-- Main Content -->
          <main class="docs-main">
            <!-- Docs Content -->
            <div class="docs-layout">
              <div class="docs-sidebar">
                ${docsMenu(slug)}
              </div>

              <div class="docs-content-area">
                <div class="docs-content">
                  <div class="doc-container">
                    <div class="doc-content" box-="square">
                      <article class="doc-article">
                        <header class="doc-header">
                          <div class="doc-title">${result.metadata.title}</div>
                          ${result.metadata.summary ? `<p class="doc-summary">${result.metadata.summary}</p>` : ""}
                        </header>
                        
                        <div class="doc-body">
                          ${result.html}
                        </div>
                        
                        <footer class="doc-footer">
                          <div class="doc-nav-buttons">
                            <a href="/docs" is-="button" variant-="foreground1" class="doc-nav-button">
                              ← Back to Docs
                            </a>
                          </div>
                        </footer>
                      </article>
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </main>
        </div>

        <style>
          body {
            background: var(--background0);
            margin: 0;
            padding: 0;
            min-height: 100vh;
            font-family: "Berkeley Mono", ui-monospace, "Cascadia Code", "Source Code Pro", Menlo, Consolas, "DejaVu Sans Mono", monospace;
          }

          /* Fixed Layout */
          .fixed-layout {
            width: 100vw;
            height: 100vh;
            display: flex;
            flex-direction: column;
            overflow: hidden;
          }

          /* Override WebTUI heading styles to remove # symbols */
          .doc-title::before,
          .doc-body h1::before,
          .doc-body h2::before,
          .doc-body h3::before,
          .doc-body h4::before,
          .doc-body h5::before,
          .doc-body h6::before {
            content: "" !important;
          }

          /* Docs Main */
          .docs-main {
            flex: 1;
            overflow-y: auto;
            padding: 2rem;
          }

          /* Docs Layout */
          .docs-layout {
            display: flex;
            gap: 2rem;
            max-width: 1200px;
            margin: 0 auto;
          }

          .docs-sidebar {
            width: 280px;
            min-width: 280px;
            background: var(--background1);
            padding: 2rem;
            height: fit-content;
          }

          .docs-menu-section {
            margin-bottom: 2rem;
          }

          .menu-section-title {
            margin: 0 0 1rem 0;
            color: var(--foreground0);
            font-size: 0.75rem;
            font-weight: 600;
            text-transform: uppercase;
            letter-spacing: 0.05em;
          }

          .docs-menu-link {
            display: block;
            padding: 0.5rem;
            margin-bottom: 0.25rem;
            color: var(--foreground1);
            text-decoration: none;
            font-size: 0.875rem;
            border-radius: 4px;
            transition: all 0.2s;
          }

          .docs-menu-link:hover,
          .docs-menu-link.active {
            background: var(--background2);
            color: var(--foreground0);
          }

          /* Content Area */
          .docs-content-area {
            flex: 1;
            min-width: 0;
          }

          .docs-content {
            max-width: 800px;
          }

          .doc-container {
            max-width: 900px;
            margin: 0 auto;
          }

          .doc-content {
            background: var(--background1);
          }

          .doc-article {
            padding: 3rem;
          }

          /* Doc Header */
          .doc-header {
            margin-bottom: 3rem;
            padding-bottom: 2rem;
            border-bottom: 1px solid var(--foreground2);
          }

          .doc-title {
            margin: 0 0 1rem 0;
            font-size: 2.25rem;
            font-weight: 700;
            color: var(--foreground0);
            line-height: 1.2;
          }

          .doc-summary {
            font-size: 1.125rem;
            line-height: 1.7;
            color: var(--foreground1);
            margin: 0;
          }

          /* Doc Body - Enhanced Typography */
          .doc-body {
            line-height: 1.8;
            color: var(--foreground1);
          }

          .doc-body h1,
          .doc-body h2,
          .doc-body h3,
          .doc-body h4,
          .doc-body h5,
          .doc-body h6 {
            color: var(--foreground0);
            margin: 2.5rem 0 1rem 0;
            font-weight: 600;
            line-height: 1.3;
          }

          .doc-body h1 {
            font-size: 1.875rem;
          }

          .doc-body h2 {
            font-size: 1.5rem;
          }

          .doc-body h3 {
            font-size: 1.25rem;
          }

          .doc-body p {
            margin: 1.5rem 0;
            line-height: 1.8;
          }

          .doc-body ul,
          .doc-body ol {
            margin: 1.5rem 0;
            padding-left: 2rem;
          }

          .doc-body li {
            margin: 0.5rem 0;
            line-height: 1.7;
          }

          .doc-body blockquote {
            margin: 2rem 0;
            padding: 1rem 1.5rem;
            border-left: 4px solid var(--foreground2);
            background: var(--background2);
            font-style: italic;
            color: var(--foreground2);
          }

          .doc-body code {
            background: var(--background2);
            color: var(--foreground0);
            padding: 0.25rem 0.5rem;
            border-radius: 3px;
            font-size: 0.9em;
            font-family: inherit;
          }

          .doc-body pre {
            background: var(--background0);
            color: var(--foreground1);
            padding: 2rem;
            border-radius: 4px;
            margin: 2rem 0;
            overflow-x: auto;
            border: 1px solid var(--foreground2);
          }

          .doc-body pre code {
            background: transparent;
            padding: 0;
            border-radius: 0;
            font-size: 0.85em;
          }

          .doc-body a {
            color: var(--foreground0);
            text-decoration: underline;
            transition: color 0.2s ease;
          }

          .doc-body a:hover {
            color: var(--foreground1);
          }

          /* Kill all HR styles and rebuild */
          hr,
          .doc-body hr,
          .doc-article hr,
          .doc-content hr,
          article hr,
          div hr {
            all: unset !important;
            display: block !important;
            width: 100% !important;
            height: 0 !important;
            border: 0 !important;
            border-top: 1px solid var(--foreground1) !important;
            margin: 3rem 0 !important;
            padding: 0 !important;
            background: transparent !important;
            color: transparent !important;
            opacity: 0.3 !important;
          }

          .doc-body table {
            width: 100%;
            border-collapse: collapse;
            margin: 2rem 0;
          }

          .doc-body th,
          .doc-body td {
            padding: 0.75rem;
            border: 1px solid var(--foreground2);
            text-align: left;
          }

          .doc-body th {
            background: var(--background2);
            font-weight: 600;
            color: var(--foreground0);
          }

          .doc-body img {
            max-width: 100%;
            height: auto;
            margin: 2rem 0;
            border-radius: 4px;
          }

          /* Doc Footer */
          .doc-footer {
            margin-top: 3rem;
            padding-top: 2rem;
            border-top: 1px solid var(--foreground2);
          }

          .doc-nav-buttons {
            display: flex;
            justify-content: space-between;
            gap: 1.5rem;
            flex-wrap: wrap;
          }

          .doc-nav-button {
            text-decoration: none;
          }

          /* Responsive */
          @media (max-width: 768px) {
            .docs-main {
              padding: 1rem;
            }

            .docs-layout {
              flex-direction: column;
              gap: 1rem;
            }

            .docs-sidebar {
              width: 100%;
              min-width: 100%;
            }

            .doc-article {
              padding: 2rem;
            }

            .doc-title {
              font-size: 1.875rem;
            }

            .doc-body h1 {
              font-size: 1.5rem;
            }

            .doc-body h2 {
              font-size: 1.25rem;
            }

            .doc-nav-buttons {
              flex-direction: column;
            }
          }
        </style>
      `
    })
  } catch (error) {
    console.error(`Error loading doc page ${slug}:`, error)

    return document({
      title: "Page Not Found - OpenAgents Documentation",
      styles: baseStyles,
      body: html`
        ${sharedHeader({ current: "docs" })}
        
        <div class="container">
          <div style="text-align: center; padding: 4rem 2rem;">
            <h1 style="color: var(--foreground1); margin-bottom: 1rem;">Documentation Page Not Found</h1>
            <p style="color: var(--foreground0); margin-bottom: 2rem;">
              The documentation page "${slug}" doesn't exist.
            </p>
            <a href="/docs" is-="button" variant-="foreground1">
              ← Back to Documentation
            </a>
          </div>
        </div>
      `
    })
  }
}
