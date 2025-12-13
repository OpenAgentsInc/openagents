# Vibe Platform Documentation

**Target: $1B ARR in 6 Months**

---

## Quick Links

| Document | Description |
|----------|-------------|
| [LAUNCH_PLAN.md](./LAUNCH_PLAN.md) | Comprehensive business strategy and execution plan |
| [TECHNICAL_ARCHITECTURE.md](./TECHNICAL_ARCHITECTURE.md) | System design and implementation details |
| [PRICING.md](./PRICING.md) | Detailed pricing structure and revenue model |
| [FEATURES.md](./FEATURES.md) | Complete feature specification |

---

## What is Vibe?

Vibe is a vibe-coding platform that combines:
- **AI-powered development** (MechaCoder, agents, completions)
- **Native + browser runtime** (Dioxus, WASM, OANIX)
- **Global edge infrastructure** (Cloudflare Workers resale)
- **Decentralized identity** (Nostr, Lightning payments)

---

## Revenue Model

| Stream | Month 6 Target | Strategy |
|--------|---------------|----------|
| Platform Subscriptions | $50M/mo | 500K users × $100 avg |
| Infrastructure Resale | $60M/mo | 6K customers × $10K avg |
| Marketplace Fees | $30M/mo | 15% of $200M GMV |
| Enterprise Contracts | $27M/mo | 90 enterprises × $300K avg |
| **Total** | **$167M/mo** | **$2B ARR run rate** |

---

## Key Differentiators

| Feature | Competitors | Vibe |
|---------|-------------|------|
| Runtime | Cloud or browser-only | Native + browser + edge |
| Identity | OAuth (vendor lock-in) | Nostr (portable) |
| Payments | Stripe (2.9% + $0.30) | Lightning (< 1%) |
| Infrastructure | Consume only | Resell (4x margin) |
| Data | Vendor-controlled | User-controlled |

---

## Technical Stack

```
┌────────────────────────────────────────┐
│         USER INTERFACES                 │
│  Desktop (Dioxus) │ Web (WASM) │ API   │
├────────────────────────────────────────┤
│         APPLICATION LAYER               │
│  MechaCoder │ Projects │ Marketplace   │
├────────────────────────────────────────┤
│         RUNTIME (OANIX)                 │
│  Namespace │ Scheduler │ WASI          │
├────────────────────────────────────────┤
│         CLOUDFLARE EDGE                 │
│  Workers │ DOs │ R2 │ D1 │ AI          │
├────────────────────────────────────────┤
│         IDENTITY & PAYMENTS             │
│  Nostr │ Lightning │ Bitcoin           │
└────────────────────────────────────────┘
```

---

## Execution Timeline

| Phase | Timeline | Key Milestones |
|-------|----------|----------------|
| **Foundation** | Month 1-2 | MVP launch, 100K users, $500K MRR |
| **Scale** | Month 3-4 | Teams, marketplace, $10M MRR |
| **Hypergrowth** | Month 5-6 | Enterprise, expansion, $167M MRR |

---

## Competitive Positioning

```
               NON-TECHNICAL ←─────────────────────────→ PROFESSIONAL
                     │                                        │
    LOVABLE ●        │                                        │  ● CURSOR
    (cloud, opinionated)                                      │  (local, IDE)
                     │                                        │
                     │              ● VIBE                    │
                     │      (native + cloud + edge)           │
                     │              (full-stack)              │
                     │              (infra resale)            │
    BOLT ●           │                                        │  ● WINDSURF
    (browser, frontend)                                       │  (local, IDE)
                     │                                        │
                     │                                        │
              REPLIT ●                                        │
              (cloud, educational)                            │
```

---

## What We've Built

### Existing Infrastructure

| Component | Location | Status |
|-----------|----------|--------|
| **OANIX** | `crates/oanix/` | 180+ tests, production-ready |
| **Cloudflare Relay** | `crates/cloudflare/` | Live, NIP-01/NIP-90 |
| **Vibe UI** | `crates/vibe/` | Stage 3, Dioxus integration |
| **MechaCoder** | `crates/mechacoder/` | Claude streaming working |
| **Dioxus App** | `crates/dioxus/` | Web UI functional |

### What Needs Building

| Component | Priority | Timeline |
|-----------|----------|----------|
| Vibe Web (WASM) | P0 | Week 1-2 |
| Nostr Auth | P0 | Week 1-2 |
| Project CRUD | P0 | Week 2-3 |
| Infrastructure Resale | P0 | Week 3-4 |
| Billing (Stripe + Lightning) | P0 | Week 4-5 |
| Marketplace | P1 | Week 5-8 |
| Team Features | P1 | Week 6-8 |
| Enterprise | P2 | Week 9-12 |

---

## Getting Started

### Development Setup

```bash
# Clone the repo
git clone https://github.com/openagents/openagents.git
cd openagents

# Run the Dioxus web app
cd crates/dioxus
dx serve

# Run Cloudflare Workers locally
cd crates/cloudflare
wrangler dev
```

### Key Files

| File | Purpose |
|------|---------|
| `crates/vibe/src/lib.rs` | Vibe crate entry |
| `crates/vibe/src/screen.rs` | Main Vibe UI |
| `crates/oanix/src/lib.rs` | OANIX runtime |
| `crates/cloudflare/src/lib.rs` | Worker entry |
| `crates/dioxus/src/main.rs` | Web app entry |

---

## Success Metrics

| Metric | Month 1 | Month 3 | Month 6 |
|--------|---------|---------|---------|
| Free Users | 50K | 400K | 5M |
| Paid Users | 2K | 25K | 500K |
| Infra Customers | 50 | 500 | 6K |
| MRR | $700K | $8M | $167M |
| ARR | $8.4M | $96M | $2B |

---

## Team Requirements

| Phase | Headcount | Key Roles |
|-------|-----------|-----------|
| Month 1-2 | 20 | Engineers, DevRel, Sales |
| Month 3-4 | 60 | + Marketing, CS, Ops |
| Month 5-6 | 120 | + Enterprise, Support |

---

## Funding Needs

| Use | Amount |
|-----|--------|
| Hiring (6 months) | $40M |
| Infrastructure | $15M |
| Sales & Marketing | $30M |
| Working Capital | $15M |
| **Total** | **$100M** |

---

## Contact

For questions about this plan, reach out to the OpenAgents team.

---

*Document Version: 1.0*
*Last Updated: December 2024*
