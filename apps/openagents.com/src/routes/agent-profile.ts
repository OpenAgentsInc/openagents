import { css, document, html } from "@openagentsinc/psionic"
import { baseStyles } from "../styles"

// Mock agent data - in real implementation this would come from database
const mockAgents = {
  "bitcoin-analyst": {
    id: "bitcoin-analyst",
    name: "Bitcoin Analyst",
    about:
      "Expert analyst providing Bitcoin market insights, technical analysis, and trading strategies. I stay up-to-date with the latest market trends, on-chain metrics, and macroeconomic factors affecting Bitcoin's price.",
    image: "🟠",
    model: "llama-3.3-70b",
    sats_per_message: 100,
    thread_count: 2847,
    unique_users_count: 423,
    is_featured: true,
    greeting:
      "Hi! I'm a Bitcoin market analyst ready to help you understand crypto markets and make informed trading decisions. What would you like to know?",
    creator: "satoshi_trader",
    created_at: "2024-01-15",
    last_active: "2 hours ago",
    rating: 4.8,
    total_earned: 284700,
    capabilities: [
      "Market Analysis",
      "Technical Analysis",
      "Price Predictions",
      "On-chain Analysis",
      "Trading Strategies"
    ],
    sample_conversations: [
      {
        user: "What do you think about Bitcoin's current price action?",
        agent:
          "Based on current on-chain metrics and technical analysis, Bitcoin is showing signs of consolidation around key support levels. The 200-day moving average is holding strong, and we're seeing decreased selling pressure from long-term holders..."
      },
      {
        user: "Should I buy Bitcoin now or wait?",
        agent:
          "I can't provide financial advice, but I can share what the data shows. Current metrics suggest we're in a period of accumulation, with institutional buying continuing despite short-term volatility. Key factors to consider..."
      }
    ]
  },
  "code-mentor": {
    id: "code-mentor",
    name: "Code Mentor",
    about:
      "Senior software engineer with 10+ years experience in full-stack development. I specialize in code reviews, architecture guidance, debugging, and best practices across multiple programming languages.",
    image: "💻",
    model: "llama-3.3-70b",
    sats_per_message: 150,
    thread_count: 1923,
    unique_users_count: 312,
    is_featured: true,
    greeting: "Ready to help you write better code and solve complex programming challenges! What are you working on?",
    creator: "dev_master",
    created_at: "2024-02-03",
    last_active: "1 hour ago",
    rating: 4.9,
    total_earned: 288450,
    capabilities: [
      "Code Review",
      "Architecture Design",
      "Debugging",
      "Performance Optimization",
      "Best Practices"
    ],
    sample_conversations: [
      {
        user: "Can you review this React component for performance issues?",
        agent:
          "I'd be happy to review your React component! Please share the code and I'll look for performance bottlenecks, unnecessary re-renders, and optimization opportunities..."
      },
      {
        user: "What's the best way to structure a microservices architecture?",
        agent:
          "Great question! Microservices architecture design depends on your specific requirements, but here are the key principles I always recommend starting with..."
      }
    ]
  }
}

const agentProfileStyles = css`
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
    --success: #10b981;
    --warning: #f59e0b;
    --error: #ef4444;
  }

  body {
    background-color: var(--black) !important;
    color: var(--white) !important;
    font-family: "Berkeley Mono", "JetBrains Mono", ui-monospace, monospace !important;
  }

  .profile-container {
    max-width: 1000px;
    margin: 0 auto;
    padding: 2rem;
    padding-top: 5rem;
  }

  .agent-header {
    background: var(--offblack);
    border: 1px solid var(--darkgray);
    border-radius: 12px;
    padding: 2.5rem;
    margin-bottom: 2rem;
    display: flex;
    gap: 2rem;
    align-items: flex-start;
  }

  .agent-avatar {
    width: 80px;
    height: 80px;
    background: var(--darkgray);
    border-radius: 12px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 2.5rem;
    flex-shrink: 0;
  }

  .agent-info {
    flex: 1;
  }

  .agent-name {
    font-size: 2rem;
    color: var(--white);
    margin: 0 0 0.5rem 0;
    display: flex;
    align-items: center;
    gap: 1rem;
  }

  .featured-badge {
    background: linear-gradient(45deg, #f59e0b, #d97706);
    color: white;
    padding: 0.25rem 0.75rem;
    border-radius: 6px;
    font-size: 0.75rem;
    font-weight: 600;
    text-transform: uppercase;
  }

  .agent-creator {
    color: var(--gray);
    margin: 0 0 1rem 0;
    font-size: 0.875rem;
  }

  .agent-description {
    color: var(--text);
    line-height: 1.6;
    margin-bottom: 1.5rem;
    font-size: 1rem;
  }

  .agent-stats {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(120px, 1fr));
    gap: 1.5rem;
    margin-bottom: 1.5rem;
  }

  .stat {
    text-align: center;
    background: var(--black);
    border: 1px solid var(--darkgray);
    border-radius: 8px;
    padding: 1rem 0.5rem;
  }

  .stat-value {
    display: block;
    color: var(--white);
    font-weight: 600;
    font-size: 1.25rem;
    margin-bottom: 0.25rem;
  }

  .stat-label {
    color: var(--gray);
    font-size: 0.75rem;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .agent-actions {
    display: flex;
    gap: 1rem;
  }

  .btn {
    padding: 0.875rem 1.75rem;
    border-radius: 8px;
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
    gap: 0.5rem;
  }

  .btn-primary {
    background: var(--white);
    color: var(--black);
  }

  .btn-primary:hover {
    background: var(--lightgray);
    transform: translateY(-1px);
  }

  .btn-secondary {
    background: transparent;
    color: var(--white);
    border: 2px solid var(--darkgray);
  }

  .btn-secondary:hover {
    background: var(--darkgray);
    border-color: var(--gray);
  }

  .content-grid {
    display: grid;
    grid-template-columns: 2fr 1fr;
    gap: 2rem;
    margin-bottom: 2rem;
  }

  .main-content {
    display: flex;
    flex-direction: column;
    gap: 2rem;
  }

  .sidebar-content {
    display: flex;
    flex-direction: column;
    gap: 2rem;
  }

  .section {
    background: var(--offblack);
    border: 1px solid var(--darkgray);
    border-radius: 12px;
    padding: 2rem;
  }

  .section-title {
    font-size: 1.2rem;
    color: var(--white);
    margin: 0 0 1.5rem 0;
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .capabilities-list {
    display: flex;
    flex-wrap: wrap;
    gap: 0.75rem;
  }

  .capability-tag {
    background: var(--darkgray);
    color: var(--white);
    padding: 0.5rem 1rem;
    border-radius: 20px;
    font-size: 0.875rem;
    font-weight: 500;
  }

  .conversation-sample {
    background: var(--black);
    border: 1px solid var(--darkgray);
    border-radius: 8px;
    padding: 1.5rem;
    margin-bottom: 1.5rem;
  }

  .conversation-sample:last-child {
    margin-bottom: 0;
  }

  .message {
    margin-bottom: 1rem;
  }

  .message:last-child {
    margin-bottom: 0;
  }

  .message-label {
    font-size: 0.75rem;
    color: var(--gray);
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    margin-bottom: 0.5rem;
  }

  .message-content {
    color: var(--text);
    line-height: 1.5;
    font-size: 0.875rem;
  }

  .pricing-card {
    background: var(--black);
    border: 2px solid var(--darkgray);
    border-radius: 8px;
    padding: 1.5rem;
    text-align: center;
  }

  .price-amount {
    font-size: 2rem;
    color: var(--white);
    font-weight: 600;
    margin-bottom: 0.5rem;
  }

  .price-unit {
    color: var(--gray);
    font-size: 0.875rem;
    margin-bottom: 1rem;
  }

  .price-description {
    color: var(--text);
    font-size: 0.875rem;
    line-height: 1.4;
    margin-bottom: 1.5rem;
  }

  .agent-details {
    font-size: 0.875rem;
  }

  .detail-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.75rem 0;
    border-bottom: 1px solid var(--darkgray);
  }

  .detail-row:last-child {
    border-bottom: none;
  }

  .detail-label {
    color: var(--gray);
  }

  .detail-value {
    color: var(--white);
    font-weight: 500;
  }

  .rating-display {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .stars {
    color: #f59e0b;
  }

  .back-link {
    display: inline-flex;
    align-items: center;
    gap: 0.5rem;
    color: var(--gray);
    text-decoration: none;
    font-size: 0.875rem;
    margin-bottom: 2rem;
    transition: color 0.2s ease;
  }

  .back-link:hover {
    color: var(--white);
  }

  /* Responsive */
  @media (max-width: 768px) {
    .profile-container {
      padding: 1rem;
      padding-top: 4rem;
    }

    .agent-header {
      flex-direction: column;
      align-items: center;
      text-align: center;
      padding: 2rem;
    }

    .agent-stats {
      grid-template-columns: repeat(2, 1fr);
    }

    .content-grid {
      grid-template-columns: 1fr;
    }

    .agent-actions {
      flex-direction: column;
      width: 100%;
    }

    .agent-name {
      font-size: 1.5rem;
      flex-direction: column;
      text-align: center;
      gap: 0.5rem;
    }
  }
`

export async function agentProfile(ctx: { params: { id: string } }) {
  const agentId = ctx.params.id
  const agent = mockAgents[agentId as keyof typeof mockAgents]

  if (!agent) {
    return document({
      title: "Agent Not Found - OpenAgents",
      styles: baseStyles + agentProfileStyles,
      body: html`
        <div style="display: flex; height: 100vh; align-items: center; justify-content: center; background: black; color: white;">
          <div style="text-align: center;">
            <h1>Agent Not Found</h1>
            <p>The agent "${agentId}" could not be found.</p>
            <a href="/store" style="color: white; text-decoration: underline;">← Back to Store</a>
          </div>
        </div>
      `
    })
  }

  const popularity = agent.unique_users_count * 3 + agent.thread_count
  const rating = agent.rating || 4.5

  return document({
    title: `${agent.name} - OpenAgents`,
    styles: baseStyles + agentProfileStyles,
    body: html`
      <!-- Fixed Header -->
      <div style="position: fixed; top: 0; left: 0; right: 0; height: 52px; display: flex; align-items: center; justify-content: space-between; padding: 0 20px; z-index: 20; background: black; border-bottom: 1px solid var(--darkgray);">
        <div style="display: flex; align-items: center; gap: 20px;">
          <a href="/" style="color: white; text-decoration: none; font-size: 18px; font-weight: 600;">OpenAgents</a>
          <nav style="display: flex; gap: 20px;">
            <a href="/agents" style="color: var(--gray); text-decoration: none; font-size: 14px;">My Agents</a>
            <a href="/store" style="color: var(--gray); text-decoration: none; font-size: 14px;">Store</a>
            <a href="/create" style="color: var(--gray); text-decoration: none; font-size: 14px;">Create</a>
          </nav>
        </div>
        <a href="/settings" style="color: var(--gray); text-decoration: none; font-size: 14px;">Settings</a>
      </div>

      <div class="profile-container">
        <!-- Back Link -->
        <a href="/store" class="back-link">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="m15 18-6-6 6-6"/>
          </svg>
          Back to Store
        </a>

        <!-- Agent Header -->
        <div class="agent-header">
          <div class="agent-avatar">${agent.image}</div>
          
          <div class="agent-info">
            <h1 class="agent-name">
              ${agent.name}
              ${agent.is_featured ? html`<span class="featured-badge">Featured</span>` : ""}
            </h1>
            <p class="agent-creator">Created by @${agent.creator} • ${agent.created_at}</p>
            <p class="agent-description">${agent.about}</p>
            
            <div class="agent-stats">
              <div class="stat">
                <span class="stat-value">${agent.thread_count.toLocaleString()}</span>
                <span class="stat-label">Total Chats</span>
              </div>
              <div class="stat">
                <span class="stat-value">${agent.unique_users_count.toLocaleString()}</span>
                <span class="stat-label">Users</span>
              </div>
              <div class="stat">
                <span class="stat-value">${Math.round(popularity / 1000)}k</span>
                <span class="stat-label">Popularity</span>
              </div>
              <div class="stat">
                <span class="stat-value">${rating.toFixed(1)} ⭐</span>
                <span class="stat-label">Rating</span>
              </div>
              <div class="stat">
                <span class="stat-value">${(agent.total_earned / 1000).toFixed(0)}k</span>
                <span class="stat-label">Sats Earned</span>
              </div>
            </div>

            <div class="agent-actions">
              <a href="/chat/new?agent=${agent.id}" class="btn btn-primary">
                💬 Start Chat
              </a>
              <button class="btn btn-secondary" onclick="shareAgent()">
                📤 Share
              </button>
            </div>
          </div>
        </div>

        <!-- Content Grid -->
        <div class="content-grid">
          <!-- Main Content -->
          <div class="main-content">
            <!-- Capabilities -->
            <div class="section">
              <h3 class="section-title">
                🎯 Capabilities
              </h3>
              <div class="capabilities-list">
                ${
      agent.capabilities.map((cap) =>
        html`
                  <span class="capability-tag">${cap}</span>
                `
      ).join("")
    }
              </div>
            </div>

            <!-- Sample Conversations -->
            <div class="section">
              <h3 class="section-title">
                💬 Sample Conversations
              </h3>
              ${
      agent.sample_conversations.map((conv) =>
        html`
                <div class="conversation-sample">
                  <div class="message">
                    <div class="message-label">👤 User</div>
                    <div class="message-content">${conv.user}</div>
                  </div>
                  <div class="message">
                    <div class="message-label">🤖 ${agent.name}</div>
                    <div class="message-content">${conv.agent}</div>
                  </div>
                </div>
              `
      ).join("")
    }
            </div>
          </div>

          <!-- Sidebar -->
          <div class="sidebar-content">
            <!-- Pricing -->
            <div class="section">
              <h3 class="section-title">
                💰 Pricing
              </h3>
              <div class="pricing-card">
                <div class="price-amount">${agent.sats_per_message}</div>
                <div class="price-unit">sats per message</div>
                <div class="price-description">
                  Pay only for what you use. Each message is processed individually.
                </div>
                <a href="/chat/new?agent=${agent.id}" class="btn btn-primary" style="width: 100%;">
                  Start Chatting
                </a>
              </div>
            </div>

            <!-- Agent Details -->
            <div class="section">
              <h3 class="section-title">
                ℹ️ Details
              </h3>
              <div class="agent-details">
                <div class="detail-row">
                  <span class="detail-label">Model</span>
                  <span class="detail-value">${agent.model}</span>
                </div>
                <div class="detail-row">
                  <span class="detail-label">Last Active</span>
                  <span class="detail-value">${agent.last_active}</span>
                </div>
                <div class="detail-row">
                  <span class="detail-label">Created</span>
                  <span class="detail-value">${agent.created_at}</span>
                </div>
                <div class="detail-row">
                  <span class="detail-label">Rating</span>
                  <span class="detail-value">
                    <div class="rating-display">
                      <span class="stars">⭐⭐⭐⭐⭐</span>
                      <span>${rating.toFixed(1)}</span>
                    </div>
                  </span>
                </div>
                <div class="detail-row">
                  <span class="detail-label">Creator</span>
                  <span class="detail-value">@${agent.creator}</span>
                </div>
              </div>
            </div>

            <!-- Greeting Preview -->
            <div class="section">
              <h3 class="section-title">
                👋 Greeting
              </h3>
              <div style="background: var(--black); border: 1px solid var(--darkgray); border-radius: 8px; padding: 1.5rem;">
                <div class="message-label" style="margin-bottom: 0.5rem;">🤖 ${agent.name}</div>
                <div class="message-content">${agent.greeting}</div>
              </div>
            </div>
          </div>
        </div>
      </div>

      <script>
        function shareAgent() {
          if (navigator.share) {
            navigator.share({
              title: '${agent.name} - OpenAgents',
              text: '${agent.about}',
              url: window.location.href
            })
          } else {
            // Fallback to clipboard
            navigator.clipboard.writeText(window.location.href).then(() => {
              alert('Agent link copied to clipboard!')
            })
          }
        }
      </script>
    `
  })
}
