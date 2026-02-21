# Inbox Autopilot: WGPUI Desktop UI Spike Proposal

Date: 2026-02-20
Status: Proposed spike
Owner: Inbox Autopilot app + daemon

## Context

Inbox Autopilot currently ships as:

- macOS UI in Swift/SwiftUI (`apps/inbox-autopilot/Inbox Autopilot/Inbox Autopilot/`)
- local Rust daemon (`apps/inbox-autopilot/daemon/`)

We want to explore adding Rust UI surfaces (WGPUI-based) in the desktop product, starting with a narrow test instead of a full UI rewrite.

## Inputs Reviewed

### 1) Feasibility audit

- `docs/audit/WGPUI_IOS_FEASIBILITY_AUDIT_2026-02-20.md`

Key signal from that audit:

- WGPUI is technically viable cross-platform, but not drop-in for host replacement.
- Risk concentrates in host integration, input model, text lifecycle, and packaging glue.
- Correct strategy is staged: shared core + narrow UI spike + decision gates.

### 2) Recent desktop<->Codex integration commits (~15+)

Reviewed commit range around:

- `91bab6c00`, `6bac2ee52`, `4476a7402`, `9ca4e8be9`, `94561cc66`
- `812b0a559`, `ffa291ed5`, `d35608057`, `de4032fb5`, `7cba2493f`, `8fb2390cb`
- `9d176f194`, `33233c5ad`, `088f86930`, `bf2a3cb6b`, `e4d0f95ec`

Files/patterns that matter for this spike:

- Desktop proto adapter and deterministic parsing: `apps/autopilot-desktop/src/runtime_codex_proto.rs`
- Desktop runtime auth/login flow: `apps/autopilot-desktop/src/runtime_auth.rs`
- Rich Rust UI host in production: `apps/autopilot-desktop/src/main.rs`, `crates/autopilot_ui/src/lib.rs`
- iOS reducer + dedupe + stream handling: `apps/autopilot-ios/Autopilot/Autopilot/CodexHandshakeViewModel.swift`

## What We Should Reuse

From those commits, these patterns were consistently useful and should be copied into Inbox Autopilot spike work:

1. Proto-first boundary
- Parse stream/wire data into strict local envelopes before UI mutation.
- Keep tolerant parsing near the edge; keep rendering state deterministic.

2. Deterministic dedupe + cursor handling
- Explicit dedupe keys, sequence tracking, and bounded caches prevent duplicate rows/flicker.
- Cursor rollback/replay logic avoids dropped events after transient failures.

3. Separate transport/auth from UI state
- Auth, stream transport, and UI reducer are separable modules.
- This keeps UI experimentation independent of protocol churn.

4. Acceptance harness before “manual it looks good”
- Add parser/reducer tests first, then run interactive manual checks.
- This prevented regressions during handshake/protocol transitions.

## Recommendation For Inbox Autopilot

Do not start with full SwiftUI replacement.

Start with a narrow Rust-rendered surface test, prove value, then decide on deeper integration.

### Integration options (ranked)

1. Sidecar Rust window (recommended first)
- Swift app remains host.
- Launch a separate Rust process that renders WGPUI and consumes daemon API/events.
- Lowest coupling and fastest proof.

2. In-process embedded Rust view in Swift host
- Requires bridging layer + event loop/lifecycle integration.
- Higher complexity/risk.

3. Full Rust desktop host for Inbox Autopilot
- Largest migration and product risk.
- Only consider after successful sidecar + embed milestones.

## Proposed Spike: “Render One WGPUI Component In Desktop Flow”

### Objective

Render a live Inbox thread-list component in WGPUI using real daemon data, while the existing Swift app and daemon remain intact.

### Scope

In scope:

- One Rust WGPUI process that displays:
  - thread subject
  - sender
  - category/risk/policy chips
  - pending-draft indicator
- Read from `GET /threads` and refresh via `/events/stream`.
- Basic interactions: select thread, refresh, keyboard up/down.

Out of scope:

- OAuth screens
- compose editor parity
- replacing Swift navigation
- sending mail from WGPUI in this spike

### Candidate data contract

- Snapshot: `GET /threads?limit=100`
- Incremental trigger: `GET /events/stream` (SSE)
- Optional detail on selection: `GET /threads/:id`

## Implementation Plan

### Phase A: static render baseline (1 day)

- Create new Rust crate for spike (e.g. `apps/inbox-autopilot/wgpui-spike` or `crates/inbox_autopilot_ui_spike`).
- Use existing `wgpui` + `winit` initialization pattern (same as `apps/autopilot-desktop` / `crates/wgpui/examples/*`).
- Render mocked thread list rows and chip styles.

Exit criteria:

- App launches and renders target component at stable 60fps on macOS.

### Phase B: live daemon data (1-2 days)

- Add daemon client module:
  - snapshot fetch (`/threads`)
  - SSE event listener (`/events/stream`)
- Add proto-first adapter layer:
  - parse daemon events to strict local enum
  - derive minimal update actions

Exit criteria:

- New events trigger deterministic UI updates with no duplicate rows.

### Phase C: host workflow hook (1 day)

- Add a debug action in Swift app to launch/close the spike process.
- Pass daemon base URL/session token via environment or local handoff file.

Exit criteria:

- Operator can open WGPUI spike from Inbox Autopilot build and observe live data.

### Phase D: quality gate + decision (1 day)

- Collect metrics and UX notes.
- Decide: stop, iterate sidecar, or begin embedded bridge POC.

## Success Metrics

Functional:

- WGPUI surface can show at least 100 live threads from daemon.
- Stream updates apply without duplicated entries or missing updates.

Performance:

- Cold start <= 1.5s on dev Mac.
- Average frame time <= 16ms while scrolling thread list.
- Event-to-render latency <= 200ms for simple updates.

Stability:

- 30-minute soak test with periodic syncs and no crash.
- Reconnect behavior after daemon restart is automatic.

## Risks and Mitigations

1. Event-loop/embedding mismatch with Swift host
- Mitigation: sidecar first; avoid in-process embedding until value is proven.

2. Protocol drift causes UI breakage
- Mitigation: explicit adapter structs + parser tests (copy desktop proto-first pattern).

3. Duplicate or out-of-order events
- Mitigation: dedupe keys + sequence watermarks + bounded caches.

4. Engineering overhead without product gain
- Mitigation: strict Phase D decision gate tied to measurable outcomes.

## Decision Gates

Gate 1 (after Phase A):

- If we cannot render a stable basic component quickly, stop.

Gate 2 (after Phase B):

- If live updates are noisy/fragile, keep Swift UI and only share Rust logic.

Gate 3 (after Phase D):

- Only proceed to embedded bridge POC if metrics and UX are clearly favorable.

## Concrete Next Steps

1. Validate local WGPUI desktop baseline still works:
- `cargo run -p wgpui --example component_showcase --no-default-features --features desktop`

2. Scaffold a spike crate and static `ThreadList` component.

3. Implement daemon `/threads` + `/events/stream` adapter with tests.

4. Add a launch toggle in Inbox Autopilot Settings (debug-only) to open the spike window.

5. Run a 30-minute soak and record numbers for Gate 3.

## Bottom Line

Yes, we should test more Rust UI here, but with a contained sidecar spike first. The recent desktop<->Codex commits show the right approach: strict protocol adapters, deterministic reducers, and hard acceptance gates. If this spike proves fast, stable, and genuinely better, then we can justify an embedded Rust UI path for selected Inbox Autopilot surfaces.
