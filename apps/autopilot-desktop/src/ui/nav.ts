// Nav shell + command registry — sub-EPIC #5462 (children #5463/#5464/#5465).
//
// THE PANE-REGISTRATION SEAM (read this before adding a pane — for #5466–#5472)
// ───────────────────────────────────────────────────────────────────────────
// This file is the SINGLE place the navigation shell, the Cmd-K command palette,
// and the keyboard layer read from. The desktop's central Foldkit files
// (model.ts / update.ts / view.ts / message.ts) deliberately know NOTHING about
// which destinations exist — they just route the `PaneId` the registry hands
// them. That keeps the shell uncluttered AND lets each Phase-2 connection issue
// plug its pane/capability in with MINIMAL edits to the hot central files.
//
// To add a destination (a new pane in an existing group), a Phase-2 agent does
// exactly this:
//   1. Add the `PaneId` literal in model.ts (the closed union) + its
//      `case` in `paneView` (view.ts) — its own pane module renders there.
//   2. Add ONE `NavDestination` entry to the right group's `destinations`
//      array below. That single entry makes it appear in:
//        - the secondary in-section strip (grouped sidebar, #5463),
//        - the Cmd-K palette as a "Go to …" command (#5464),
//        - and j/k navigation where applicable.
//   No edits to the sidebar renderer, palette renderer, or keyboard layer are
//   needed — they are all driven by `NAV_GROUPS` + `paletteCommands`.
//
// To add an ACTION command to the palette (run a thing, not navigate), add one
// entry to `actionCommands` below, mapping it to an EXISTING Foldkit `Message`.
// The palette dispatches that message verbatim; no new control/RPC verb.
//
// Anti-clutter rule (audit §5.2): a new system NEVER gets a new top-level
// sidebar button. It joins a group's `destinations` (a secondary entry) or it
// becomes a palette command. The primary sidebar stays at the group count (~5).

import type { PaneId } from "./model"

// A leaf destination inside a group: the pane it opens + how it's labelled.
// `keywords` widen the palette's fuzzy match without changing the label.
export type NavDestination = Readonly<{
  pane: PaneId
  label: string
  keywords?: ReadonlyArray<string>
}>

// One of the ~5 primary, intent-named groups. `accel` is the Cmd-<n> index
// (#5465). `defaultPane` is where clicking the group header lands.
export type NavGroup = Readonly<{
  id: string
  label: string
  accel: number
  defaultPane: PaneId
  destinations: ReadonlyArray<NavDestination>
}>

// ── The ~5 grouped destinations (#5463; audit §5.2) ─────────────────────────
// Every existing pane stays reachable. `session-detail`, `onboarding`,
// `builtin-agent`, `nodes`, and `training-fullscreen` are reachable as
// destinations or leaves (see NAV_LEAF_PANES); none are orphaned.
export const NAV_GROUPS: ReadonlyArray<NavGroup> = [
  {
    id: "chat",
    label: "Chat",
    accel: 1,
    // The default post-onboarding home (audit §5.2; onboarding→chat auto-advance
    // in update.ts is preserved).
    defaultPane: "chat",
    destinations: [{ pane: "chat", label: "Chat", keywords: ["blueprint", "talk", "ask"] }],
  },
  {
    id: "code",
    label: "Code",
    accel: 2,
    defaultPane: "composer",
    destinations: [
      { pane: "composer", label: "Composer", keywords: ["code", "edit", "cli"] },
      { pane: "swarm", label: "Swarm", keywords: ["grid", "concurrent", "batch", "fanout"] },
      { pane: "sessions", label: "Sessions", keywords: ["runs", "history"] },
      { pane: "spawn", label: "Spawn", keywords: ["new", "launch", "start"] },
    ],
  },
  {
    id: "supervise",
    label: "Supervise",
    accel: 3,
    defaultPane: "decisions",
    destinations: [
      {
        pane: "decisions",
        label: "Decisions",
        keywords: ["approvals", "approve", "deny", "auto-approve", "intent"],
      },
      // #5467: the autonomous coordinator loop is now a first-class view (its own
      // pane module, autonomous-loop-pane.ts) — intent → plan → fanout →
      // reconcile → ship gate, read-only over intent.list + coordinator.status,
      // reusing the existing pause/resume. A secondary entry in Supervise, NOT a
      // new top-level button (audit §5.2).
      {
        pane: "autonomous-loop",
        label: "Autonomous loop",
        keywords: ["autonomous", "loop", "intent", "coordinator", "afk", "fanout", "ship", "plan"],
      },
      // Accounts/managed accounts + the Ask card live in the `nodes` pane today
      // (view.ts accountsSection / askCard). Surfaced here so the Supervise group
      // covers accounts without a new top-level button. #5468/#5470 deepen these
      // in place.
      { pane: "nodes", label: "Accounts", keywords: ["accounts", "providers", "node", "wallet", "deploy"] },
    ],
  },
  {
    id: "explore",
    label: "Explore",
    accel: 4,
    defaultPane: "network",
    destinations: [
      { pane: "network", label: "Network", keywords: ["map", "pylon", "globe", "home"] },
      { pane: "training", label: "Training", keywords: ["runs", "gradient", "evidence"] },
      { pane: "training-fullscreen", label: "Training Live", keywords: ["scene", "immersive", "fullscreen"] },
      { pane: "builtin-agent", label: "Agent", keywords: ["builtin", "apple fm", "hosted", "compute"] },
    ],
  },
  {
    id: "settings",
    label: "Settings",
    accel: 5,
    defaultPane: "settings",
    destinations: [
      { pane: "settings", label: "Settings", keywords: ["preferences", "keybindings", "shortcuts", "health"] },
      { pane: "onboarding", label: "Get started", keywords: ["onboarding", "wizard", "first run", "setup"] },
    ],
  },
]

// Panes that are reachable but never sit in the primary nav (intentional leaves
// per the audit). `session-detail` is opened by selecting a session row.
// ZERO-BASE SHELL (owner directive, 2026-06-19): `shell` is the dead-simple
// DEFAULT surface (black + bottom text bar). It is reached by launch, by the
// "open panes ← back" path (ClosedPanes), and by Esc — never by a nav button —
// so it is an intentional leaf, not a nav destination, and it is deliberately
// kept OUT of the palette/sidebar (the shell hides the nav, it is not part of it).
export const NAV_LEAF_PANES: ReadonlyArray<PaneId> = ["shell", "session-detail"]

// Flattened destination list (used by the palette + lookups).
export const NAV_DESTINATIONS: ReadonlyArray<NavDestination & { groupId: string }> =
  NAV_GROUPS.flatMap((group) =>
    group.destinations.map((dest) => ({ ...dest, groupId: group.id })),
  )

// Which group "owns" a pane, so the grouped sidebar highlights the active group
// and renders the right secondary strip for the current pane. Leaves fall back
// to their nearest owning group (session-detail → Code).
const PANE_TO_GROUP: ReadonlyMap<PaneId, NavGroup> = (() => {
  const map = new Map<PaneId, NavGroup>()
  for (const group of NAV_GROUPS) {
    for (const dest of group.destinations) map.set(dest.pane, group)
  }
  // session-detail is a Code leaf (opened from Sessions/Swarm/Composer).
  const code = NAV_GROUPS.find((g) => g.id === "code")
  if (code && !map.has("session-detail")) map.set("session-detail", code)
  return map
})()

export const groupForPane = (pane: PaneId): NavGroup | null =>
  PANE_TO_GROUP.get(pane) ?? null

export const groupById = (id: string): NavGroup | null =>
  NAV_GROUPS.find((group) => group.id === id) ?? null

export const groupByAccel = (accel: number): NavGroup | null =>
  NAV_GROUPS.find((group) => group.accel === accel) ?? null

// ── Command registry (#5464) ────────────────────────────────────────────────
// A "command" is one searchable Cmd-K row. Two kinds:
//   - `navigate`: jump to a pane (built from NAV_DESTINATIONS above).
//   - `action`:   dispatch an existing message tag (mapped in view.ts to the
//                 real Message constructor — no new RPC verb, audit §5.2).
// `messageTag` is the Message `_tag` the palette dispatches; `args` carries the
// (already-typed) literal payload for that tag. The palette layer in view.ts
// maps `(messageTag,args) → Message` so this registry stays free of the Message
// union import (keeps the seam declarative + cheap to extend).
export type PaletteCommand = Readonly<{
  // Stable id, also a fuzzy-match anchor.
  id: string
  label: string
  group: string
  keywords: ReadonlyArray<string>
}> &
  (
    | Readonly<{ kind: "navigate"; pane: PaneId }>
    | Readonly<{ kind: "action"; messageTag: string; args?: Record<string, unknown> }>
  )

// Navigate-to commands are derived from the registry so a new destination shows
// up in the palette automatically (one registry entry → one palette row).
const navigateCommands: ReadonlyArray<PaletteCommand> = NAV_DESTINATIONS.map((dest) => ({
  kind: "navigate" as const,
  id: `go.${dest.pane}`,
  label: `Go to ${dest.label}`,
  group: dest.groupId,
  pane: dest.pane,
  keywords: ["go", "open", "navigate", dest.label.toLowerCase(), ...(dest.keywords ?? [])],
}))

// Action commands map to EXISTING messages (audit §5.3): no new control verbs.
// Phase-2 agents add their key action here (one entry) rather than a button.
const actionCommands: ReadonlyArray<PaletteCommand> = [
  {
    kind: "action",
    id: "action.spawn",
    label: "Spawn a session",
    group: "code",
    messageTag: "NavigatedTo",
    args: { pane: "spawn" },
    keywords: ["spawn", "new", "session", "launch", "start", "objective"],
  },
  {
    kind: "action",
    id: "action.submit-intent",
    label: "Submit an intent (Ask Autopilot)",
    group: "supervise",
    messageTag: "ClickedSubmitIntent",
    keywords: ["intent", "ask", "objective", "autonomous", "request"],
  },
  {
    kind: "action",
    id: "action.resolve-next-approval",
    label: "Resolve the next pending approval",
    group: "supervise",
    // Navigates to the approvals roll-up; j/k + Enter (or the buttons) act on
    // the next pending approval there. Mapped to NavigatedTo to avoid inventing
    // a "resolve-next" verb the runtime does not yet expose for MVP.
    messageTag: "NavigatedTo",
    args: { pane: "decisions" },
    keywords: ["approve", "deny", "approval", "decision", "resolve", "next"],
  },
  {
    kind: "action",
    id: "action.coordinator-pause",
    label: "Pause the autonomous coordinator",
    group: "supervise",
    messageTag: "ClickedCoordinatorToggle",
    args: { paused: true },
    keywords: ["coordinator", "pause", "stop", "loop", "autonomous", "afk"],
  },
  {
    kind: "action",
    id: "action.coordinator-resume",
    label: "Resume the autonomous coordinator",
    group: "supervise",
    messageTag: "ClickedCoordinatorToggle",
    args: { paused: false },
    keywords: ["coordinator", "resume", "start", "loop", "autonomous", "afk"],
  },
  {
    kind: "action",
    id: "action.open-replay",
    label: "Open a proof replay",
    group: "explore",
    messageTag: "NavigatedTo",
    args: { pane: "training" },
    keywords: ["replay", "proof", "receipt", "bundle", "verify", "evidence"],
  },
]

export const paletteCommands: ReadonlyArray<PaletteCommand> = [
  ...navigateCommands,
  ...actionCommands,
]

// ── Fuzzy matcher for the palette (#5464) ────────────────────────────────────
// A small, dependency-free subsequence matcher over label + keywords. Pure so
// the view stays a function of (model.commandPaletteQuery). Lower score = better.
export type PaletteMatch = Readonly<{ command: PaletteCommand; score: number }>

const subsequenceScore = (needle: string, haystack: string): number | null => {
  if (needle.length === 0) return 0
  let hi = 0
  let score = 0
  let lastMatch = -1
  for (let ni = 0; ni < needle.length; ni++) {
    const ch = needle[ni]
    let found = -1
    for (; hi < haystack.length; hi++) {
      if (haystack[hi] === ch) {
        found = hi
        break
      }
    }
    if (found === -1) return null
    // Penalise gaps between matched chars so contiguous matches rank first.
    score += lastMatch === -1 ? found : found - lastMatch - 1
    lastMatch = found
    hi = found + 1
  }
  return score
}

export const filterPaletteCommands = (
  query: string,
  commands: ReadonlyArray<PaletteCommand> = paletteCommands,
): ReadonlyArray<PaletteMatch> => {
  const q = query.trim().toLowerCase()
  if (q === "") return commands.map((command) => ({ command, score: 0 }))
  const matches: PaletteMatch[] = []
  for (const command of commands) {
    const haystacks = [command.label.toLowerCase(), command.id.toLowerCase(), ...command.keywords]
    let best: number | null = null
    for (const haystack of haystacks) {
      const s = subsequenceScore(q, haystack)
      if (s !== null && (best === null || s < best)) best = s
    }
    if (best !== null) matches.push({ command, score: best })
  }
  matches.sort((a, b) => a.score - b.score)
  return matches
}

// ── Keyboard shortcut catalogue (#5465) ──────────────────────────────────────
// Single source of truth for the shortcut layer AND the Settings listing, so
// the displayed list can never drift from what the keyboard subscription
// actually handles. `chord` is display copy; `when` documents the scope.
export type ShortcutSpec = Readonly<{
  chord: string
  description: string
  when: string
}>

export const SHORTCUTS: ReadonlyArray<ShortcutSpec> = [
  { chord: "⌘K / Ctrl-K", description: "Open the command palette", when: "anywhere" },
  { chord: "Esc", description: "Close the command palette", when: "palette open" },
  { chord: "↑ / ↓", description: "Move the palette selection", when: "palette open" },
  { chord: "Enter", description: "Run the selected command", when: "palette open" },
  ...NAV_GROUPS.map((group) => ({
    chord: `⌘${group.accel} / Ctrl-${group.accel}`,
    description: `Jump to ${group.label}`,
    when: "anywhere (not while typing)",
  })),
  { chord: "⌘↵ / Ctrl-↵", description: "Submit the chat / composer turn", when: "Chat or Composer" },
  { chord: "j / k", description: "Move to the next / previous sub-pane in the group", when: "anywhere (not while typing)" },
]
