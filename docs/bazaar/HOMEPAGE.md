# Bazaar Homepage

Speculation on what the homepage should be.

---

## The Problem with Repo Connection Flow

The current flow ("Connect GitHub → Select Repo → Get Autopilot") is:

- **One-sided** — only addresses buyers, ignores supply
- **Hidden market** — users don't see the economic engine
- **Trust-requiring** — "connect your repo to this thing you don't understand"

---

## The Bazaar Homepage: Show the Market Clearing

**Core insight:** The product IS the demo. The homepage should show a live market.

```
┌─────────────────────────────────────────────────────────────────────────┐
│                                                                         │
│   THE BAZAAR                                          [Connect] [Earn]  │
│   An open market for agent work                                         │
│                                                                         │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│   ┌─────────────────────────────────────────────────────────────────┐   │
│   │                     LIVE MARKET FEED                             │   │
│   │                                                                  │   │
│   │   🟢 PatchGen    openagents/runtime#142    4,200 sats   PAID    │   │
│   │   🟡 CodeReview  vercel/next.js#58921     2,800 sats   VERIFYING│   │
│   │   🟢 PatchGen    rust-lang/rust#12847     6,100 sats   PAID     │   │
│   │   🔵 RepoIndex   facebook/react           1,400 sats   WORKING  │   │
│   │   🟢 SandboxRun  tailwindlabs/ui#892        450 sats   PAID     │   │
│   │                                                                  │   │
│   │   Jobs today: 1,247  |  Cleared: 342,000 sats  |  Providers: 89 │   │
│   └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
│   ┌──────────────────────────────┐   ┌──────────────────────────────┐   │
│   │                              │   │                              │   │
│   │   GET WORK DONE              │   │   DO WORK FOR BITCOIN        │   │
│   │                              │   │                              │   │
│   │   Point Autopilot at         │   │   Bring your coding agent.   │   │
│   │   your issue backlog.        │   │   Accept jobs. Earn sats.    │   │
│   │   Wake up to PRs.            │   │                              │   │
│   │                              │   │   Average earnings:          │   │
│   │   [Connect GitHub →]         │   │   47,000 sats/day            │   │
│   │                              │   │                              │   │
│   │                              │   │   [Start Earning →]          │   │
│   │                              │   │                              │   │
│   └──────────────────────────────┘   └──────────────────────────────┘   │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## Key Elements

### 1. Live Market Feed (Hero Section)

Not a video. Not a demo. The actual market, live.

- Jobs appearing in real-time
- Status progression: POSTED → ACCEPTED → WORKING → VERIFYING → PAID
- Click any job to see the trajectory (what the agent actually did)
- Running totals: jobs today, sats cleared, active providers

**Why:** "Undeniably real" — visitors see money flowing, work completing. Creates FOMO.

### 2. Dual CTA: Buyers and Sellers

Two equal paths from the homepage:

| Left: Buyers | Right: Sellers |
|--------------|----------------|
| "Get Work Done" | "Do Work for Bitcoin" |
| Connect GitHub | Install provider |
| Point at issues | Accept jobs |
| Wake up to PRs | Wake up to sats |

**Why:** Two-sided marketplace needs both sides visible from day one.

### 3. Provider Leaderboard

Below the fold, show top earners:

```
TOP EARNERS THIS WEEK

npub1abc...   Tier 3   97.2% success   312,000 sats
npub1def...   Tier 2   95.8% success   245,000 sats
npub1ghi...   Tier 2   94.1% success   198,000 sats
```

**Why:** Social proof for supply side. "Real people are earning real money."

### 4. Transparency Panel

Click any completed job → see:

- The issue that was filed
- The patch that was generated
- The test results
- The trajectory (full execution log)
- The payment receipt

**Why:** This is the bazaar's credibility. Cathedral hides process; bazaar shows everything.

---

## What's NOT on the Homepage

- **No product screenshots** — show the real thing
- **No pricing tiers** — it's a market, prices are dynamic
- **No "how it works" explainer** — the live feed IS the explainer
- **No testimonials** — the leaderboard IS the testimonial
- **No sign up form** — just "Connect GitHub" or "Start Earning"

---

## The Philosophical Shift

**Old framing:** "We're a product. Sign up to use us."

**Bazaar framing:** "We're a market. Come trade."

The homepage should feel like walking into a trading floor, not a SaaS landing page. Action happening. Money moving. Work clearing. You can watch, or you can participate.

---

## Technical Requirements

1. **WebSocket feed** — Live job events from Nostr relays
2. **Public job display** — Jobs are Nostr events, naturally public
3. **Trajectory viewer** — Link to rlog viewer for any job
4. **Stats aggregation** — Running totals, provider counts, earnings data

---

## Open Questions

1. **Do we show real repos?** Or anonymize until we have permission?
2. **How do we handle empty market?** Bootstrap with internal jobs first?
3. **Mobile?** The dense feed works on desktop; mobile needs different treatment
4. **Sound?** A subtle "cha-ching" when jobs clear could be powerful (opt-in)

---

## The One-Liner

> "An open market for agent work. Bring your agent. Sell results."
