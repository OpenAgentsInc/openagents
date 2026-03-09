# Mission Control Pane

Documentation of the Mission Control UI based on the current design. This describes layout, sections, copy, and behavior visible in the screenshot.

---

## Balance & amount display (BIP 177)

All Bitcoin amounts in Mission Control follow **BIP 177**: the protocol uses whole integers on-chain; the decimal is a display convention only. By default we show the **integer view** so amounts are simpler and match how Bitcoin actually works.

| Convention | Description |
|------------|-------------|
| **Default** | Integer view: **₿** (e.g. ₿ 2698437). One unit = one on-chain integer (legacy: 1 sat). No decimal. |
| **Legacy** | Decimal view: **BTC** (e.g. 0.02698437 BTC). 1 BTC = 100 000 000 integer units. |
| **Toggle** | One-click in-app toggle: Integer (₿) vs Legacy (BTC). Internally everything remains integers; only the label and formatting change. |

- Use the **₿** symbol and whole numbers for the primary display (wallet balance, earnings, job bounties).
- Provide a **Legacy (BTC)** option for users who prefer the decimal view; no consensus or backend change required.
- Keeps wallet and payout state explicit and truthful; no change to underlying data.

---

## High-Level Overview

Mission Control is a dark-themed dashboard with neon green/teal accents. It is used to sell compute, view earnings, manage wallet state, and monitor status via a log stream. The example state shown is **OFFLINE**.

---

## Header Bar

| Element | Details |
|--------|---------|
| **Title** | "MISSION CONTROL" — top-left, monospace, neon green/teal |
| **Status** | "STATUS: OFFLINE" — top-right; "OFFLINE" in yellow-orange |
| **Close** | 'X' icon on the far right (dismiss/close) |

---

## Left Column

### // SELL COMPUTE

- **Primary CTA:** "GO ONLINE" button
  - Glowing neon green/teal border and text
  - Used to start selling compute

### // EARNINGS

Cryptocurrency earnings (BIP 177 integer view by default):

| Period      | Value (₿)   | Legacy (BTC) equivalent |
|-------------|-------------|---------------------------|
| Today       | ₿ 6 284     | 0.00006284 BTC           |
| This Month  | ₿ 56 439    | 0.00056439 BTC           |
| All Time    | ₿ 236 579   | 0.00236579 BTC           |

- "All Time" is emphasized (brighter neon green/teal).
- One-click toggle available: Integer (₿) vs Legacy (BTC).

### // WALLET

| Field    | Example value (default) | Legacy view      |
|----------|-------------------------|------------------|
| Status   | Connected               | —                |
| Address  | 2836******9aj2          | —                |
| Balance  | ₿ 2 698 437             | 0.02698437 BTC   |

- **Default:** Balance in BIP 177 integer form (₿), whole numbers, no decimal.
- One-click toggle: Integer (₿) vs Legacy (BTC).
- Address is partially masked for privacy.

### Bottom Actions (Left Column)

- **DOWNLOAD GPT-1234** — download client/model version (e.g. for local compute or integration).
- **DOCUMENTATION** — link to help/manuals for Mission Control.

---

## Right Column

### // ACTIVE JOBS

- Section for currently running jobs.
- When offline: message **"Go Online to Start Jobs."**
- Reinforces that no jobs run until status is online.

### // LOG STREAM

A single pane on the right (roughly two-thirds of the right column) used only for a **log stream**.

- **Purpose:** Display dynamic status output: log lines, status updates, notifications. No table, no job list—just a scrolling stream of text.
- **Layout:** Large rectangular area with a clear border; most of the space is for the stream. Content flows from top (e.g. latest at bottom or top, per product choice).
- **Example content when idle:** e.g. "No local model found." or similar status line.
- **Style:** Same dark theme and monospace as the rest of Mission Control; readable, minimal chrome so the log is the focus.

---

## Visual Styling

| Aspect | Description |
|--------|-------------|
| **Background** | Dark gray/black |
| **Accents** | Neon green/teal; yellow-orange for "OFFLINE" |
| **Font** | Monospace, technical/code-editor feel |
| **Layout** | Two main columns; left = controls/summary, right = larger log-stream pane; rounded panels/cards |
| **Buttons** | Clear borders and labels; "GO ONLINE" has a glow |

---

## Summary of Copy and Labels

- **Title:** MISSION CONTROL
- **Status label:** STATUS: OFFLINE
- **Section labels:** // SELL COMPUTE, // EARNINGS, // WALLET, // ACTIVE JOBS, // LOG STREAM
- **CTAs:** GO ONLINE, DOWNLOAD GPT-1234, DOCUMENTATION
- **Empty state:** Go Online to Start Jobs.
- **Right pane:** LOG STREAM — single pane for log/status stream only (no job table).
- **Amount display:** BIP 177 integer (₿) by default; one-click toggle to Legacy (BTC).

---

---

## Audit: Panes in WGPUI and Autopilot-Desktop

### WGPUI (crates/wgpui) — reusable pane and section components

| Location | Component | Purpose |
|----------|-----------|---------|
| **HUD** (`components/hud/`) | `PaneFrame` | Titled pane chrome: title, optional close (X), `content_bounds` below title bar. Active vs default border; glow when active. |
| **HUD** | `ResizablePane` | Wrapper with optional edge/corner resize handles; min/max size; `on_resize` callback; optional background/border. |
| **Sections** (`components/sections/`) | `TerminalPane` | Scrollable log: `TerminalLine` (stream: Stdout/Stderr, text), `push_line()`, `clear()`, `auto_scroll`, max_lines, line_height. Mono font, clip, scrollbar. |
| **Sections** | `MetricsPane` | Structured metrics (APM, queue, usage, last PR). Not used for Mission Control. |
| **Sections** | `CodePane`, `ThreadView`, `ThreadHeader`, `MessageEditor`, `TrajectoryView` | Chat/code/thread UI. Not used for Mission Control. |

**Primitives (used inside panes):** `Button`, `Text`, `TextInput`, `ScrollView`, `Tabs`, `Modal`, `Div`, `Dropdown`, `VirtualList` (see `crates/wgpui/src/components/mod.rs`).

**App ownership:** `docs/OWNERSHIP.md` — pane orchestration and product behavior live in `apps/autopilot-desktop`; wgpui provides product-agnostic UI only.

### Autopilot-Desktop — pane kinds and Mission Control mapping

- **Pane registry** (`pane_registry.rs`): Each pane has a `PaneSpec` (kind, title, default size, singleton, startup, command). Mission Control is **`PaneKind::GoOnline`**: title "Mission Control", id `pane.mission_control`, startup pane, singleton.
- **Pane chrome:** Every floating pane uses a shared `PaneFrame` per pane (in `DesktopPane.frame`); `pane_system.rs` paints `pane.frame.paint(pane.bounds, paint)` then content via `PaneRenderer` with `content_bounds = pane_content_bounds(pane.bounds)`.
- **Content dispatch:** `pane_renderer.rs` switches on `pane.kind`; `PaneKind::GoOnline` → `paint_go_online_pane(content_bounds, provider_runtime, …)`.
- **Current GoOnline layout:** Custom paint only. Three cards: left = "Provider Rig", right = "Wallet + First Earnings", bottom = "Job Flow". No wgpui `TerminalPane` or `Button` used yet; all quads and text via `paint.scene` / `paint.text`.

**Other relevant panes (for reference):** ProviderStatus, EarningsScoreboard, JobInbox, ActiveJob, JobHistory, SparkWallet, LocalInference, Settings, Credentials, etc. (40+ `PaneKind` variants in `app_state.rs`).

---

## Relationship to current Mission Control pane — change in place

**Decision: change the existing Mission Control pane in place.** Do not add a new pane or a second “Mission Control” surface. The same `PaneKind::GoOnline` pane stays the single earn-first shell; we refactor its content layout and replace the Job Flow card with a log stream.

**Current implementation (as shipped):**

- **Chrome:** Title “Mission Control”, subtitle “Earn-first shell for provider state, wallet truth, and job flow.” Buttons/tabs: “Go Online”, “Mission Control”.
- **Layout:** Three cards in one view:
  - **Left card — “Provider Rig”:** Lane, Backend, Control, Projection, Settlement, Mode, Local inference, Apple FM, Serving model, Preflight clear (or blocker list).
  - **Right card — “Wallet + First Earnings”:** Wallet (β0 / amount), Wallet status, Today, Lifetime, Jobs today, First earnings progression (milestone + progress bar).
  - **Bottom card — “Job Flow”:** Active job line (“No active job yet”), then rows of job previews (e.g. OPEN, amount, nip90 kind, id, preview/accept).
- **Data:** Same sources (provider_runtime, spark_wallet, earnings_scoreboard, job_inbox, active_job). Amounts already shown in integer style (e.g. β0) in some builds.

**Target design (from this doc):**

- **Chrome:** “MISSION CONTROL” title, STATUS: OFFLINE (or online) in header, close (X). Optional subtitle; “Go Online” as the primary CTA in content, not a tab.
- **Layout:** Two columns, not three cards:
  - **Left column:** // SELL COMPUTE (GO ONLINE button), // EARNINGS (Today / This Month / All Time in ₿), // WALLET (status, masked address, balance in ₿), DOWNLOAD GPT-1234, DOCUMENTATION.
  - **Right column:** // ACTIVE JOBS (one-line summary or “Go Online to Start Jobs.”), then **// LOG STREAM** — a single scrolling log pane (e.g. TerminalPane), replacing the Job Flow card’s job rows. Status lines like “No local model found.” or provider/job events go here.

**What to reuse vs replace:**

| Current | Target | Action |
|--------|--------|--------|
| Provider Rig card | Left column “provider” state | Move or condense into left column or a compact block (e.g. mode + preflight); keep same data. |
| Wallet + First Earnings card | // EARNINGS + // WALLET in left column | Reuse data; restate as section labels and BIP 177 ₿; keep first-earnings milestone. |
| Job Flow card (active job + job rows) | ACTIVE JOBS one-liner + LOG STREAM | Replace job list with log stream pane; keep “Active job: …” summary line only. |
| Go Online as tab/button | GO ONLINE as primary CTA in left column | Single prominent button in // SELL COMPUTE. |
| Single pane (GoOnline) | Same | No new pane; same `paint_go_online_pane` entry point, refactored layout. |

So the plan is a **refactor of the existing Mission Control pane**: same pane kind, same data, new two-column layout and a dedicated log stream instead of the current Job Flow card.

---

## Backend and data flow — Nostr, relays, real jobs

**Confirmed:** The Mission Control pane is already backed by real backend functionality. Jobs are not mock-only.

1. **Nostr NIP-90 and relays**
   - Job requests are NIP-90 events (kind 5000–5999) on Nostr. The app uses `ProviderNip90LaneWorker` (`provider_nip90_lane.rs`), which connects to the configured relays (e.g. from settings), subscribes with `autopilot-provider-nip90-ingress` and filters for job request kinds, and receives events from the relay pool.
   - Each received `Event` is converted to `JobInboxNetworkRequest` via `event_to_inbox_request(event)` (NIP-90 parsing, capability, price_sats, etc.).

2. **Ingress into app state**
   - The worker emits `ProviderNip90LaneUpdate::IngressedRequest(request)`. The input reducer (`input/reducers/mod.rs`) calls `provider_ingress::apply_ingressed_request(state, request)`, which upserts into `state.job_inbox.requests` and updates `job_inbox.last_action` (e.g. "Observed preview" when offline, "Ingested live" when online).

3. **What Mission Control shows**
   - The current Mission Control pane (GoOnline) paints from the same `job_inbox` and `active_job`: the "Job Flow" card uses `job_inbox.requests` (e.g. `recent_requests = job_inbox.requests.iter().rev().take(5)`). So **the jobs listed are real jobs from relays** (or from starter demand — see below), not a separate mock list.

4. **Preview vs online**
   - When the provider is **offline**, the lane can still be in Preview/Connecting: it stays subscribed to relays and ingresses events, but they are shown as "preview" (not claimable). When the user clicks **Go Online**, the provider goes online and new/updated jobs become claimable; the same `job_inbox` continues to be fed from the same relay ingress.

5. **Starter demand (optional)**
   - Jobs can also come from **StarterDemand** (local simulator via `OPENAGENTS_ENABLE_LOCAL_STARTER_DEMAND_SIMULATOR` or hosted starter demand on the OpenAgents Nexus relay). Those are also upserted into `job_inbox` and appear in the same list.

**Conclusion:** Backend functionality for Nostr and relays is in place; Mission Control already displays real jobs from relays (and optional starter demand). The planned refactor (log stream instead of job list in the right column) does **not** change where jobs come from or how they are ingested; it only changes how we present the right-hand area (log stream vs job rows). The "Active job" summary and any job-related log lines we feed into the new log stream will still reflect the same `active_job` and `job_inbox` state.

---

## Implementation plan: Mission Control pane (grounded in existing components)

Goal: Refactor the existing Mission Control pane (`paint_go_online_pane`) to the design above: left column = SELL COMPUTE / EARNINGS / WALLET / actions; right column = ACTIVE JOBS summary + LOG STREAM (wgpui TerminalPane). Replace the current three-card “Job Flow” area with the log stream; keep using existing components and pane system.

### 1. Pane and chrome (unchanged)

- **Pane:** Keep using `PaneKind::GoOnline` and existing `PaneSpec` (Mission Control, singleton, startup). No new `PaneKind`.
- **Chrome:** Keep using `PaneFrame` for title + close; title can be set to "MISSION CONTROL", status (e.g. OFFLINE) in header or first content row.

### 2. Layout within `paint_go_online_pane`

- **Left column** (~1/3 width): // SELL COMPUTE (GO ONLINE button), // EARNINGS (Today / This Month / All Time in ₿), // WALLET (status, address, balance in ₿), DOWNLOAD GPT-1234, DOCUMENTATION. Use wgpui `Button` for actions; label/value lines with existing paint helpers; BIP 177 integer view by default.
- **Right column** (remainder): // ACTIVE JOBS summary line or "Go Online to Start Jobs."; **// LOG STREAM** — single pane using **wgpui `TerminalPane`**.

### 3. Log stream with `TerminalPane`

- **Component:** `wgpui::components::sections::{TerminalPane, TerminalLine, TerminalStream}` — scrollable mono log, `push_line()`, `clear()`, `auto_scroll`, max_lines.
- **State:** Add mission control log buffer in app state (e.g. `Vec<TerminalLine>` or state holding a `TerminalPane`). Feed lines from provider status ("No local model found.", blockers, mode changes), download progress, job lifecycle.
- **Rendering:** In `paint_go_online_pane`, compute `log_stream_bounds` (right column, below ACTIVE JOBS); call `terminal_pane.paint(log_stream_bounds, paint)` (or equivalent from state).
- **Events:** In `pane_system.rs`, when hit is inside log stream bounds, route scroll/mouse to TerminalPane so scroll works.

### 4. BIP 177 and copy

- Amounts: integer (₿) strings via existing formatting; optional legacy BTC toggle in state. No backend change.
- Copy: section labels and CTAs from "Summary of Copy and Labels" in this doc.

### 5. Files to touch (summary)

| Area | Change |
|------|--------|
| `app_state.rs` | Add mission control log state (TerminalLine buffer or TerminalPane). Optional BIP 177 legacy toggle. |
| `pane_renderer.rs` | Refactor `paint_go_online_pane`: two-column layout; left = GO ONLINE + earnings + wallet + actions; right = active job + TerminalPane log stream. Replace current three-card Job Flow with right-column log. |
| `pane_system.rs` | Hit-test and event routing for log stream bounds; forward scroll/events to TerminalPane. |
| `pane_registry.rs` | No change. |
| `crates/wgpui` | No change; use existing PaneFrame, Button, TerminalPane, TerminalLine, TerminalStream. |

### 6. Validation

- Mission Control opens as startup pane; header "MISSION CONTROL" and status.
- Left: GO ONLINE, earnings (₿), wallet (₿), download/docs.
- Right: active job summary + scrolling log (e.g. "No local model found." when idle).
- Wallet and payout state explicit and truthful; sync/replay unchanged per MVP.

---

## Implementation Notes (for MVP)

- Align with `docs/MVP.md` and `docs/OWNERSHIP.md`.
- Wallet and payout state must be explicit and truthful in UI and behavior.
- Sync and state continuity should remain deterministic and replay-safe.
- **Balance display:** Implement BIP 177 integer view (₿) as default; legacy BTC view is a UI toggle only (one field/label change). No consensus or backend change; same on-chain integers throughout.
