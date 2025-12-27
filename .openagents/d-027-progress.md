# d-027 Implementation Progress

## Overview

**Directive:** Autopilot Demo + Dogfooding Funnel
**Status:** In Progress (MVP Phase)
**Last Updated:** 2025-12-26

## Completed Infrastructure

### ✅ Core Autopilot System
- Continuous autopilot loop with verification (commit 19cc225d9)
- GitHub integration for issue→branch→PR workflow (commit 4bf77ac53)
- CI integration with GitHub Actions detection (commit bd4ba5cdb)
- Session logging to JSONL format (logger.rs)
- After-action reporting with session stats (report.rs)
- Verification checklist with 9 checks (verification.rs)

### ✅ GitHub Workflow
- OAuth token management (github.rs)
- Repository connection and permissions
- Issue claiming with bot comments
- Branch creation from base SHA
- Pull request creation with body
- Label management (in-progress, needs-review)
- Receipt posting to PR comments

### ✅ Replay Bundle System (commit c4f2bb48d)
- ReplayBundle format matching d-027 spec
- JSONL→JSON conversion with timeline events
- Metadata extraction (duration, model, costs)
- Receipts (tests, CI status, files changed)
- Secret redaction (API keys, tokens, paths)
- CLI tool: `replay_demo` for bundle creation

## Implementation Status by Phase

### Phase 1: Demo Replay Component ⚠️ Partial
- [x] Replay bundle format specification
- [x] Timeline event structure
- [x] Tool call visualization data
- [x] Receipts panel data
- [x] Cost + duration + APM metadata
- [ ] **MISSING:** WGPUI replay viewer component
- [ ] **MISSING:** Scrub-able timeline UI
- [ ] **MISSING:** Inline diff display
- [ ] **MISSING:** Playback controls

**Status:** Data format complete, UI rendering not started

### Phase 2: Replay Publishing Pipeline ✅ Complete
- [x] Replay bundle format (ReplayBundle struct)
- [x] Automated redaction (regex-based secrets/PII)
- [x] CLI tool for conversion (replay_demo example)
- [x] Versioning in bundle format (version: "1.0")
- [ ] **MISSING:** CDN hosting configuration
- [ ] **MISSING:** "Promote to demo" workflow

**Status:** Core pipeline functional, distribution not configured

### Phase 3: Homepage Integration ❌ Not Started
- [ ] Homepage website existence
- [ ] Replay viewer embed
- [ ] Auto-play on scroll
- [ ] Mobile-responsive layout
- [ ] Loading states and error handling
- [ ] A/B testing infrastructure

**Status:** Requires web frontend (not in codebase)

### Phase 4: Repo Connection (FREE) ✅ Complete
- [x] GitHub OAuth flow (github.rs)
- [x] Repository picker (list_repos)
- [x] Permission validation (RepoPermissions)
- [x] Connection health check (check_github_auth)
- [x] No payment required (correct)

**Status:** Fully functional

### Phase 5: Free First Analysis ⚠️ Partial
- [x] Preflight config (preflight.rs)
- [x] Codebase detection (git, language, tools)
- [x] Project structure analysis
- [ ] **MISSING:** Issue detection/prioritization
- [ ] **MISSING:** "What Autopilot can do" summary UI
- [ ] **MISSING:** Value estimation ("X hours of work")

**Status:** Foundation exists, user-facing analysis not built

### Phase 6: Free Trial Run ⚠️ Partial
- [x] Full autopilot execution capability
- [x] PR creation with receipts
- [ ] **MISSING:** "1 free run" limit enforcement
- [ ] **MISSING:** Issue picker UI
- [ ] **MISSING:** "This would have taken X hours" messaging

**Status:** Backend works, UX/limits not implemented

### Phase 7: Upgrade Prompt ❌ Not Started
- [ ] Value delivered summary
- [ ] Pricing comparison UI
- [ ] Stripe integration
- [ ] Lightning payment option
- [ ] Plan selection (Solo/Team/Enterprise)
- [ ] Coupon/referral codes
- [ ] Immediate provisioning

**Status:** Payment infrastructure not in codebase

### Phase 8: Demo Testing ⚠️ Partial
- [x] Unit tests for replay (3 tests)
- [x] Integration test (end-to-end conversion)
- [ ] **MISSING:** CI pipeline for demo rendering
- [ ] **MISSING:** Regression tests
- [ ] **MISSING:** Performance tests (<3s load)
- [ ] **MISSING:** Accessibility tests

**Status:** Basic tests exist, comprehensive testing not configured

### Phase 9: Analytics + Optimization ❌ Not Started
- [ ] Funnel conversion tracking
- [ ] Heatmaps
- [ ] A/B testing framework
- [ ] Cohort analysis

**Status:** Not implemented

## Current Capability Assessment

### What Works Today
1. **Autopilot can autonomously work on OpenAgents issues**
   - Claim issue → create branch → edit code → run tests → create PR
   - Full GitHub integration functional
   - CI status checking works
   - Receipt generation works

2. **Session logs are captured**
   - JSONL format at `~/.openagents/sessions/YYYYMMDD/`
   - Full trajectory including tool calls, results, phases
   - Suitable for replay conversion

3. **Replay bundles can be generated**
   - `cargo run -p autopilot --example replay_demo <log.jsonl> <output.json>`
   - Secrets automatically redacted
   - Format matches d-027 specification

### What's Missing for MVP Demo

#### Critical Path (Must Have)
1. **Replay Viewer Component**
   - Need WGPUI component or web viewer
   - Timeline with scrubbing
   - Display tool calls and results
   - Show diff view for edits
   - Playback controls

2. **Demo Selection Workflow**
   - Mark session as "demo-worthy"
   - Automated quality checks (no crashes, tests passed)
   - One-command publish to demo

3. **Homepage/Landing Page**
   - Static site or served page
   - Embed replay viewer
   - Clear CTA ("Try It On Your Repo")

#### Nice to Have (Can Defer)
- Free trial enforcement (can be honor system initially)
- Payment integration (can start with manual onboarding)
- Advanced analytics (can use simple tracking)
- Mobile optimization (desktop-first is fine)

## Recommended Next Steps

### Option A: Web-Based MVP (Fastest to Demo)
1. Create static HTML page with embedded replay viewer
2. Use existing replay bundles as demo content
3. Simple CTA linking to GitHub OAuth flow
4. Launch on openagents.com subdomain
5. Manual onboarding for early users

**Pros:** Fast, no WGPUI complexity, easy to iterate
**Cons:** Separate from desktop app, need web dev skills

### Option B: Desktop-First Approach
1. Build WGPUI replay viewer component
2. Integrate into existing autopilot binary
3. `openagents replay view <bundle.json>` command
4. Export demo as video/GIF for homepage
5. Link to binary download

**Pros:** Unified codebase, showcases WGPUI
**Cons:** Slower, video less interactive than live demo

### Option C: Hybrid (Recommended)
1. Build simple web replay viewer (JavaScript)
2. Share data format with desktop viewer (to build later)
3. Homepage uses web viewer for instant demo
4. Desktop app uses WGPUI viewer for rich experience
5. Both consume same replay bundle format ✅ (already done)

**Pros:** Best of both worlds, pragmatic
**Cons:** Need both implementations eventually

## Technical Debt & Considerations

### Existing Issues to Address
1. **No stub implementations** - all code is real (d-012 compliant ✅)
2. **Tests are passing** - 12/12 in autopilot crate ✅
3. **CI integration works** - GitHub Actions detection functional ✅
4. **Clippy clean** - no warnings after recent fix ✅

### Security Review Needed
- [ ] Ensure secret redaction catches all patterns
- [ ] Review GitHub token storage (currently plaintext in ~/.openagents)
- [ ] Audit replay bundles before public demo
- [ ] Rate limiting on free trial to prevent abuse

### Scalability Concerns
- [ ] Session logs grow unbounded (need cleanup policy)
- [ ] Replay bundles can be large (need compression)
- [ ] No CDN configured for demo hosting
- [ ] GitHub API rate limits not handled

## Dogfooding Strategy

### Current OpenAgents Repo Status
- 26 of 27 directives complete
- Clean main branch
- Active development
- Suitable for dogfooding

### Proposed Dogfooding Loop
1. Create small, well-scoped issues on OpenAgents
2. Let autopilot claim and work them
3. Review PRs from autopilot
4. Promote successful runs to demos
5. Iterate based on results

### Success Metrics
- **Quality:** >80% of autopilot PRs merged without changes
- **Speed:** <10 minutes per issue on average
- **Reliability:** >90% of runs complete without crashes
- **Demo-Worthiness:** >30% of runs suitable for public demo

## Files Modified This Session

```
crates/autopilot/src/replay.rs              +400 lines (NEW)
crates/autopilot/examples/replay_demo.rs    +80 lines (NEW)
crates/autopilot/src/lib.rs                 +7 lines
crates/autopilot/src/logger.rs              +1 line (Deserialize)
crates/autopilot/src/ci.rs                  -1 line (unused import)
crates/autopilot/Cargo.toml                 +1 line (regex dep)
```

## Commit Hash
Latest: `c4f2bb48d` - Add replay bundle format and publishing pipeline for d-027

## Conclusion

**d-027 Progress:** ~40% complete

**MVP Blockers:**
1. Replay viewer UI (critical)
2. Homepage/landing page (critical)
3. Demo selection workflow (important)

**Recommendation:**
Proceed with **Option C (Hybrid approach)** - build simple web replay viewer using JavaScript/HTML, reuse existing replay bundle format, launch minimal demo page, then iterate.

**Estimated time to MVP demo:** 2-4 days with focused effort on web viewer.

**Next autonomous task:** Implement basic web-based replay viewer in HTML/JS that consumes the replay bundle format.
