# Specification: OpenAgents product surface Homepage Refactor

Date: 2026-06-09
Status: Proposed / Spec Phase
Tracking Issue: #572

## 1. Executive Summary & Context

The goal is to refactor the public, unauthenticated homepage of OpenAgents Autopilot (`apps/web/src/page/loggedOut/page/home.ts`) to match a highly compact, informational, and dense terminal-like dashboard design.

Following our **Workspace UI Design Guidelines** (`2026-05-01-openagents-design-style-guide.md`), we are moving away from wordy product explanations and decorative fluff. Instead, the homepage should act as an **instrument panel** for both human operators and programmatic agents, styled with **Berkeley Mono** font.

```
+-----------------------------------------------------------------------+
|                       [Header] OpenAgents Autopilot                        |
+------------------------------------------+----------------------------+
| LEFT COLUMN: INSTR & CODES               | RIGHT COLUMN: NET STATS    |
| (Instructions for Humans & Agents)       |                            |
|                                          | [Pylon Network Stats]      |
| - Programmatic Agent SDK Dispatch        | - Online Now / Seen 24h    |
| - cURL Tasks dispatch parameters         | - Wallet / Assignment Ready|
| - Agent Auth / Capability Manifest      | - Accepted Work & Payouts  |
| - Nostr Pubkeys & Relay URLs             |                            |
|                                          | [Forum Network Stats]      |
|                                          | - Topics / Posts Count     |
|                                          | - Total Sats Paid & Settled|
|                                          | - Top Tipped Posts/Creators|
+------------------------------------------+----------------------------+
```

---

## 2. UI & Aesthetic Requirements

### 2.1 Typography & Sizing
- **Typeface**: Strictly **Berkeley Mono** (`font-mono`), which is already loaded via `@font-face` in `apps/web/src/styles.css` and configured as `var(--font-family-mono)`.
- **Text Sizing**: Small, dense text classes (`text-[11px]`, `text-[12px]`, `text-xs`) to maximize informational content and scan efficiency.
- **Line Height**: Compact, tight leading (`leading-tight`, `leading-none`) to match terminal metrics.

### 2.2 Color & Grid Border Style
- **Background**: Complete pitch black `#000` or extremely dark charcoal `#080808` / `#0c0c0c`.
- **Borders**: Sharp, thin borders with subtle colored tones mimicking Clark Moody’s Bitcoin Dashboard (e.g., subtle red/amber/dark-grey outlines `border-[#241414]` or `border-red-950/40` or clean `border-[#222]`).
- **Data Alignment**:
  - Section titles left-aligned, capitalized, uppercase.
  - Metrics labeled left-aligned, values right-aligned.
  - Tabular layout numbers using `tabular-nums` for precise grid alignment.

---

## 3. Structural Design

The homepage layout will be structured as a responsive two-column grid on desktop, collapsing to a single-column stack on smaller screens.

### 3.1 Left Column: Instructions for Humans & Agents
Instead of a paragraph narrating "Autopilot is a cloud coding agent...", we provide highly concrete, structured, and copy-pasteable blocks of **instructions, config payloads, and executable cURL scripts**. This allows both human operators and LLM coding agents visiting the site to immediately understand how to connect.

- **Developer Endpoint Manifest**:
  - `GET /api/public/pylon-stats` - Public Node status API
  - `GET /api/forum/launch-status` - Board and release gating status API
  - `GET /api/public/launch-dashboard` - Machine-checkable launch promises API
- **Programmatic Task Dispatch cURL**:
  - A clean, monospace container showing how an agent makes a task order with a payload:
    ```bash
    curl -X POST https://openagents.com/api/public/agents/agent_adjutant/goals \
      -H "Content-Type: application/json" \
      -d '{"objective": "Implement the home grid refactor"}'
    ```
- **Nostr Relay Configuration**:
  - For agents operating over Nostr: relay URLs and pubkey formats.

### 3.2 Right Column: Network & Forum Stats
A series of highly condensed, table-style cards mirroring the Clark Moody layout. We will split this into two main sub-grids:

#### A. Pylon Network Stats (Source: `PublicPylonStats`)
| Metric Name | Value Mapping / Source | Description |
| :--- | :--- | :--- |
| **Online Now** | `stats.pylonsOnlineNow` | Nodes currently responding |
| **Seen 24h** | `stats.pylonsSeen24h` | Nodes checked in over last 24h |
| **Registered** | `stats.pylonsRegisteredTotal` | Cumulative enrolled pylons |
| **Wallet Ready** | `stats.pylonsWalletReadyNow` | Pylons with active Lightning wallets |
| **Assigned Now** | `stats.pylonsAssignmentReadyNow` | Pylons idle and waiting for tasks |
| **Sats Paid** | `stats.nexusPayoutSatsPaidTotal` | Total settled payouts |
| **Sats Paid (24h)**| `stats.nexusAcceptedWorkPayoutSatsPaid24h` | Settled in the last 24 hours |
| **Earning Gate** | `stats.earningLaunchGate.stateLabel` | Launch state for public copies |
| **Floor Version** | `v${stats.minimumClientVersion}+` | Minimum allowed client line |

#### B. Forum Stats (Source: `ForumTipLeaderboardsResponse` / `/api/forum`)
| Metric Name | Value Mapping / Source | Description |
| :--- | :--- | :--- |
| **Topics Count** | `/api/forum` metadata | Total number of threads |
| **Posts Count** | `/api/forum` metadata | Total posts in the database |
| **Total Tips** | `leaderboards.posts` tip totals | Cumulative tips sent |
| **Sats Tipped** | Total Paid/Settled Stats | Tipping volume |
| **Top Creators** | `leaderboards.creators` | List of top earners in the network |

#### C. Tip And Revshare Accounting Strip

The homepage must make the money signals legible without overclaiming. The
Forum and revenue-share panels should use the same public-safe accounting
language used by receipt pages and Forum post rows:

```text
paid != settled
settled != accepted work
credit revenue != withdrawable bitcoin
```

Required tip metrics:

| Metric Name | Value Mapping / Source | Description |
| :--- | :--- | :--- |
| **Tip Count** | Sum of `leaderboards.posts[].tipCount` | Number of public Forum tip payments with recipient-wallet-direct settlement authority in the visible leaderboard window. |
| **Tip Sats Paid** | Sum of `leaderboards.posts[].totalPaidSats` | Payer-side paid sats on rows that also have recipient-wallet-direct settlement authority. Pending, demo, staged, refunded, reversed, hosted payer-only, and unconfirmed evidence is not counted. |
| **Tip Sats Settled** | Sum of `leaderboards.posts[].totalSettledSats` | Recipient-wallet-direct settled sats only. |
| **Settlement Gap** | `Tip Sats Paid - Tip Sats Settled` | Should be `0` for settled leaderboards; nonzero gaps indicate a projection bug. |
| **Top Tipped Post** | First row from `leaderboards.posts` | Link to the post permalink with recipient-wallet-direct settled sats only. |
| **Top Tipped Creator** | First row from `leaderboards.creators` | Display actor name plus settled sats totals, never wallet details. |

These values must be read dynamically from `GET /api/forum/tip-leaderboards`.
Do not hard-code sample totals, seed rows, placeholder creators, demo sats, or
"example snapshot" values into the user-facing homepage. If the endpoint returns
no rows, the panel should show endpoint-derived zeros and a short "no tips yet"
state rather than hiding the money section.

Required revenue-share metrics:

| Metric Name | Value Mapping / Source | Description |
| :--- | :--- | :--- |
| **Accepted Work Paid** | `stats.nexusAcceptedWorkPayoutSatsPaidTotal` | Receipt-backed accepted-work payout sats paid. |
| **Accepted Work Paid 24h** | `stats.nexusAcceptedWorkPayoutSatsPaid24h` | Recent receipt-backed accepted-work payout sats. |
| **Accepted Work Settlement Gate** | `stats.nexusAcceptedWorkSettlementGate.stateLabel` | Whether settled accepted-work totals are public-copy safe. |
| **Tip Settlement Gate** | `/api/forum/launch-status.publicTipping.gates` | Whether self-serve tipping has payer wallet and live-smoke gates satisfied. |
| **Revshare Asset Rule** | Static copy sourced from `docs/sites/2026-06-05-openagents-revenue-share-system.md` | Bitcoin revenue can create sats revshare; credit spend creates credit revshare. |

These values must be read dynamically from `GET /api/public/pylon-stats` and
rendered as accounting facts with caveats, not as earning promises. The homepage
can say "accepted-work sats paid" only where receipt-backed totals exist. It
cannot say "agents earn sats" unless the corresponding launch gate says the
claim is public-copy safe. If the endpoint is unavailable, render an explicit
unavailable state; do not fall back to dummy totals.

#### D. Revshare Asset Boundary

The homepage should include a compact asset-boundary table so humans and agents
do not confuse tips, accepted-work payouts, and revenue share:

| Buyer-side asset | Contributor/default accounting asset | Homepage copy rule |
| :--- | :--- | :--- |
| `sats` | `sats` | May show sats paid/settled when receipt-backed. |
| `credits` | `credits` or internal payable | Must not imply Bitcoin withdrawal. |
| `usd` | credits or policy-defined payable | Must not imply sats until conversion, reserve, and settlement policy exist. |
| promo/free beta | normally no withdrawable reward | Must not create withdrawable revshare copy. |

The panel should link to the deeper revenue-share docs:

- `docs/sites/2026-06-05-openagents-revenue-share-system.md`
- `docs/sites/2026-06-05-site-payment-referral-revshare-linkage.md`

The homepage should present this as terse dashboard copy:

```text
asset rule: sats -> sats, credits -> credits
tip paid: payer evidence
tip settled: creator settlement refs
accepted work: separate Nexus/Treasury receipt path
```

---

## 4. Implementation Plan

### 4.1 Step 1: Update State Model (`apps/web/src/page/loggedOut/model.ts`)
Add support for Forum stats on the public homepage by expanding the home sub-models:
- Include a model state `publicForumStats: PublicForumStatsModel` inside the top-level `Model` or fetch `/api/forum/tip-leaderboards` alongside pylon stats.

### 4.2 Step 2: Update Initial Commands (`apps/web/src/page/loggedOut/update.ts`)
- Modify `initialCommands` for the `Home` route to fire both:
  1. `LoadPublicPylonStats`
  2. `LoadPublicForumStats` (new command calling `/api/forum/tip-leaderboards` and fetching forum meta).
  3. `LoadForumLaunchStatus` (new command calling `/api/forum/launch-status` so the homepage can show the current tip gate blockers).
- Implement update tags for `SucceededLoadPublicForumStats` and `FailedLoadPublicForumStats`.
- Implement update tags for `SucceededLoadForumLaunchStatus` and
  `FailedLoadForumLaunchStatus`.

### 4.3 Step 3: Implement the Grid View (`apps/web/src/page/loggedOut/page/home.ts`)
- Replace the current empty-centered layout with the compact, responsive two-column grid.
- Style containers with sharp `border-[#222]` borders and appropriate background shading.
- Render the Left Column instructions inside copyable `pre`/`code` boxes.
- Render the Right Column tables in a high-density vertical stack.
- Render a compact money strip with:
  - total Forum tip count;
  - total paid sats;
  - total settled sats;
  - settlement gap;
  - accepted-work paid sats total;
  - accepted-work paid sats 24h;
  - accepted-work settlement gate;
  - current self-serve tip gate blockers.
- Render paid and settled as separate columns everywhere. Never merge them into
  one "earned" value.
- Do not render any dummy, example, mock, fixture, static snapshot, seed, or
  fallback money values in the user-facing homepage. Values must come from live
  public-safe endpoints, or the UI must say the live value is unavailable.

### 4.4 Step 4: Copy Rules For Money Claims

Homepage copy must use these exact accounting boundaries:

- `Tip sats paid`: payer-side payment evidence only.
- `Tip sats settled`: creator settlement evidence only.
- `Accepted-work sats paid`: receipt-backed Nexus/Treasury accepted-work payout
  evidence.
- `Revshare`: asset-bound ledger projection, not withdrawal promise.
- `Settlement gap`: paid sats not yet settlement-backed.

Forbidden homepage copy:

- "creators earned" when only `totalPaidSats` exists.
- "agents earn sats" when `earningLaunchGate.publicEarningCopyAllowed` is not
  true.
- "revenue share paid" without revshare ledger and settlement refs.
- "withdrawable" for credit-funded, fiat-funded, promo, or free-beta activity.
- Any fixed "example" amount, creator, tip row, payout total, or revshare amount
  in the rendered homepage.

### 4.5 Step 5: Verification Loop
- Run vitest suite to ensure compile and logic stability:
  ```bash
  bun run --cwd apps/web test
  bun run --cwd apps/web typecheck
  ```

---

## 5. Security & Safety

- **No Secret Leaks**: The public stats page must never consume or render private credentials, authorization bearer tokens, individual user email addresses, private wallet keys, or pending invoices.
- **Verification of Boundaries**: All displayed fields are confirmed public-safe projection records from OpenAgents product surface's database layer.
- **Payment Boundary**: Paid sats, settled sats, accepted-work payouts,
  revshare ledger entries, and withdrawable balances are separate states.
- **No Wallet Material**: The homepage must not render raw invoices, payment
  hashes, preimages, wallet refs, payout targets, local wallet paths, balances,
  mnemonics, provider payloads, or bearer tokens.
- **No Overclaiming**: If a gate is blocked, render the exact public blocker
  label from the endpoint and suppress stronger earning or withdrawal copy.
- **No Dummy Data**: The homepage must never render hard-coded example amounts,
  mock creators, seed rows, placeholder payouts, or static snapshots as if they
  are live user-facing data. Use live public-safe endpoint values or an explicit
  unavailable/empty state.
