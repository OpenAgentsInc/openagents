import { css, document, html } from "@openagentsinc/psionic"
import { baseStyles } from "../styles"

// Mock agent data based on v1 structure
const featuredAgents = [
  {
    id: "bitcoin-analyst",
    name: "Bitcoin Analyst",
    about: "Expert analyst providing Bitcoin market insights, technical analysis, and trading strategies.",
    image: "🟠",
    model: "llama-3.3-70b",
    sats_per_message: 100,
    thread_count: 2847,
    unique_users_count: 423,
    is_featured: true,
    greeting: "I'm a Bitcoin market analyst ready to help you understand crypto markets!",
    creator: "satoshi_trader"
  },
  {
    id: "code-mentor",
    name: "Code Mentor",
    about: "Senior software engineer specialized in code reviews, architecture guidance, and best practices.",
    image: "💻",
    model: "llama-3.3-70b",
    sats_per_message: 150,
    thread_count: 1923,
    unique_users_count: 312,
    is_featured: true,
    greeting: "Ready to help you write better code and solve complex programming challenges!",
    creator: "dev_master"
  },
  {
    id: "lightning-dev",
    name: "Lightning Developer",
    about: "Lightning Network specialist helping with payments, channels, and L2 development.",
    image: "⚡",
    model: "llama-3.3-70b",
    sats_per_message: 200,
    thread_count: 1456,
    unique_users_count: 289,
    is_featured: true,
    greeting: "Let's build the future of Bitcoin payments together!",
    creator: "ln_builder"
  }
]

const popularAgents = [
  {
    id: "ai-researcher",
    name: "AI Researcher",
    about: "Deep learning expert focused on transformer architectures and AI safety research.",
    image: "🤖",
    model: "llama-3.3-70b",
    sats_per_message: 175,
    thread_count: 3241,
    unique_users_count: 678,
    is_featured: false,
    greeting: "Let's explore the frontiers of artificial intelligence together!",
    creator: "ai_pioneer"
  },
  {
    id: "nostr-guide",
    name: "Nostr Guide",
    about: "Nostr protocol expert helping developers build decentralized social applications.",
    image: "🟣",
    model: "llama-3.3-70b",
    sats_per_message: 125,
    thread_count: 2156,
    unique_users_count: 445,
    is_featured: false,
    greeting: "Ready to help you navigate the decentralized social web!",
    creator: "nostr_dev"
  },
  {
    id: "rust-expert",
    name: "Rust Expert",
    about: "Systems programming specialist with deep Rust expertise for performance-critical applications.",
    image: "🦀",
    model: "llama-3.3-70b",
    sats_per_message: 180,
    thread_count: 1834,
    unique_users_count: 356,
    is_featured: false,
    greeting: "Let's write safe, fast, and reliable systems code together!",
    creator: "rust_dev"
  }
]

const storeStyles = css`
  /* V1 Color Palette */
  :root {
    --text: #D7D8E5;
    --offblack: #1e1e1e;
    --darkgray: #3D3D40;
    --gray: #8B8585;
    --lightgray: #A7A7A7;
    --white: #fff;
    --black: #000000;
    --input-border: #3D3E42;
    --placeholder: #777A81;
    --active-thread: #262626;
    --sidebar-border: rgba(255, 255, 255, 0.15);
  }

  body {
    background-color: var(--black) !important;
    color: var(--white) !important;
    font-family: "Berkeley Mono", "JetBrains Mono", ui-monospace, monospace !important;
  }

  .store-container {
    max-width: 1200px;
    margin: 0 auto;
    padding: 2rem;
    padding-top: 5rem; /* Account for fixed header */
  }

  .store-header {
    text-align: center;
    margin-bottom: 3rem;
  }

  .store-title {
    font-size: 3rem;
    color: var(--white);
    margin-bottom: 1rem;
  }

  .store-subtitle {
    font-size: 1.2rem;
    color: var(--gray);
    margin-bottom: 2rem;
  }

  .search-bar {
    max-width: 600px;
    margin: 0 auto 3rem;
    position: relative;
  }

  .search-input {
    width: 100%;
    padding: 1rem 3rem 1rem 1rem;
    background: var(--offblack);
    border: 2px solid var(--darkgray);
    border-radius: 8px;
    color: var(--white);
    font-size: 1rem;
    font-family: inherit;
    transition: border-color 0.3s ease;
  }

  .search-input:focus {
    outline: none;
    border-color: var(--white);
  }

  .search-input::placeholder {
    color: var(--placeholder);
  }

  .search-icon {
    position: absolute;
    right: 1rem;
    top: 50%;
    transform: translateY(-50%);
    color: var(--gray);
    width: 20px;
    height: 20px;
  }

  .section {
    margin-bottom: 4rem;
  }

  .section-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 2rem;
  }

  .section-title {
    font-size: 1.8rem;
    color: var(--white);
    margin: 0;
  }

  .section-badge {
    background: var(--darkgray);
    color: var(--white);
    padding: 0.25rem 0.75rem;
    border-radius: 12px;
    font-size: 0.875rem;
    font-weight: 600;
  }

  .agents-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(380px, 1fr));
    gap: 2rem;
  }

  .agent-card {
    background: var(--offblack);
    border: 1px solid var(--darkgray);
    border-radius: 12px;
    padding: 2rem;
    transition: all 0.3s ease;
    cursor: pointer;
  }

  .agent-card:hover {
    background: #2a2a2a;
    border-color: var(--gray);
    transform: translateY(-2px);
  }

  .agent-header {
    display: flex;
    align-items: center;
    gap: 1rem;
    margin-bottom: 1rem;
  }

  .agent-avatar {
    width: 48px;
    height: 48px;
    background: var(--darkgray);
    border-radius: 8px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 1.5rem;
  }

  .agent-info h3 {
    margin: 0;
    font-size: 1.2rem;
    color: var(--white);
  }

  .agent-creator {
    margin: 0;
    font-size: 0.875rem;
    color: var(--gray);
  }

  .agent-description {
    color: var(--text);
    line-height: 1.6;
    margin-bottom: 1.5rem;
  }

  .agent-stats {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 1.5rem;
    padding: 1rem;
    background: var(--black);
    border-radius: 8px;
    font-size: 0.875rem;
  }

  .stat {
    text-align: center;
  }

  .stat-value {
    display: block;
    color: var(--white);
    font-weight: 600;
    font-size: 1rem;
  }

  .stat-label {
    color: var(--gray);
  }

  .agent-pricing {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 1.5rem;
  }

  .price {
    font-size: 1.1rem;
    color: var(--white);
    font-weight: 600;
  }

  .price-unit {
    color: var(--gray);
    font-size: 0.875rem;
  }

  .featured-badge {
    background: linear-gradient(45deg, #f59e0b, #d97706);
    color: white;
    padding: 0.25rem 0.5rem;
    border-radius: 4px;
    font-size: 0.75rem;
    font-weight: 600;
    text-transform: uppercase;
  }

  .agent-actions {
    display: flex;
    gap: 1rem;
  }

  .btn {
    padding: 0.75rem 1.5rem;
    border-radius: 6px;
    font-family: inherit;
    font-size: 0.875rem;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.2s ease;
    text-decoration: none;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border: none;
  }

  .btn-primary {
    background: var(--white);
    color: var(--black);
  }

  .btn-primary:hover {
    background: var(--lightgray);
  }

  .btn-secondary {
    background: transparent;
    color: var(--white);
    border: 1px solid var(--darkgray);
  }

  .btn-secondary:hover {
    background: var(--darkgray);
  }

  .empty-state {
    text-align: center;
    padding: 4rem 2rem;
    color: var(--gray);
  }

  .empty-state h3 {
    color: var(--white);
    margin-bottom: 1rem;
  }

  /* Responsive */
  @media (max-width: 768px) {
    .store-container {
      padding: 1rem;
      padding-top: 4rem;
    }

    .store-title {
      font-size: 2rem;
    }

    .agents-grid {
      grid-template-columns: 1fr;
      gap: 1.5rem;
    }

    .agent-card {
      padding: 1.5rem;
    }

    .agent-stats {
      flex-direction: column;
      gap: 1rem;
    }

    .agent-actions {
      flex-direction: column;
    }
  }
`

function renderAgentCard(agent: typeof featuredAgents[0]) {
  const popularity = agent.unique_users_count * 3 + agent.thread_count

  return html`
    <div class="agent-card" onclick="window.location.href='/agents/${agent.id}'">
      <div class="agent-header">
        <div class="agent-avatar">${agent.image}</div>
        <div class="agent-info">
          <h3>${agent.name}</h3>
          <p class="agent-creator">by @${agent.creator}</p>
        </div>
        ${agent.is_featured ? html`<div class="featured-badge">Featured</div>` : ""}
      </div>
      
      <p class="agent-description">${agent.about}</p>
      
      <div class="agent-stats">
        <div class="stat">
          <span class="stat-value">${agent.thread_count.toLocaleString()}</span>
          <span class="stat-label">Chats</span>
        </div>
        <div class="stat">
          <span class="stat-value">${agent.unique_users_count.toLocaleString()}</span>
          <span class="stat-label">Users</span>
        </div>
        <div class="stat">
          <span class="stat-value">${Math.round(popularity / 1000)}k</span>
          <span class="stat-label">Score</span>
        </div>
      </div>
      
      <div class="agent-pricing">
        <div class="price">
          ${agent.sats_per_message} <span class="price-unit">sats/message</span>
        </div>
        <div style="color: var(--gray); font-size: 0.875rem;">${agent.model}</div>
      </div>
      
      <div class="agent-actions">
        <a href="/chat/new?agent=${agent.id}" class="btn btn-primary">
          💬 Start Chat
        </a>
        <a href="/agents/${agent.id}" class="btn btn-secondary">
          👁️ View Details
        </a>
      </div>
    </div>
  `
}

export async function store() {
  return document({
    title: "Agent Store - OpenAgents",
    styles: baseStyles + storeStyles,
    body: html`
      <!-- Fixed Header -->
      <div style="position: fixed; top: 0; left: 0; right: 0; height: 52px; display: flex; align-items: center; justify-content: space-between; padding: 0 20px; z-index: 20; background: black; border-bottom: 1px solid var(--darkgray);">
        <div style="display: flex; align-items: center; gap: 20px;">
          <a href="/" style="color: white; text-decoration: none; font-size: 18px; font-weight: 600;">OpenAgents</a>
          <nav style="display: flex; gap: 20px;">
            <a href="/agents" style="color: var(--gray); text-decoration: none; font-size: 14px;">My Agents</a>
            <a href="/store" style="color: var(--white); text-decoration: none; font-size: 14px;">Store</a>
            <a href="/create" style="color: var(--gray); text-decoration: none; font-size: 14px;">Create</a>
          </nav>
        </div>
        <a href="/settings" style="color: var(--gray); text-decoration: none; font-size: 14px;">Settings</a>
      </div>

      <div class="store-container">
        <!-- Store Header -->
        <div class="store-header">
          <h1 class="store-title">🤖 Agent Store</h1>
          <p class="store-subtitle">Discover and chat with specialized AI agents built by the community</p>
          
          <!-- Search Bar -->
          <div class="search-bar">
            <input type="text" class="search-input" placeholder="Search agents by name, description, or capabilities..." id="search-input">
            <svg class="search-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <circle cx="11" cy="11" r="8"></circle>
              <path d="m21 21-4.35-4.35"></path>
            </svg>
          </div>
        </div>

        <!-- Featured Agents Section -->
        <section class="section">
          <div class="section-header">
            <h2 class="section-title">⭐ Featured Agents</h2>
            <div class="section-badge">${featuredAgents.length} agents</div>
          </div>
          <div class="agents-grid" id="featured-grid">
            ${featuredAgents.map((agent) => renderAgentCard(agent)).join("")}
          </div>
        </section>

        <!-- Popular Agents Section -->
        <section class="section">
          <div class="section-header">
            <h2 class="section-title">🔥 Popular Agents</h2>
            <div class="section-badge">${popularAgents.length} agents</div>
          </div>
          <div class="agents-grid" id="popular-grid">
            ${popularAgents.map((agent) => renderAgentCard(agent)).join("")}
          </div>
        </section>

        <!-- Empty Search Results -->
        <div class="empty-state" id="empty-results" style="display: none;">
          <h3>No agents found</h3>
          <p>Try adjusting your search terms or browse the featured and popular agents above.</p>
        </div>
      </div>

      <script>
        // Search functionality
        const searchInput = document.getElementById('search-input')
        const featuredGrid = document.getElementById('featured-grid')
        const popularGrid = document.getElementById('popular-grid')
        const emptyResults = document.getElementById('empty-results')
        
        // All agents for search
        const allAgents = ${JSON.stringify([...featuredAgents, ...popularAgents])}
        
        function renderAgentCard(agent) {
          const popularity = agent.unique_users_count * 3 + agent.thread_count
          const featuredBadge = agent.is_featured ? '<div class="featured-badge">Featured</div>' : ''
          
          return \`
            <div class="agent-card" onclick="window.location.href='/agents/\${agent.id}'">
              <div class="agent-header">
                <div class="agent-avatar">\${agent.image}</div>
                <div class="agent-info">
                  <h3>\${agent.name}</h3>
                  <p class="agent-creator">by @\${agent.creator}</p>
                </div>
                \${featuredBadge}
              </div>
              
              <p class="agent-description">\${agent.about}</p>
              
              <div class="agent-stats">
                <div class="stat">
                  <span class="stat-value">\${agent.thread_count.toLocaleString()}</span>
                  <span class="stat-label">Chats</span>
                </div>
                <div class="stat">
                  <span class="stat-value">\${agent.unique_users_count.toLocaleString()}</span>
                  <span class="stat-label">Users</span>
                </div>
                <div class="stat">
                  <span class="stat-value">\${Math.round(popularity / 1000)}k</span>
                  <span class="stat-label">Score</span>
                </div>
              </div>
              
              <div class="agent-pricing">
                <div class="price">
                  \${agent.sats_per_message} <span class="price-unit">sats/message</span>
                </div>
                <div style="color: var(--gray); font-size: 0.875rem;">\${agent.model}</div>
              </div>
              
              <div class="agent-actions">
                <a href="/chat/new?agent=\${agent.id}" class="btn btn-primary" onclick="event.stopPropagation()">
                  💬 Start Chat
                </a>
                <a href="/agents/\${agent.id}" class="btn btn-secondary" onclick="event.stopPropagation()">
                  👁️ View Details
                </a>
              </div>
            </div>
          \`
        }
        
        function performSearch(query) {
          if (!query.trim()) {
            // Show original sections
            document.querySelector('.section:nth-of-type(1)').style.display = 'block'
            document.querySelector('.section:nth-of-type(2)').style.display = 'block'
            emptyResults.style.display = 'none'
            return
          }
          
          const searchTerm = query.toLowerCase()
          const filteredAgents = allAgents.filter(agent => 
            agent.name.toLowerCase().includes(searchTerm) ||
            agent.about.toLowerCase().includes(searchTerm) ||
            agent.creator.toLowerCase().includes(searchTerm)
          )
          
          // Hide sections
          document.querySelector('.section:nth-of-type(1)').style.display = 'none'
          document.querySelector('.section:nth-of-type(2)').style.display = 'none'
          
          if (filteredAgents.length === 0) {
            emptyResults.style.display = 'block'
          } else {
            emptyResults.style.display = 'none'
            
            // Create search results section
            let searchResultsSection = document.getElementById('search-results')
            if (!searchResultsSection) {
              searchResultsSection = document.createElement('section')
              searchResultsSection.className = 'section'
              searchResultsSection.id = 'search-results'
              document.querySelector('.store-container').appendChild(searchResultsSection)
            }
            
            searchResultsSection.innerHTML = \`
              <div class="section-header">
                <h2 class="section-title">🔍 Search Results</h2>
                <div class="section-badge">\${filteredAgents.length} agents</div>
              </div>
              <div class="agents-grid">
                \${filteredAgents.map(agent => renderAgentCard(agent)).join('')}
              </div>
            \`
            searchResultsSection.style.display = 'block'
          }
        }
        
        // Search input handler
        searchInput.addEventListener('input', (e) => {
          performSearch(e.target.value)
        })
        
        // Clear search on escape
        searchInput.addEventListener('keydown', (e) => {
          if (e.key === 'Escape') {
            searchInput.value = ''
            performSearch('')
          }
        })
      </script>
    `
  })
}
