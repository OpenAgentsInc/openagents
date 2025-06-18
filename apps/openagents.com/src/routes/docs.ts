import { document, html, renderMarkdownWithMetadata } from "@openagentsinc/psionic"
import type { RouteHandler } from "@openagentsinc/psionic"
import fs from "fs/promises"
import path from "path"
import { navigation } from "../components/navigation"
import { baseStyles } from "../styles"

const DOCS_DIR = path.join(process.cwd(), "content", "docs")

// Main docs index page
export function docs() {
  return document({
    title: "Documentation - OpenAgents",
    styles: baseStyles,
    body: html`
      ${navigation({ current: "docs" })}
      
      <!-- Docs Header -->
      <div class="docs-header">
        <div class="docs-header-content">
          <div class="docs-title">
            <h1>📚 Documentation</h1>
            <p>Learn to build Bitcoin-powered AI agents</p>
          </div>
          
          <div class="docs-nav">
            <a href="/docs/getting-started" class="docs-nav-item active">
              📖 Getting Started
            </a>
            <a href="/docs/api-reference" class="docs-nav-item">
              🔧 API Reference
            </a>
            <a href="/docs/agent-lifecycle" class="docs-nav-item">
              ⚡ Agent Lifecycle
            </a>
            <a href="/docs/troubleshooting" class="docs-nav-item">
              🐛 Troubleshooting
            </a>
          </div>
          
          <div class="docs-search">
            <input type="text" placeholder="Search docs..." />
            <span class="search-icon">🔍</span>
          </div>
        </div>
      </div>

      <!-- Docs Content -->
      <div class="docs-layout">
        <div class="docs-sidebar">
          <div class="docs-menu" box-="square">
            <div class="docs-menu-section">
              <h3>Getting Started</h3>
              <a href="/docs/getting-started" class="docs-menu-link active">
                Quick Start Guide
              </a>
            </div>
            
            <div class="docs-menu-section">
              <h3>Core Concepts</h3>
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
              <h3>API Reference</h3>
              <a href="/docs/api-reference" class="docs-menu-link">
                Complete API Docs
              </a>
              <a href="/docs/sdk-examples" class="docs-menu-link">
                SDK Examples
              </a>
            </div>
            
            <div class="docs-menu-section">
              <h3>Support</h3>
              <a href="/docs/troubleshooting" class="docs-menu-link">
                Troubleshooting
              </a>
              <a href="/docs/faq" class="docs-menu-link">
                FAQ
              </a>
            </div>
          </div>
        </div>

        <div class="docs-main">
          <div class="docs-content" box-="square">
            <div class="welcome-section">
              <h2>🚀 Welcome to OpenAgents</h2>
              <p class="lead">
                Build autonomous AI agents that earn Bitcoin by providing valuable services. 
                Each agent must sustain itself economically, ensuring alignment with user needs.
              </p>
              
              <div class="quick-links">
                <a href="/docs/getting-started" class="quick-link-card" box-="square">
                  <div class="quick-link-icon">📖</div>
                  <h3>Getting Started</h3>
                  <p>Create your first Bitcoin-powered agent in minutes</p>
                </a>
                
                <a href="/docs/api-reference" class="quick-link-card" box-="square">
                  <div class="quick-link-icon">🔧</div>
                  <h3>API Reference</h3>
                  <p>Complete SDK documentation and examples</p>
                </a>
                
                <a href="/docs/agent-lifecycle" class="quick-link-card" box-="square">
                  <div class="quick-link-icon">⚡</div>
                  <h3>Agent Lifecycle</h3>
                  <p>Understanding agent economics and survival</p>
                </a>
              </div>
            </div>
            
            <div class="features-section">
              <h2>🌟 Key Features</h2>
              <div class="features-grid">
                <div class="feature-item" box-="square">
                  <h3>🧠 Intelligent Agents</h3>
                  <p>AI agents that learn and adapt to provide better services over time</p>
                </div>
                
                <div class="feature-item" box-="square">
                  <h3>💰 Bitcoin Economics</h3>
                  <p>Agents must earn Bitcoin to survive, naturally aligning with human needs</p>
                </div>
                
                <div class="feature-item" box-="square">
                  <h3>🔓 Open Protocols</h3>
                  <p>Built on Nostr for identity and Lightning for instant micropayments</p>
                </div>
                
                <div class="feature-item" box-="square">
                  <h3>⚡ Real-time Streaming</h3>
                  <p>Streaming inference with live token generation and cost tracking</p>
                </div>
              </div>
            </div>
            
            <div class="code-example">
              <h2>💻 Quick Example</h2>
              <pre is-="pre"><code># Install the SDK
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

      <style>
        .docs-header {
          background: var(--background1);
          border-bottom: 2px solid var(--background2);
          padding: 1rem 0;
        }
        
        .docs-header-content {
          max-width: 1400px;
          margin: 0 auto;
          padding: 0 1rem;
          display: flex;
          align-items: center;
          gap: 2rem;
          flex-wrap: wrap;
        }
        
        .docs-title h1 {
          margin: 0;
          color: var(--foreground1);
          font-size: 1.5rem;
        }
        
        .docs-title p {
          margin: 0;
          color: var(--foreground0);
          font-size: 0.9rem;
        }
        
        .docs-nav {
          display: flex;
          gap: 0.5rem;
          flex-wrap: wrap;
        }
        
        .docs-nav-item {
          padding: 0.5rem 1rem;
          color: var(--foreground0);
          text-decoration: none;
          border-radius: 4px;
          font-size: 0.9rem;
          transition: all 0.2s;
          border: 1px solid transparent;
        }
        
        .docs-nav-item:hover {
          background: var(--background2);
          color: var(--foreground1);
        }
        
        .docs-nav-item.active {
          background: var(--background2);
          color: var(--foreground1);
          border-color: var(--background3);
        }
        
        .docs-search {
          margin-left: auto;
          position: relative;
        }
        
        .docs-search input {
          padding: 0.5rem 2.5rem 0.5rem 1rem;
          background: var(--background0);
          border: 1px solid var(--background2);
          border-radius: 4px;
          color: var(--foreground1);
          font-family: var(--font-family);
          font-size: 0.9rem;
          width: 200px;
        }
        
        .docs-search input:focus {
          outline: none;
          border-color: var(--foreground0);
        }
        
        .search-icon {
          position: absolute;
          right: 0.75rem;
          top: 50%;
          transform: translateY(-50%);
          color: var(--foreground0);
          pointer-events: none;
        }
        
        .docs-layout {
          display: flex;
          max-width: 1400px;
          margin: 0 auto;
          gap: 1rem;
          padding: 1rem;
        }
        
        .docs-sidebar {
          width: 280px;
          min-width: 280px;
        }
        
        .docs-menu {
          padding: 1.5rem;
          position: sticky;
          top: 1rem;
        }
        
        .docs-menu-section {
          margin-bottom: 2rem;
        }
        
        .docs-menu-section:last-child {
          margin-bottom: 0;
        }
        
        .docs-menu-section h3 {
          margin: 0 0 0.75rem 0;
          color: var(--foreground1);
          font-size: 0.9rem;
          font-weight: 600;
        }
        
        .docs-menu-link {
          display: block;
          padding: 0.5rem 0;
          color: var(--foreground0);
          text-decoration: none;
          font-size: 0.85rem;
          line-height: 1.4;
          transition: color 0.2s;
        }
        
        .docs-menu-link:hover {
          color: var(--foreground1);
        }
        
        .docs-menu-link.active {
          color: var(--foreground1);
          font-weight: 500;
        }
        
        .docs-main {
          flex: 1;
          min-width: 0;
        }
        
        .docs-content {
          padding: 2rem;
        }
        
        .welcome-section {
          margin-bottom: 3rem;
        }
        
        .welcome-section h2 {
          margin: 0 0 1rem 0;
          color: var(--foreground1);
        }
        
        .lead {
          font-size: 1.1rem;
          line-height: 1.6;
          color: var(--foreground0);
          margin-bottom: 2rem;
        }
        
        .quick-links {
          display: grid;
          grid-template-columns: repeat(auto-fit, minmax(250px, 1fr));
          gap: 1rem;
        }
        
        .quick-link-card {
          display: block;
          padding: 1.5rem;
          text-decoration: none;
          color: inherit;
          transition: all 0.2s;
        }
        
        .quick-link-card:hover {
          background: var(--background1);
        }
        
        .quick-link-icon {
          font-size: 2rem;
          margin-bottom: 0.5rem;
        }
        
        .quick-link-card h3 {
          margin: 0 0 0.5rem 0;
          color: var(--foreground1);
          font-size: 1rem;
        }
        
        .quick-link-card p {
          margin: 0;
          color: var(--foreground0);
          font-size: 0.9rem;
          line-height: 1.4;
        }
        
        .features-section {
          margin-bottom: 3rem;
        }
        
        .features-section h2 {
          margin: 0 0 1.5rem 0;
          color: var(--foreground1);
        }
        
        .features-grid {
          display: grid;
          grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
          gap: 1rem;
        }
        
        .feature-item {
          padding: 1.5rem;
        }
        
        .feature-item h3 {
          margin: 0 0 0.75rem 0;
          color: var(--foreground1);
          font-size: 1rem;
        }
        
        .feature-item p {
          margin: 0;
          color: var(--foreground0);
          font-size: 0.9rem;
          line-height: 1.5;
        }
        
        .code-example h2 {
          margin: 0 0 1rem 0;
          color: var(--foreground1);
        }
        
        /* Mobile responsiveness */
        @media (max-width: 768px) {
          .docs-header-content {
            flex-direction: column;
            align-items: stretch;
            gap: 1rem;
          }
          
          .docs-nav {
            justify-content: center;
          }
          
          .docs-search {
            margin-left: 0;
          }
          
          .docs-search input {
            width: 100%;
          }
          
          .docs-layout {
            flex-direction: column;
          }
          
          .docs-sidebar {
            width: 100%;
            min-width: 100%;
          }
          
          .docs-menu {
            position: static;
          }
          
          .quick-links {
            grid-template-columns: 1fr;
          }
          
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
        ${navigation({ current: "docs" })}
        
        <!-- Docs Header -->
        <div class="docs-header">
          <div class="docs-header-content">
            <div class="docs-title">
              <h1>📚 Documentation</h1>
              <p>Learn to build Bitcoin-powered AI agents</p>
            </div>
            
            <div class="docs-nav">
              <a href="/docs/getting-started" class="docs-nav-item ${slug === "getting-started" ? "active" : ""}">
                📖 Getting Started
              </a>
              <a href="/docs/api-reference" class="docs-nav-item ${slug === "api-reference" ? "active" : ""}">
                🔧 API Reference
              </a>
              <a href="/docs/agent-lifecycle" class="docs-nav-item ${slug === "agent-lifecycle" ? "active" : ""}">
                ⚡ Agent Lifecycle
              </a>
              <a href="/docs/troubleshooting" class="docs-nav-item ${slug === "troubleshooting" ? "active" : ""}">
                🐛 Troubleshooting
              </a>
            </div>
            
            <div class="docs-search">
              <input type="text" placeholder="Search docs..." />
              <span class="search-icon">🔍</span>
            </div>
          </div>
        </div>

        <!-- Docs Content -->
        <div class="docs-layout">
          <div class="docs-sidebar">
            <div class="docs-menu" box-="square">
              <div class="docs-menu-section">
                <h3>Getting Started</h3>
                <a href="/docs/getting-started" class="docs-menu-link ${slug === "getting-started" ? "active" : ""}">
                  Quick Start Guide
                </a>
              </div>
              
              <div class="docs-menu-section">
                <h3>Core Concepts</h3>
                <a href="/docs/agent-lifecycle" class="docs-menu-link ${slug === "agent-lifecycle" ? "active" : ""}">
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
                <h3>API Reference</h3>
                <a href="/docs/api-reference" class="docs-menu-link ${slug === "api-reference" ? "active" : ""}">
                  Complete API Docs
                </a>
                <a href="/docs/sdk-examples" class="docs-menu-link">
                  SDK Examples
                </a>
              </div>
              
              <div class="docs-menu-section">
                <h3>Support</h3>
                <a href="/docs/troubleshooting" class="docs-menu-link ${slug === "troubleshooting" ? "active" : ""}">
                  Troubleshooting
                </a>
                <a href="/docs/faq" class="docs-menu-link">
                  FAQ
                </a>
              </div>
            </div>
          </div>

          <div class="docs-main">
            <article class="docs-content" box-="square">
              <div class="doc-header">
                <h1>${result.metadata.title}</h1>
                ${result.metadata.summary ? `<p class="doc-summary">${result.metadata.summary}</p>` : ""}
              </div>
              
              <div class="doc-body">
                ${result.html}
              </div>
              
              <div class="doc-footer">
                <div class="doc-nav-buttons">
                  <a href="/docs" class="doc-nav-button" is-="button" variant-="background1">
                    ← Back to Docs
                  </a>
                  <a href="https://github.com/OpenAgentsInc/openagents/edit/main/content/docs/${slug}.md" 
                     class="doc-nav-button" is-="button" variant-="background1">
                    Edit on GitHub →
                  </a>
                </div>
              </div>
            </article>
          </div>
        </div>

        <style>
          .docs-header {
            background: var(--background1);
            border-bottom: 2px solid var(--background2);
            padding: 1rem 0;
          }
          
          .docs-header-content {
            max-width: 1400px;
            margin: 0 auto;
            padding: 0 1rem;
            display: flex;
            align-items: center;
            gap: 2rem;
            flex-wrap: wrap;
          }
          
          .docs-title h1 {
            margin: 0;
            color: var(--foreground1);
            font-size: 1.5rem;
          }
          
          .docs-title p {
            margin: 0;
            color: var(--foreground0);
            font-size: 0.9rem;
          }
          
          .docs-nav {
            display: flex;
            gap: 0.5rem;
            flex-wrap: wrap;
          }
          
          .docs-nav-item {
            padding: 0.5rem 1rem;
            color: var(--foreground0);
            text-decoration: none;
            border-radius: 4px;
            font-size: 0.9rem;
            transition: all 0.2s;
            border: 1px solid transparent;
          }
          
          .docs-nav-item:hover {
            background: var(--background2);
            color: var(--foreground1);
          }
          
          .docs-nav-item.active {
            background: var(--background2);
            color: var(--foreground1);
            border-color: var(--background3);
          }
          
          .docs-search {
            margin-left: auto;
            position: relative;
          }
          
          .docs-search input {
            padding: 0.5rem 2.5rem 0.5rem 1rem;
            background: var(--background0);
            border: 1px solid var(--background2);
            border-radius: 4px;
            color: var(--foreground1);
            font-family: var(--font-family);
            font-size: 0.9rem;
            width: 200px;
          }
          
          .docs-search input:focus {
            outline: none;
            border-color: var(--foreground0);
          }
          
          .search-icon {
            position: absolute;
            right: 0.75rem;
            top: 50%;
            transform: translateY(-50%);
            color: var(--foreground0);
            pointer-events: none;
          }
          
          .docs-layout {
            display: flex;
            max-width: 1400px;
            margin: 0 auto;
            gap: 1rem;
            padding: 1rem;
          }
          
          .docs-sidebar {
            width: 280px;
            min-width: 280px;
          }
          
          .docs-menu {
            padding: 1.5rem;
            position: sticky;
            top: 1rem;
          }
          
          .docs-menu-section {
            margin-bottom: 2rem;
          }
          
          .docs-menu-section:last-child {
            margin-bottom: 0;
          }
          
          .docs-menu-section h3 {
            margin: 0 0 0.75rem 0;
            color: var(--foreground1);
            font-size: 0.9rem;
            font-weight: 600;
          }
          
          .docs-menu-link {
            display: block;
            padding: 0.5rem 0;
            color: var(--foreground0);
            text-decoration: none;
            font-size: 0.85rem;
            line-height: 1.4;
            transition: color 0.2s;
          }
          
          .docs-menu-link:hover {
            color: var(--foreground1);
          }
          
          .docs-menu-link.active {
            color: var(--foreground1);
            font-weight: 500;
          }
          
          .docs-main {
            flex: 1;
            min-width: 0;
          }
          
          .docs-content {
            padding: 2rem;
          }
          
          .doc-header h1 {
            margin: 0 0 1rem 0;
            color: var(--foreground1);
            font-size: 2.5rem;
          }
          
          .doc-summary {
            font-size: 1.1rem;
            line-height: 1.6;
            color: var(--foreground0);
            margin-bottom: 2rem;
            padding-bottom: 2rem;
            border-bottom: 1px solid var(--background2);
          }
          
          .doc-body {
            line-height: 1.7;
            color: var(--foreground1);
          }
          
          .doc-body h1,
          .doc-body h2,
          .doc-body h3,
          .doc-body h4,
          .doc-body h5,
          .doc-body h6 {
            color: var(--foreground1);
            margin-top: 2rem;
            margin-bottom: 1rem;
          }
          
          .doc-body h1 {
            font-size: 2rem;
            border-bottom: 2px solid var(--background2);
            padding-bottom: 0.5rem;
          }
          
          .doc-body h2 {
            font-size: 1.5rem;
          }
          
          .doc-body h3 {
            font-size: 1.25rem;
          }
          
          .doc-body p {
            margin-bottom: 1rem;
            color: var(--foreground0);
          }
          
          .doc-body ul,
          .doc-body ol {
            margin-bottom: 1rem;
            color: var(--foreground0);
            padding-left: 1.5rem;
          }
          
          .doc-body li {
            margin-bottom: 0.5rem;
          }
          
          .doc-body code {
            background: var(--background1);
            padding: 0.2rem 0.4rem;
            border-radius: 3px;
            font-family: var(--font-family);
            font-size: 0.9em;
            color: var(--foreground1);
          }
          
          .doc-body pre {
            background: var(--background1);
            padding: 1rem;
            border-radius: 4px;
            overflow-x: auto;
            margin-bottom: 1rem;
            border: 1px solid var(--background2);
          }
          
          .doc-body pre code {
            background: none;
            padding: 0;
            color: var(--foreground1);
          }
          
          .doc-body blockquote {
            border-left: 4px solid var(--background2);
            padding-left: 1rem;
            margin: 1rem 0;
            color: var(--foreground0);
            font-style: italic;
          }
          
          .doc-body a {
            color: var(--foreground1);
            text-decoration: underline;
          }
          
          .doc-body a:hover {
            color: var(--foreground2);
          }
          
          .doc-footer {
            margin-top: 3rem;
            padding-top: 2rem;
            border-top: 1px solid var(--background2);
          }
          
          .doc-nav-buttons {
            display: flex;
            justify-content: space-between;
            gap: 1rem;
            flex-wrap: wrap;
          }
          
          .doc-nav-button {
            text-decoration: none;
          }
          
          /* Mobile responsiveness */
          @media (max-width: 768px) {
            .docs-header-content {
              flex-direction: column;
              align-items: stretch;
              gap: 1rem;
            }
            
            .docs-nav {
              justify-content: center;
            }
            
            .docs-search {
              margin-left: 0;
            }
            
            .docs-search input {
              width: 100%;
            }
            
            .docs-layout {
              flex-direction: column;
            }
            
            .docs-sidebar {
              width: 100%;
              min-width: 100%;
            }
            
            .docs-menu {
              position: static;
            }
            
            .doc-header h1 {
              font-size: 2rem;
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
        ${navigation({ current: "docs" })}
        
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
