# **ARCHIVED** - OpenAgents Production Readiness TODO

> **ARCHIVED:** 2026-01-13. This document is no longer maintained.
>
> For current planning:
> - **Roadmap:** [ROADMAP.md](../../../ROADMAP.md)
> - **What's wired:** [SYNTHESIS_EXECUTION.md](../../../SYNTHESIS_EXECUTION.md)
> - **Terminology:** [GLOSSARY.md](../../../GLOSSARY.md)

---

**Generated:** 2025-12-26 (Updated: 2025-12-26)
**Goal:** Bring all 27 directives to full production readiness with WGPUI, real integrations, and comprehensive testing.

---

## Launch Critical Path (Priority: Episodes 199/200)

These items are required for the public launch demo and revenue funnel.

### Demo + Funnel (d-027) ❌ NEW

- [ ] Homepage replay component (WGPUI or web)
- [ ] Free repo connect wizard (GitHub OAuth) - NO PAYMENT FIRST
- [ ] Free first analysis (scan repo, show value)
- [ ] Free trial run (1 issue → see PR)
- [ ] Upgrade prompt (show value delivered)
- [ ] Checkout flow (Stripe + Lightning) - AFTER value proven
- [ ] Replay publishing pipeline (promote → redact → publish)

### Autopilot GitHub Flow (d-004 Phase 9-13) ❌ NEW

- [ ] GitHub repo connection (`openagents autopilot connect`)
- [ ] Issue claim with label/comment
- [ ] Branch → commits → PR creation
- [ ] PR with replay/receipts link
- [ ] CI detection and auto-fix
- [ ] Run Bundle export (`openagents autopilot export`)

### Compute Marketplace v1 (d-008 Phase 9) ❌ NEW

- [ ] SandboxRun job type (kind 5930/6930)
- [ ] RepoIndex job type (kind 5931/6931)
- [ ] Pay-after-verify settlement
- [ ] Provider tier system
- [ ] Reserve provider fallback
- [ ] Price book configuration

### Autopilot GUI (d-009 Phase 9-10) ❌ NEW

- [ ] Replay viewer with timeline scrubber
- [ ] Receipts panel (tests, CI, costs)
- [ ] Approval UX (allow once/repo/always)

### Dogfooding Infrastructure (d-018) 🟡 PARTIAL

- [ ] Fleet on OpenAgents repo
- [ ] Demo candidate selection
- [ ] Run Bundle generation for marketing

---

## Status Legend

| Symbol | Meaning |
|--------|---------|
| ✅ | Complete - Production ready |
| 🟡 | In Progress - Partially implemented |
| ⚠️ | Blocked - External dependency required |
| ❌ | Not Started - Needs implementation |

---

*[Remainder of original file continues below - see git history for full content]*
