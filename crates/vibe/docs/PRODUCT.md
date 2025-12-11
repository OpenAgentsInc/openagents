# Vibe Product Specification

**The AI-native development platform for building products at the speed of thought.**

---

## Vision

Vibe is the development environment for the age of AI. Describe what you want to build, and Vibe builds it. Not mockups. Not wireframes. Real, working, deployable applications with authentication, databases, payments, and polished UI—all from natural language.

**Our thesis:** The future of software development isn't writing code line by line. It's describing intent and iterating on working software. Vibe makes this real.

---

## The $100M Platform

### Market Opportunity

The AI-assisted development market is exploding. Platforms in this space have achieved:
- $9.9B valuations for professional developer tools
- $1.8B valuations for no-code AI builders
- $40M+ ARR within 18 months of launch
- 500,000+ users building applications

**Vibe's opportunity:** No platform has combined:
1. Native-first performance with browser accessibility
2. Decentralized identity (Nostr) with mainstream UX
3. Bitcoin-native payments with traditional pricing
4. Multi-agent orchestration with single-user simplicity
5. Full code ownership with zero-config deployment

We're building the platform that captures the next wave.

### Target: $100M ARR

| Metric | Target | Path |
|--------|--------|------|
| Users | 1M+ | Viral loops, free tier, community |
| Paid Conversion | 5% | Generous free tier → value-driven upgrade |
| ARPU | $200/year | Tiered pricing, usage-based credits |
| Revenue | $100M ARR | 500K paid users × $200 |

---

## Core Product Pillars

### 1. Instant Value

**Time to first app: Under 2 minutes.**

Users should see a working application within 120 seconds of their first prompt. No setup. No configuration. No waiting. The AI agent builds in real-time, showing progress via an animated task feed so users feel momentum, not mystery.

Key elements:
- **5-second signup** — Email only, skip everything else
- **Smart onboarding** — Brief survey captures project context (industry, app type)
- **Immediate building** — First prompt triggers visible agent work
- **Live task feed** — "Creating database schema → Building UI → Starting preview"
- **Working preview** — Interactive app appears as agent builds

### 2. AI-First Everything

**The agent does the work. You guide it.**

Vibe's AI agent operates in two modes:

**Agent Mode (Default):**
- Autonomous multi-step execution
- Creates files, writes code, fixes bugs
- Searches documentation when stuck
- Generates images when needed
- Self-verifies by running the app

**Chat Mode:**
- Brainstorm and plan without changing code
- Diagnose issues collaboratively
- Get step-by-step plans, then "Implement the plan" in one click

The agent shows its work—every file changed, every line modified. Users see exactly what happened, building trust through transparency.

### 3. Full-Stack Out of the Box

**Every app comes with a production-ready backend.**

When you create a project, Vibe automatically provisions:
- **PostgreSQL database** — Real relational data, not toy storage
- **User authentication** — Email, OAuth, magic links
- **File storage** — Images, documents, user uploads
- **Edge functions** — Custom server-side logic
- **API routes** — RESTful endpoints, auto-generated

No configuration. No DevOps. Just describe what your app should do.

### 4. Visual + Code Hybrid

**Non-coders design. Developers code. Same project.**

**Design Mode:**
- Click any element to select it
- Adjust spacing, typography, colors via GUI
- Apply themes across entire project
- Import brand assets and style guides
- Visual changes don't consume credits

**Code Mode:**
- Full editor with syntax highlighting
- Edit React/TypeScript directly
- Add npm packages
- See AI changes as diffs
- Changes sync with visual model

**Select-and-Edit:**
- Click element in preview → "Make this button blue"
- AI modifies the exact component
- See the code change in real-time

### 5. One-Click Everything

**Publish, share, collaborate—no friction.**

- **Publish** — Single button deploys to production URL
- **Custom domains** — Connect your own domain
- **GitHub sync** — Two-way sync, never locked in
- **Invite team** — Unlimited collaborators on all plans
- **Share** — Public by default, social proof built in

---

## Complete Feature Set

### AI Agent Capabilities

| Feature | Description |
|---------|-------------|
| **Multi-step execution** | Complex tasks broken into subtasks automatically |
| **Self-correction** | Agent detects errors and fixes them |
| **Documentation search** | Agent reads docs when implementing unfamiliar APIs |
| **Image generation** | AI-generated images for placeholders and assets |
| **Code review** | Agent explains changes and suggests improvements |
| **Debugging** | Agent inspects logs, network calls, runtime errors |
| **Test generation** | Automatic test creation for critical paths |

### Editor Features

| Feature | Description |
|---------|-------------|
| **Multi-file editing** | Open multiple files in tabs |
| **Syntax highlighting** | Full language support (TS, JS, CSS, HTML, Rust) |
| **IntelliSense** | Autocomplete, type hints, error highlighting |
| **Search & replace** | Project-wide search with regex support |
| **Git integration** | Commit, push, pull from within IDE |
| **Terminal** | Integrated terminal for commands |
| **ATIF viewer** | Browse agent trajectories, see every action |

### Design Features

| Feature | Description |
|---------|-------------|
| **Theme system** | Global colors, fonts, spacing |
| **Component inspector** | Click to select, see properties |
| **Spacing controls** | Visual margin/padding adjustment |
| **Typography panel** | Font family, size, weight, alignment |
| **Color picker** | Text, background, border colors |
| **Image management** | Upload, AI-generate, or URL |
| **Responsive preview** | Desktop, tablet, mobile views |

### Backend Features

| Feature | Description |
|---------|-------------|
| **Database browser** | View tables, run queries, edit data |
| **Auth dashboard** | Manage users, sessions, permissions |
| **Storage browser** | Upload, organize, serve files |
| **Function logs** | Real-time serverless function logs |
| **Environment variables** | Secure secret storage |
| **API explorer** | Test endpoints, see responses |

### Deployment Features

| Feature | Description |
|---------|-------------|
| **Instant publish** | One click to production |
| **Preview deployments** | Share work-in-progress |
| **Custom domains** | HTTPS, auto-renewal |
| **CDN** | Global edge caching |
| **Security scanning** | Static analysis on publish |
| **SEO optimization** | Meta tags, semantic HTML |
| **Analytics** | Page views, user signups, events |

### Collaboration Features

| Feature | Description |
|---------|-------------|
| **Unlimited collaborators** | Even on free tier |
| **Role-based permissions** | Admin, Editor, Viewer |
| **Real-time presence** | See who's editing what |
| **Comment threads** | Discuss code and design |
| **Version history** | Every AI edit is versioned |
| **One-click rollback** | Undo any change instantly |
| **Activity feed** | See all project activity |

### Integration Features

| Feature | Description |
|---------|-------------|
| **Payments** | Stripe-style payment integration |
| **E-commerce** | Product catalog, cart, checkout |
| **Email** | Transactional and marketing email |
| **Analytics** | Third-party analytics integration |
| **CRM** | Customer relationship management |
| **Automation** | Workflow automation (n8n-style) |
| **Any API** | Agent can integrate any REST API |

### Enterprise Features

| Feature | Description |
|---------|-------------|
| **SSO** | SAML, OIDC corporate login |
| **Data residency** | Choose where data lives |
| **Audit logs** | Complete activity history |
| **Custom branding** | White-label option |
| **SLA** | Guaranteed uptime |
| **Dedicated support** | Priority assistance |
| **Custom integrations** | Bespoke connector development |

---

## User Experience Flows

### Flow 1: "I Have an Idea"

```
User arrives → Email signup (5 sec) → Quick survey (30 sec)
    ↓
"Build me a task management app with teams and deadlines"
    ↓
Agent starts building (visible task feed)
    ↓
"Creating database schema..."
"Building team management UI..."
"Adding deadline notifications..."
"Starting preview server..."
    ↓
Working app in preview (~90 seconds)
    ↓
User clicks around, tests features
    ↓
"Make the sidebar collapsible"
    ↓
Agent modifies code, preview updates
    ↓
"Publish" → Live at myapp.vibe.dev
```

### Flow 2: "I Have a Design"

```
User imports Figma/screenshot
    ↓
Agent analyzes design, identifies components
    ↓
"Implementing header navigation..."
"Creating hero section..."
"Building feature grid..."
    ↓
Pixel-perfect UI appears in preview
    ↓
User tweaks in Design Mode (no credits used)
    ↓
"Add user authentication"
    ↓
Agent adds auth, preserving design
    ↓
Publish
```

### Flow 3: "I Need to Iterate"

```
User opens existing project
    ↓
"Users are complaining the checkout is confusing"
    ↓
Agent analyzes checkout flow
    ↓
(Chat Mode) "I recommend these changes:
1. Simplify to 3 steps
2. Add progress indicator
3. Show order summary sidebar"
    ↓
User: "Implement the plan"
    ↓
Agent executes all changes
    ↓
Preview shows new checkout
    ↓
User tests, adjusts minor details in Design Mode
    ↓
Publish update
```

### Flow 4: "Team Collaboration"

```
Founder builds MVP with Vibe
    ↓
Invites designer (free)
    ↓
Designer uses Design Mode to polish UI
    ↓
Invites developer (free)
    ↓
Developer uses Code Mode to add custom logic
    ↓
All changes sync in real-time
    ↓
Founder reviews in activity feed
    ↓
Team publishes together
```

---

## Pricing Model

### Philosophy

- **Generous free tier** — Users should build real things for free
- **Value-based upgrade** — Pay when you need more, not sooner
- **Predictable costs** — No surprise bills
- **Team-friendly** — Collaborators don't cost extra

### Tiers

#### Free — $0/month
- 5 AI credits per day (150/month)
- Public projects only
- Unlimited collaborators
- Community support
- Vibe branding on published sites

*Perfect for: Trying Vibe, learning, hobby projects*

#### Pro — $25/month
- 100 AI credits
- Private projects
- Custom domains
- No Vibe branding
- GitHub sync
- Code export
- Email support

*Perfect for: Solo builders, freelancers, side projects*

#### Team — $50/month
- 200 AI credits
- Everything in Pro
- Role-based permissions
- Team workspaces
- Priority support
- SSO integration
- Data training opt-out

*Perfect for: Startups, agencies, small teams*

#### Enterprise — Custom
- Unlimited credits
- Everything in Team
- Dedicated support
- Custom integrations
- SLA guarantee
- On-premise option
- Custom design systems

*Perfect for: Large organizations, regulated industries*

### Credit System

| Action | Credits |
|--------|---------|
| Simple edit | 0.5 |
| Chat message | 1 |
| Component generation | 2-5 |
| Full page generation | 5-10 |
| App scaffold | 10-20 |
| Design Mode edits | 0 (free) |

- Daily free credits used first
- Unused credits roll over (annual plans)
- Credits visible in UI, never surprise bills

---

## OpenAgents Integration

### Nostr Identity

Unlike traditional platforms requiring OAuth with tech giants, Vibe uses Nostr:

- **No account creation** — Keypair generated on first run
- **Self-sovereign** — You own your identity
- **Portable** — Same identity across OpenAgents ecosystem
- **Optional GitHub** — Link later for cross-device access

### Bitcoin Payments

Vibe accepts Bitcoin/Lightning alongside traditional payment:

- **Pay with Bitcoin** — Lightning for instant settlement
- **Earn with Bitcoin** — Sell compute to swarm network
- **Micropayments** — Pay per credit, no minimums
- **Self-custodial** — Your keys, your bitcoin

### Commander Integration

Commander and Vibe are siblings in the OpenAgents family:

| Commander | Vibe |
|-----------|------|
| Multi-agent orchestration | Single-focus building |
| StarCraft-style control | IDE-style workflow |
| Agent monitoring | Product creation |
| Swarm compute management | App development |

**Shared capabilities:**
- Same Nostr identity
- Same Bitcoin wallet
- Same ATIF trajectory format
- UI components can be shared

### Marketplace Integration

- **Discover agents** — Browse agents optimized for different tasks
- **Publish apps** — Share your Vibe projects as templates
- **Sell templates** — Monetize your designs and scaffolds
- **Use swarm compute** — Agent work can run on network

### ATIF Trajectories

Every agent action is logged in ATIF format:

```json
{
  "schema_version": "ATIF-v1.4",
  "session_id": "vibe-session-123",
  "agent": { "name": "vibe-builder", "version": "1.0" },
  "steps": [
    {
      "step_id": 1,
      "type": "action",
      "tool_calls": [{
        "tool": "file_write",
        "args": { "path": "/src/App.tsx", "content": "..." }
      }]
    }
  ]
}
```

This enables:
- **Transparency** — See exactly what agent did
- **Debugging** — Understand failures
- **Learning** — Improve agents over time
- **Sharing** — Publish successful trajectories

---

## Technical Differentiation

### Native-First, Browser-Later

Most platforms are browser-only or cloud-only. Vibe is native-first:

**Phase 1 (Now): Native Desktop**
- GPUI rendering (GPU-accelerated)
- Local filesystem access
- Wasmtime for WASI modules
- Full IDE performance

**Phase 2 (Later): Browser**
- Same code, compiled to WASM
- IndexedDB for storage
- Browser WASM runtime
- Zero-install experience

**Why this matters:**
- Native performance during development
- Browser reach for distribution
- Same codebase, two deployment targets

### OANIX Architecture

Vibe runs on OANIX, our Plan 9-inspired agent OS:

```
/workspace    → Project files (WorkspaceFs)
/logs         → Build logs, ATIF trajectories (LogsFs)
/cap/agents   → Agent capabilities (AgentFs)
/cap/net      → Network capabilities (NetFs)
/cap/payments → Payment capabilities (PaymentsFs)
```

**Benefits:**
- Clean abstractions that work native and browser
- Capability-based security (mount what you need)
- Agent isolation (each agent has its own namespace)
- Transparent logging (everything goes through FS)

### Rust Backend, Not Node

While the frontend uses React, backend handlers are Rust compiled to WASM:

**Benefits:**
- Smaller, faster binaries
- No Node.js runtime to port
- Deterministic execution
- Same language as OANIX kernel
- Better security guarantees

```rust
// backend/src/routes.rs
pub async fn hello(_req: Request) -> Response {
    Response::json(json!({ "message": "Hello from Rust" }))
}
```

### Full Code Ownership

Users own their code completely:

- **GitHub sync** — Two-way, real-time
- **Export anytime** — Download full codebase
- **Standard stack** — React, Tailwind, no proprietary frameworks
- **No lock-in** — Continue development anywhere
- **Open formats** — ATIF, standard JSON configs

---

## Growth Strategy

### Viral Loops

1. **Public by default** — Every project is shareable
2. **Social proof** — "Built with Vibe" badge
3. **Template gallery** — Users create, others clone
4. **Success stories** — Highlight fast builds
5. **Community** — Discord, Twitter, forums

### Content Marketing

1. **"Build X in Y minutes"** — Tutorial videos
2. **Template library** — Pre-built starting points
3. **Documentation** — Comprehensive, searchable
4. **Blog** — Thought leadership on AI development
5. **Comparisons** — SEO for "how to build X"

### Community Building

1. **Discord** — Real-time help, sharing
2. **Experts program** — Power users help others
3. **Hackathons** — Build competitions
4. **Office hours** — Live building sessions
5. **Affiliate program** — Incentivize referrals

### Enterprise Motion

1. **Bottom-up adoption** — Individual users → team → org
2. **Enterprise features** — SSO, audit logs, compliance
3. **Custom success** — Dedicated onboarding
4. **Partner channel** — Agencies, consultants

---

## Success Metrics

### North Star: Apps Published

The ultimate measure: how many working applications do users publish?

### Leading Indicators

| Metric | Target | Why It Matters |
|--------|--------|----------------|
| Time to first app | <2 min | Immediate value |
| Day-1 retention | >40% | Users find value |
| Day-30 retention | >25% | Long-term engagement |
| Apps per user | >3 | Platform stickiness |
| Publish rate | >50% | Users complete projects |
| Upgrade rate | >5% | Willingness to pay |
| NPS | >50 | User satisfaction |

### Business Metrics

| Metric | Year 1 | Year 2 | Year 3 |
|--------|--------|--------|--------|
| Users | 100K | 500K | 1M |
| Paid Users | 5K | 25K | 50K |
| ARR | $1M | $10M | $50M |
| Team Size | 10 | 30 | 75 |

---

## Competitive Advantages

### 1. Native + Browser

No other platform offers native desktop performance with browser extraction. Users get the best of both worlds.

### 2. Decentralized Identity

Nostr identity is unique in this space. Appeals to sovereignty-focused users and aligns with Web3 trends without the crypto complexity.

### 3. Bitcoin-Native

First vibe-coding platform with real Bitcoin/Lightning integration. Micropayments, swarm compute payments, self-custody.

### 4. Multi-Agent Future

Through Commander integration, Vibe can orchestrate multiple agents—a capability no competitor has explored.

### 5. Open Source Core

OANIX and core Vibe components will be open source. Builds trust, enables contributions, creates moat through ecosystem.

### 6. Plan 9 Architecture

OANIX's Plan 9-inspired design is technically superior—clean abstractions that competitors' ad-hoc architectures can't match.

### 7. Local-First Data

User data stays on their computer by default. Growing concern about cloud data makes this increasingly attractive.

### 8. OpenAgents Ecosystem

Vibe isn't standalone—it's part of Commander, Gym, Marketplace, Wallet. Network effects across products.

---

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| LLM costs | Multi-provider support, local FM option, efficient prompting |
| AI quality ceiling | Agentic architecture, specialized agents, human-in-loop |
| Large competitor resources | Speed of execution, unique positioning, community |
| User trust in AI | Transparency (ATIF), code ownership, undo/rollback |
| Browser WASM limitations | Native-first ensures fallback always works |
| Enterprise adoption barriers | Security certifications, compliance features |

---

## Conclusion

Vibe is positioned to capture the AI-native development market by combining:

1. **Best-in-class UX** — Instant value, visual + code hybrid
2. **Unique technical architecture** — OANIX, native-first, Rust backend
3. **Differentiated positioning** — Nostr, Bitcoin, open source
4. **Ecosystem effects** — Commander, Marketplace, swarm compute

The market is ready. The technology is ready. The only question is execution speed.

**Build something Vibe.**

---

*Last Updated: December 2024*
