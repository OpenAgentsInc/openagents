# Vibe Implementation Plan

Execution plan derived from `README.md`, `LAUNCH_PLAN.md`, `TECHNICAL_ARCHITECTURE.md`, `FEATURES.md`, and `PRICING.md`. Targets the 6-month $1B ARR path with a phased delivery schedule.

---

## Guiding Principles
- Ship native + web + edge together; Vibe Web (WASM) is the public entrypoint, Desktop/GPUI stays in lockstep.
- Treat every project as an OANIX namespace; same API on browser, desktop, and Cloudflare.
- Nostr-first identity, Lightning-first payments; Stripe is a fallback.
- Infra resale is a first-class product (multi-tenant DOs, R2/D1/KV, usage metering, billing).
- Prefer small, shippable cuts with observable outcomes (deployable demos every week).

---

## Phases & Deliverables

### Phase 1 - Foundation (Weeks 1-2)
- [ ] Vibe Web (Dioxus 0.7 to WASM) builds & serves at `vibe.run` (Pages) with router + cached bundle <=3 MB.
- [ ] Nostr auth (challenge/verify) wired to D1 `users`, JWT issuance, client IndexedDB key storage.
- [ ] Project CRUD + file tree backed by Project DO + R2; previews via sandboxed iframe.
- [ ] MechaCoder chat -> `/api/chat/ws` (Claude primary, Workers AI fallback) with streaming UI.
- [ ] Starter templates (landing, dashboard, API) selectable in UI and hydrated into /workspace.
- [ ] Basic deploy flow to Cloudflare Pages/Workers from UI; logs streamed from DO.
- [ ] Billing rails stub: Stripe + Lightning endpoints, customer records in D1.
- [ ] Infra resale skeleton: provisioning request -> DO entry + R2/D1 prefix, subdomain allocation.

### Phase 2 - Growth (Weeks 3-4)
- [ ] Team/Business tiers: team workspaces, role enforcement, shared billing.
- [ ] Usage metering and invoices for infra resale; plan-based rate limits applied in Worker middleware.
- [ ] Marketplace beta: list/install agents + templates, Lightning payouts.
- [ ] OANIX real execution path: WASI job submission from UI, LogsFs streaming, scheduler wiring.
- [ ] Analytics dashboard v1: requests, latency, errors, AI token usage pulled from D1/KV aggregates.

### Phase 3 - Scale (Weeks 5-6)
- [ ] Enterprise features: SSO (SAML/OIDC), audit logs, SLA dashboards, data residency toggles.
- [ ] Multi-region Cloudflare deployment, shard placement for DOs, aggressive KV caching.
- [ ] Marketplace GA: reviews, versioning, private/team agents/templates.
- [ ] Team collab upgrades: presence, cursors, live edits (CRDT/OT), approvals.

### Phase 4 - Hypergrowth (Weeks 7-12)
- [ ] Compliance tracks: SOC 2 Type II, HIPAA option scaffolding, logging retention policies.
- [ ] Compute marketplace (buy/sell credits), spot/reservation pricing.
- [ ] Mobile + desktop packaging; offline mode via IndexedDB + replay.
- [ ] On-prem / private cloud option for enterprise accounts.

---

## Workstreams & Owners (proposed)
- **Web/Desktop Experience:** Dioxus app (router, editor, preview, auth UX).
- **Runtime & OANIX:** Namespace mounts, scheduler, WASI execution, LogsFs streaming.
- **AI & Agents:** MechaCoder chat + tools, agent run orchestration, completions.
- **Infra Resale:** Provisioning (subdomain/DO/R2/D1), metering, billing, customer dashboard.
- **Payments & Identity:** Nostr auth, Lightning rails, Stripe fallback, JWT/session middleware.
- **Marketplace:** Listings, payments/payouts, install to project, analytics.
- **Compliance & Observability:** Rate limiting, audit logs, metrics, alerts, SOC 2 evidence.

---

## Immediate Backlog (this sprint)
- Vibe UI: add infra resale + billing scaffolding (mock data, customer view, invoice preview).
- Define client auth state + placeholders for Nostr challenge/verify flows.
- Flesh Project snapshot mocks to mirror planned data (projects, infra customers, billing events).
- Hook deploy/log buttons to snapshot updates (keeps UX responsive until real backends land).
- CI: ensure wasm32 target build command lands in Dioxus workflow (follow TECHNICAL_ARCHITECTURE.md).

---

## Success Criteria (short-term)
- Browser demo shows: projects/templates, editor + agent feed, deploy/log actions, infra customer list, billing summary.
- Bundles build with `cargo build -p dioxus --features web` (or wasm target) without manual patching.
- Clear mapping from docs -> UI/DTOs -> endpoints so backend team can pick tasks without re-reading strategy docs.
