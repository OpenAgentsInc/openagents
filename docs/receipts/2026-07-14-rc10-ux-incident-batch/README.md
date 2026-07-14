# rc.10 owner incident batch receipts (#8787 / #8788 / #8789)

Built-Electron smoke receipts for the 2026-07-14 rc.10 first-run regression
batch, captured from the packaged smoke journey
(`OPENAGENTS_DESKTOP_SMOKE=1`, fixture `~/.codex`):

- `01-shell.png` — the shell at open. The composer holds keyboard focus at
  shell-interactable and after hydration; the paired step receipts (below)
  record `focusedAtMount`/`focusedAfterHydration` and a real Chromium
  keystroke landing as typed text (#8787).
- `13-session-search-filtered.png` — sidebar session search actually
  filtering: query `cut-02 verif` shows `SEARCH · 1 RESULT` with the matching
  fixture session (#8788).
- `03-codex-history-detail.png` — the truthful sidebar header
  `CODING HISTORY · ALL 1` (counted disclosure; the untrue `ALL TIME` claim is
  gone) with a loaded history detail (#8789).
- `smoke-step-receipts.log` — the exact smoke step JSON lines for the focus,
  keystroke, header, and search oracles.

Root cause receipt for #8789/#8788 (real store, this machine): the catalog
graph build previously `readFileSync`'d whole rollouts for titles and ENOMEMed
on a 4.5 GB rollout in a 20 GB / 1,582-session `~/.codex`, collapsing the
sidebar to the 24-hour list. With the byte-bounded streaming reads the same
store builds 1,289 roots from 1,582 sessions in ~1.9 s at ~438 MB RSS.

Contracts: `openagents_desktop.composer.focused_on_open.v1`,
`openagents_desktop.history.session_search_filters.v1`,
`openagents_desktop.history.sidebar_header_truthful_scope.v1`
(`apps/openagents-desktop/src/contracts/ux-contracts.ts`).
