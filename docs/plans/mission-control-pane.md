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

## Implementation Notes (for MVP)

- Align with `docs/MVP.md` and `docs/OWNERSHIP.md`.
- Wallet and payout state must be explicit and truthful in UI and behavior.
- Sync and state continuity should remain deterministic and replay-safe.
- **Balance display:** Implement BIP 177 integer view (₿) as default; legacy BTC view is a UI toggle only (one field/label change). No consensus or backend change; same on-chain integers throughout.
