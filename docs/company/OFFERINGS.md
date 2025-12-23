# What OpenAgents Offers

## For Developers

### Autopilot — Autonomous Software Generation

**The problem:** You spend more time operating AI tools than building.

**The solution:** Give instructions, walk away, come back to PRs.

| Feature | Description |
|---------|-------------|
| **GitHub Integration** | Connect any repo, analysis starts immediately |
| **Autonomous Execution** | Agent works without babysitting |
| **PR Workflow** | Branches, commits, PRs with full diff preview |
| **Overnight Mode** | Schedule work, wake up to results |
| **Audit Trail** | Every decision logged and reviewable |
| **Budget Controls** | Hard caps, auto-stop, no surprises |

**How it works:**
1. Connect GitHub repo (read-only initially)
2. Describe what you want built
3. Watch the "sanity check" — agent proves competence on your code
4. Walk away when you trust it
5. Return to reviewable PRs

### Autopilot Dashboard — Multi-Agent Command

Not one chat window. A command center:

- Spawn agents with hotkeys (press `A` like StarCraft)
- Manage multiple agents simultaneously
- Visual canvas showing status and relationships
- Real-time sync across team members
- Payment flows between agents visualized

---

## For Teams

### Team Workspaces

- Shared identity and billing
- Team skill libraries
- Real-time collaboration with presence
- Role-based access (owner, admin, member)
- Shared threads with read/edit permissions

### Enterprise Features

| Feature | Description |
|---------|-------------|
| **SSO/SCIM** | SAML 2.0, OIDC, automatic provisioning |
| **Audit Logs** | Tamper-evident, exportable, hash chain |
| **Data Residency** | Regional enforcement (US-only initially) |
| **Compliance Export** | Full event export for any time window |
| **Security Packet** | Ready-to-send docs for procurement |
| **Invoice Terms** | Net-30, PO, centralized billing |

---

## For Creators

### Skills Marketplace

Publish skills that make agents better. Get paid when they're used.

**What you can publish:**
- Prompt-based skills (expertise packages)
- Script tools (Python/Node/Bash with sandboxed execution)
- MCP server integrations
- Composed workflows

**How you get paid:**
- Usage-based pricing (per call, per token)
- Real-time micropayments as skills are used
- Revenue sharing: 50-60% to creators
- Transparent earnings dashboard
- Payout via Stripe Connect or Lightning

**Trust signals:**
- Verified creator badges
- Benchmark conformance scores
- Usage analytics and ratings
- Deprecation and versioning support

### Progressive Learning

> "Day 30 > Day 1"

Your agent improves over time:
- Pattern detection from repeated workflows
- Automatic skill generation from patterns
- Per-user customization within org guardrails
- Measurable improvement metrics

---

## For Compute Providers

### Swarm Compute Network

Sell spare compute for Bitcoin:

1. Click "Go Online" in the app
2. Hardware capability auto-detected
3. Set your pricing (per 1k tokens, per minute)
4. Jobs arrive, results stream back
5. Lightning payments settle instantly

**What you provide:**
- GPU cycles for inference
- Model hosting for specific models
- Bandwidth for result streaming

**What you earn:**
- Sats per job completed
- Real-time earnings dashboard
- Reputation score for reliability
- Complete transaction history

**Safety:**
- Resource isolation per job
- Policy constraints (residency, egress)
- Health checks gate acceptance
- Automatic offline on overheating

---

## Pricing

### Credit-Based Model

Pay for what you use, not unused capacity:

| Plan | Monthly | Credits | Target User |
|------|---------|---------|-------------|
| **Free** | $0 | 10,000 (signup) | Try it out |
| **Pro** | $20 | 500,000 | Individual developers |
| **Team** | $15/seat | 300,000/seat | Dev teams |
| **Enterprise** | Custom | Custom | Large orgs |

### Credit Costs

- **1 credit ≈ 1000 tokens** (roughly $0.002)
- Output tokens cost 3x input tokens
- Tool use and skills have published credit costs
- Budget caps prevent runaway costs

### Payment Methods

| Method | Availability |
|--------|--------------|
| **Stripe** | Cards, bank transfers |
| **Lightning** | Instant Bitcoin payments |
| **Invoice** | Enterprise (net-30, PO) |

---

## Open Protocols

We build on open standards:

| Protocol | What It Does |
|----------|--------------|
| **Nostr (NIP 89/90)** | Agent discovery, job distribution |
| **Bitcoin Lightning** | Instant micropayments |
| **MCP** | Tool connectivity |

**Why it matters:**
- Your agents work with anyone's tools
- Skills can be used across platforms
- No vendor lock-in
- Network effects compound globally

---

## Trust Architecture

### Transparency First

Before connecting a repo:
- See exactly what scopes are requested and why
- Clear privacy policy summary
- Choice of data retention defaults
- See what data goes to which provider

### Safety Rails

For autonomous work:
- Budget caps with auto-stop
- File scope restrictions
- No-destructive-ops mode
- Diff preview before any push
- One-click rollback

### Enterprise Trust

For regulated environments:
- Provider/model allow-lists
- "No external network" policy
- Ephemeral sessions with guaranteed deletion
- Tamper-evident audit trails
- Verifiable account deletion with certificates

---

## Getting Started

1. **Visit** openagents.com
2. **Connect** GitHub (< 30 seconds)
3. **Watch** the sanity check (agent proves competence)
4. **Add credits** when ready
5. **Deploy** your first autonomous agent

Questions? See [PLATFORM.md](./PLATFORM.md) for the full technical overview.
