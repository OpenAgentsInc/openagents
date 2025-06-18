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
            <h1># 📚 Documentation</h1>
            <p>Learn to build Bitcoin-powered AI agents</p>
          </div>
          
          <div class="docs-search">
            <input type="text" placeholder="Search docs..." />
          </div>
        </div>
      </div>

      <!-- Docs Content -->
      <div class="docs-layout">
        <div class="docs-sidebar">
          <div class="docs-menu">
            <div class="docs-menu-section">
              <h3>### Getting Started</h3>
              <a href="/docs/getting-started" class="docs-menu-link active">
                Quick Start Guide
              </a>
            </div>
            
            <div class="docs-menu-section">
              <h3>### Core Concepts</h3>
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
              <h3>### API Reference</h3>
              <a href="/docs/api-reference" class="docs-menu-link">
                Complete API Docs
              </a>
              <a href="/docs/sdk-examples" class="docs-menu-link">
                SDK Examples
              </a>
            </div>
            
            <div class="docs-menu-section">
              <h3>### Support</h3>
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
          <div class="docs-content">
            <div class="welcome-section">
              <h2>## 🚀 Welcome to OpenAgents</h2>
              <p class="lead">
                Build autonomous AI agents that earn Bitcoin by providing valuable services. Each agent must sustain itself economically, ensuring alignment with user needs.
              </p>
              
              <div class="quick-links">
                <a href="/docs/getting-started" class="quick-link-card">
                  <h3>### 📖 Getting Started</h3>
                  <p>Create your first Bitcoin-powered agent in minutes</p>
                </a>
                
                <a href="/docs/api-reference" class="quick-link-card">
                  <h3>### 🔧 API Reference</h3>
                  <p>Complete SDK documentation and examples</p>
                </a>
                
                <a href="/docs/agent-lifecycle" class="quick-link-card">
                  <h3>### ⚡ Agent Lifecycle</h3>
                  <p>Understanding agent economics and survival</p>
                </a>
              </div>
            </div>
            
            <div class="features-section">
              <h2>## 🌟 Key Features</h2>
              <div class="features-grid">
                <div class="feature-item">
                  <h3>### 🧠 Intelligent Agents</h3>
                  <p>AI agents that learn and adapt to provide better services over time</p>
                </div>
                
                <div class="feature-item">
                  <h3>### 💰 Bitcoin Economics</h3>
                  <p>Agents must earn Bitcoin to survive, naturally aligning with human needs</p>
                </div>
                
                <div class="feature-item">
                  <h3>### 🔓 Open Protocols</h3>
                  <p>Built on Nostr for identity and Lightning for instant micropayments</p>
                </div>
                
                <div class="feature-item">
                  <h3>### ⚡ Real-time Streaming</h3>
                  <p>Streaming inference with live token generation and cost tracking</p>
                </div>
              </div>
            </div>
            
            <div class="code-example">
              <h2>## 💻 Quick Example</h2>
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
          background: var(--background0);
          border-bottom: 1px solid var(--background2);
          padding: 1.5rem 0;
        }
        
        .docs-header-content {
          max-width: 1400px;
          margin: 0 auto;
          padding: 0 2rem;
          display: flex;
          align-items: center;
          justify-content: space-between;
          gap: 2rem;
        }
        
        .docs-title h1 {
          margin: 0 0 0.25rem 0;
          color: var(--foreground1);
          font-size: 1.75rem;
          font-weight: 600;
          font-family: "Berkeley Mono", monospace;
        }
        
        .docs-title p {
          margin: 0;
          color: var(--foreground0);
          font-size: 0.875rem;
          opacity: 0.8;
        }
        
        .docs-search {
          position: relative;
        }
        
        .docs-search input {
          padding: 0.5rem 1rem;
          background: var(--background1);
          border: 1px solid var(--background2);
          border-radius: 6px;
          color: var(--foreground1);
          font-family: "Berkeley Mono", monospace;
          font-size: 0.875rem;
          width: 300px;
          transition: all 0.2s;
        }
        
        .docs-search input:focus {
          outline: none;
          border-color: var(--background3);
          background: var(--background2);
        }
        
        .docs-search input::placeholder {
          color: var(--foreground0);
          opacity: 0.5;
        }
        
        .docs-layout {
          display: flex;
          max-width: 1400px;
          margin: 0 auto;
          gap: 0;
          padding: 0;
        }
        
        .docs-sidebar {
          width: 300px;
          min-width: 300px;
          background: var(--background0);
          border-right: 1px solid var(--background2);
          height: calc(100vh - 8rem);
          overflow-y: auto;
        }
        
        .docs-menu {
          padding: 2rem 1.5rem;
        }
        
        .docs-menu-section {
          margin-bottom: 2.5rem;
        }
        
        .docs-menu-section:last-child {
          margin-bottom: 0;
        }
        
        .docs-menu-section h3 {
          margin: 0 0 1rem 0;
          color: var(--foreground1);
          font-size: 0.75rem;
          font-weight: 600;
          text-transform: uppercase;
          letter-spacing: 0.05em;
          opacity: 0.8;
          font-family: "Berkeley Mono", monospace;
        }
        
        .docs-menu-link {
          display: block;
          padding: 0.5rem 0.75rem;
          margin: 0 -0.75rem;
          color: var(--foreground0);
          text-decoration: none;
          font-size: 0.875rem;
          line-height: 1.5;
          transition: all 0.15s;
          border-radius: 4px;
          font-family: "Berkeley Mono", monospace;
        }
        
        .docs-menu-link:hover {
          color: var(--foreground1);
          background: var(--background1);
        }
        
        .docs-menu-link.active {
          color: var(--foreground1);
          background: var(--background2);
          font-weight: 500;
        }
        
        .docs-main {
          flex: 1;
          min-width: 0;
          background: var(--background0);
          overflow-y: auto;
          height: calc(100vh - 8rem);
        }
        
        .docs-content {
          padding: 3rem 4rem;
          max-width: 900px;
        }
        
        .welcome-section {
          margin-bottom: 4rem;
        }
        
        .welcome-section h2 {
          margin: 0 0 1.5rem 0;
          color: var(--foreground1);
          font-size: 1.25rem;
          font-weight: 600;
          font-family: "Berkeley Mono", monospace;
        }
        
        .lead {
          font-size: 1rem;
          line-height: 1.75;
          color: var(--foreground0);
          margin-bottom: 3rem;
          opacity: 0.9;
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
          border: 1px solid var(--background2);
          border-radius: 8px;
        }
        
        .quick-link-card:hover {
          background: var(--background2);
          border-color: var(--background3);
          transform: translateY(-2px);
        }
        
        .quick-link-card h3 {
          margin: 0 0 0.75rem 0;
          color: var(--foreground1);
          font-size: 1rem;
          font-weight: 600;
          font-family: "Berkeley Mono", monospace;
        }
        
        .quick-link-card p {
          margin: 0;
          color: var(--foreground0);
          font-size: 0.875rem;
          line-height: 1.5;
          opacity: 0.8;
        }
        
        .features-section {
          margin-bottom: 4rem;
        }
        
        .features-section h2 {
          margin: 0 0 2rem 0;
          color: var(--foreground1);
          font-size: 1.25rem;
          font-weight: 600;
          font-family: "Berkeley Mono", monospace;
        }
        
        .features-grid {
          display: grid;
          grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
          gap: 1.5rem;
        }
        
        .feature-item {
          padding: 2rem;
          background: var(--background1);
          border: 1px solid var(--background2);
          border-radius: 8px;
        }
        
        .feature-item h3 {
          margin: 0 0 1rem 0;
          color: var(--foreground1);
          font-size: 1rem;
          font-weight: 600;
          font-family: "Berkeley Mono", monospace;
        }
        
        .feature-item p {
          margin: 0;
          color: var(--foreground0);
          font-size: 0.875rem;
          line-height: 1.5;
          opacity: 0.8;
        }
        
        .code-example h2 {
          margin: 0 0 1.5rem 0;
          color: var(--foreground1);
          font-size: 1.25rem;
          font-weight: 600;
          font-family: "Berkeley Mono", monospace;
        }
        
        pre[is-="pre"] {
          font-family: "Berkeley Mono", monospace;
          font-size: 0.875rem;
          line-height: 1.6;
        }
        
        /* Dark theme adjustments */
        @media (prefers-color-scheme: dark) {
          .docs-header {
            background: var(--background0);
          }
          
          .docs-sidebar {
            background: var(--background0);
          }
          
          .docs-main {
            background: var(--background0);
          }
        }
        
        /* Mobile responsiveness */
        @media (max-width: 768px) {
          .docs-header-content {
            flex-direction: column;
            align-items: stretch;
            gap: 1rem;
            padding: 0 1rem;
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
            height: auto;
            border-right: none;
            border-bottom: 1px solid var(--background2);
          }
          
          .docs-main {
            height: auto;
          }
          
          .docs-content {
            padding: 2rem 1rem;
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
              <h1># 📚 Documentation</h1>
              <p>Learn to build Bitcoin-powered AI agents</p>
            </div>
            
            <div class="docs-search">
              <input type="text" placeholder="Search docs..." />
            </div>
          </div>
        </div>

        <!-- Docs Content -->
        <div class="docs-layout">
          <div class="docs-sidebar">
            <div class="docs-menu">
              <div class="docs-menu-section">
                <h3>### Getting Started</h3>
                <a href="/docs/getting-started" class="docs-menu-link ${slug === "getting-started" ? "active" : ""}">
                  Quick Start Guide
                </a>
              </div>
              
              <div class="docs-menu-section">
                <h3>### Core Concepts</h3>
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
                <h3>### API Reference</h3>
                <a href="/docs/api-reference" class="docs-menu-link ${slug === "api-reference" ? "active" : ""}">
                  Complete API Docs
                </a>
                <a href="/docs/sdk-examples" class="docs-menu-link">
                  SDK Examples
                </a>
              </div>
              
              <div class="docs-menu-section">
                <h3>### Support</h3>
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
            <article class="docs-content">
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
            background: var(--background0);
            border-bottom: 1px solid var(--background2);
            padding: 1.5rem 0;
          }
          
          .docs-header-content {
            max-width: 1400px;
            margin: 0 auto;
            padding: 0 2rem;
            display: flex;
            align-items: center;
            justify-content: space-between;
            gap: 2rem;
          }
          
          .docs-title h1 {
            margin: 0 0 0.25rem 0;
            color: var(--foreground1);
            font-size: 1.75rem;
            font-weight: 600;
            font-family: "Berkeley Mono", monospace;
          }
          
          .docs-title p {
            margin: 0;
            color: var(--foreground0);
            font-size: 0.875rem;
            opacity: 0.8;
          }
          
          .docs-search {
            position: relative;
          }
          
          .docs-search input {
            padding: 0.5rem 1rem;
            background: var(--background1);
            border: 1px solid var(--background2);
            border-radius: 6px;
            color: var(--foreground1);
            font-family: "Berkeley Mono", monospace;
            font-size: 0.875rem;
            width: 300px;
            transition: all 0.2s;
          }
          
          .docs-search input:focus {
            outline: none;
            border-color: var(--background3);
            background: var(--background2);
          }
          
          .docs-search input::placeholder {
            color: var(--foreground0);
            opacity: 0.5;
          }
          
          .docs-layout {
            display: flex;
            max-width: 1400px;
            margin: 0 auto;
            gap: 0;
            padding: 0;
          }
          
          .docs-sidebar {
            width: 300px;
            min-width: 300px;
            background: var(--background0);
            border-right: 1px solid var(--background2);
            height: calc(100vh - 8rem);
            overflow-y: auto;
          }
          
          .docs-menu {
            padding: 2rem 1.5rem;
          }
          
          .docs-menu-section {
            margin-bottom: 2.5rem;
          }
          
          .docs-menu-section:last-child {
            margin-bottom: 0;
          }
          
          .docs-menu-section h3 {
            margin: 0 0 1rem 0;
            color: var(--foreground1);
            font-size: 0.75rem;
            font-weight: 600;
            text-transform: uppercase;
            letter-spacing: 0.05em;
            opacity: 0.8;
            font-family: "Berkeley Mono", monospace;
          }
          
          .docs-menu-link {
            display: block;
            padding: 0.5rem 0.75rem;
            margin: 0 -0.75rem;
            color: var(--foreground0);
            text-decoration: none;
            font-size: 0.875rem;
            line-height: 1.5;
            transition: all 0.15s;
            border-radius: 4px;
            font-family: "Berkeley Mono", monospace;
          }
          
          .docs-menu-link:hover {
            color: var(--foreground1);
            background: var(--background1);
          }
          
          .docs-menu-link.active {
            color: var(--foreground1);
            background: var(--background2);
            font-weight: 500;
          }
          
          .docs-main {
            flex: 1;
            min-width: 0;
            background: var(--background0);
            overflow-y: auto;
            height: calc(100vh - 8rem);
          }
          
          .docs-content {
            padding: 3rem 4rem;
            max-width: 900px;
          }
          
          .doc-header h1 {
            margin: 0 0 1rem 0;
            color: var(--foreground1);
            font-size: 2.25rem;
            font-weight: 700;
            font-family: "Berkeley Mono", monospace;
          }
          
          .doc-summary {
            font-size: 1.125rem;
            line-height: 1.75;
            color: var(--foreground0);
            margin-bottom: 2.5rem;
            padding-bottom: 2.5rem;
            border-bottom: 1px solid var(--background2);
            opacity: 0.9;
          }
          
          .doc-body {
            line-height: 1.75;
            color: var(--foreground1);
            font-family: var(--font-family);
          }
          
          .doc-body h1,
          .doc-body h2,
          .doc-body h3,
          .doc-body h4,
          .doc-body h5,
          .doc-body h6 {
            color: var(--foreground1);
            margin-top: 2.5rem;
            margin-bottom: 1rem;
            font-family: "Berkeley Mono", monospace;
            font-weight: 600;
          }
          
          .doc-body h1 {
            font-size: 1.875rem;
            border-bottom: 1px solid var(--background2);
            padding-bottom: 0.75rem;
          }
          
          .doc-body h2 {
            font-size: 1.5rem;
          }
          
          .doc-body h3 {
            font-size: 1.25rem;
          }
          
          .doc-body p {
            margin-bottom: 1.25rem;
            color: var(--foreground0);
            opacity: 0.9;
          }
          
          .doc-body ul,
          .doc-body ol {
            margin-bottom: 1.25rem;
            color: var(--foreground0);
            padding-left: 1.75rem;
            opacity: 0.9;
          }
          
          .doc-body li {
            margin-bottom: 0.5rem;
            line-height: 1.6;
          }
          
          .doc-body code {
            background: var(--background1);
            padding: 0.125rem 0.375rem;
            border-radius: 4px;
            font-family: "Berkeley Mono", monospace;
            font-size: 0.875em;
            color: var(--foreground1);
            border: 1px solid var(--background2);
          }
          
          .doc-body pre {
            background: var(--background1);
            padding: 1.5rem;
            border-radius: 8px;
            overflow-x: auto;
            margin-bottom: 1.5rem;
            border: 1px solid var(--background2);
          }
          
          .doc-body pre code {
            background: none;
            padding: 0;
            color: var(--foreground1);
            border: none;
            font-size: 0.875rem;
            line-height: 1.6;
          }
          
          .doc-body blockquote {
            border-left: 3px solid var(--background3);
            padding-left: 1.5rem;
            margin: 1.5rem 0;
            color: var(--foreground0);
            font-style: italic;
            opacity: 0.85;
          }
          
          .doc-body a {
            color: var(--foreground1);
            text-decoration: underline;
            text-decoration-color: var(--background3);
            text-underline-offset: 2px;
            transition: all 0.2s;
          }
          
          .doc-body a:hover {
            color: var(--foreground2);
            text-decoration-color: var(--foreground1);
          }
          
          .doc-footer {
            margin-top: 4rem;
            padding-top: 2.5rem;
            border-top: 1px solid var(--background2);
          }
          
          .doc-nav-buttons {
            display: flex;
            justify-content: space-between;
            gap: 1.5rem;
            flex-wrap: wrap;
          }
          
          .doc-nav-button {
            text-decoration: none;
            font-family: "Berkeley Mono", monospace;
            font-size: 0.875rem;
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
