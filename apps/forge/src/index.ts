import { Effect, Schema } from "effect"

import {
  autopilotCoreDarkCssVars,
  autopilotCoreDarkTokens,
  oaTokens,
} from "@openagentsinc/ui/tokens"

export const FORGE_UI_WORKER_VERSION = "forge-ui.2026-06-28.6769"
export const OPENAGENTS_FORGE_TENANT_REF = "tenant.openagents"
export const OPENAGENTS_FORGE_REPOSITORY_REF = "repo.openagents.openagents"
export const OPENAGENTS_FORGE_DEFAULT_BRANCH_REF = "refs/heads/main"

export const ForgeMount = Schema.Struct({
  product: Schema.Literal("forge"),
  host: Schema.Literal("forge.openagents.com"),
  basePath: Schema.Literal("/"),
  runtime: Schema.Literal("cloudflare-worker"),
  uiPackage: Schema.Literal("@openagentsinc/ui"),
})

export type ForgeMount = typeof ForgeMount.Type

export const defaultForgeMount: ForgeMount = {
  product: "forge",
  host: "forge.openagents.com",
  basePath: "/",
  runtime: "cloudflare-worker",
  uiPackage: "@openagentsinc/ui",
}

export const forgeLandingCopy = {
  title: "THE FORGE",
  tagline: "where agents git it on",
} as const

export const ForgeShellRouteId = Schema.Literals([
  "overview",
  "dogfood",
  "work",
  "changes",
  "verification",
  "queue",
  "refs",
])
export type ForgeShellRouteId = typeof ForgeShellRouteId.Type

export const ForgeShellRoute = Schema.Struct({
  id: ForgeShellRouteId,
  path: Schema.String,
  label: Schema.String,
  summary: Schema.String,
  apiPath: Schema.String,
})
export type ForgeShellRoute = typeof ForgeShellRoute.Type

export const forgeShellRoutes: ReadonlyArray<ForgeShellRoute> = [
  {
    id: "overview",
    path: "/",
    label: "Overview",
    summary: "system state, boundaries, and active slices",
    apiPath: "/api/forge/overview",
  },
  {
    id: "dogfood",
    path: "/dogfood",
    label: "Dogfood Lane",
    summary: "SU-7 fleet lane, fallback path, and first live lessons",
    apiPath: "/api/forge/dogfood-lanes",
  },
  {
    id: "work",
    path: "/work",
    label: "Work Queue",
    summary: "issue-backed work records and leases",
    apiPath: "/api/forge/work-records",
  },
  {
    id: "changes",
    path: "/changes",
    label: "Changes",
    summary: "change records, patch heads, and blockers",
    apiPath: "/api/forge/changes",
  },
  {
    id: "verification",
    path: "/verification",
    label: "Verification",
    summary: "runner receipts and promotion evidence",
    apiPath: "/api/forge/verification-receipts",
  },
  {
    id: "queue",
    path: "/queue",
    label: "Merge Queue",
    summary: "virtual heads, gate state, and next promotion",
    apiPath: "/api/forge/queue",
  },
  {
    id: "refs",
    path: "/refs",
    label: "Git Refs",
    summary: "canonical ref namespaces and mirror state",
    apiPath: "/api/forge/refs",
  },
]

export const ForgeShellWorkItem = Schema.Struct({
  workRef: Schema.String,
  issueRef: Schema.String,
  title: Schema.String,
  priority: Schema.String,
  owner: Schema.String,
  state: Schema.String,
  lease: Schema.String,
})
export type ForgeShellWorkItem = typeof ForgeShellWorkItem.Type

export const ForgeShellChangeItem = Schema.Struct({
  changeRef: Schema.String,
  workRef: Schema.String,
  baseHead: Schema.String,
  patchHead: Schema.String,
  verificationRef: Schema.String,
  verificationState: Schema.String,
  state: Schema.String,
  blockers: Schema.Array(Schema.String),
})
export type ForgeShellChangeItem = typeof ForgeShellChangeItem.Type

export const ForgeShellVerificationItem = Schema.Struct({
  receiptRef: Schema.String,
  changeRef: Schema.String,
  verdict: Schema.String,
  command: Schema.String,
  baseHead: Schema.String,
  headHead: Schema.String,
  executor: Schema.String,
  logDigest: Schema.String,
})
export type ForgeShellVerificationItem = typeof ForgeShellVerificationItem.Type

export const ForgeShellQueueItem = Schema.Struct({
  position: Schema.String,
  changeRef: Schema.String,
  virtualHead: Schema.String,
  actualHead: Schema.String,
  gate: Schema.String,
  state: Schema.String,
})
export type ForgeShellQueueItem = typeof ForgeShellQueueItem.Type

export const ForgeShellRefItem = Schema.Struct({
  tenantRef: Schema.String,
  repositoryRef: Schema.String,
  ref: Schema.String,
  target: Schema.String,
  authority: Schema.String,
  state: Schema.String,
})
export type ForgeShellRefItem = typeof ForgeShellRefItem.Type

export const ForgeShellDogfoodLane = Schema.Struct({
  laneRef: Schema.String,
  issueRef: Schema.String,
  repository: Schema.String,
  status: Schema.String,
  intakeRef: Schema.String,
  verificationRef: Schema.String,
  queueRef: Schema.String,
  promotionRef: Schema.String,
  mirrorRef: Schema.String,
  verificationCommand: Schema.String,
  fallbackPath: Schema.String,
  lessons: Schema.Array(Schema.String),
  metrics: Schema.Struct({
    triage: Schema.String,
    changeInspector: Schema.String,
    attentionQueue: Schema.String,
    cycleVelocity: Schema.String,
  }),
})
export type ForgeShellDogfoodLane = typeof ForgeShellDogfoodLane.Type

export const ForgeShellSnapshot = Schema.Struct({
  dataMode: Schema.Literal("live-api-contract"),
  apiBasePath: Schema.Literal("/api/forge"),
  generatedAt: Schema.String,
  dogfoodLanes: Schema.Array(ForgeShellDogfoodLane),
  workQueue: Schema.Array(ForgeShellWorkItem),
  changes: Schema.Array(ForgeShellChangeItem),
  verification: Schema.Array(ForgeShellVerificationItem),
  mergeQueue: Schema.Array(ForgeShellQueueItem),
  refs: Schema.Array(ForgeShellRefItem),
})
export type ForgeShellSnapshot = typeof ForgeShellSnapshot.Type

export const forgeShellPreviewState: ForgeShellSnapshot = {
  dataMode: "live-api-contract",
  apiBasePath: "/api/forge",
  generatedAt: "2026-06-28T00:00:00.000Z",
  dogfoodLanes: [
    {
      laneRef: "lane.forge.su7.openagents-codex-low-risk",
      issueRef: "#6797",
      repository: "OpenAgentsInc/openagents",
      status: "operator-ready",
      intakeRef: "refs/forge/intake/openagents/codex-low-risk",
      verificationRef: "receipt.forge.su7.su5-check-deploy",
      queueRef: "queue.forge.su7.nextActualPromotion",
      promotionRef: "promotion.forge.su7.su4-blueprint-gated",
      mirrorRef: "mirror.github.openagents.main.su7",
      verificationCommand: "bun run --cwd apps/openagents.com check:deploy",
      fallbackPath: "pause Forge lane, reopen GitHub PR path, keep Forge rows as audit evidence",
      lessons: [
        "Triage needs one lane owner, one issue ref, and one visible blocked reason.",
        "Change inspection must keep base head, patch head, verification receipt, and promotion receipt in the same row.",
        "Attention queue should sort by operator action: needs verification, needs gate, needs mirror, or escaped.",
        "Cycle metrics must count Forge intake-to-mirror time, not GitHub PR time.",
      ],
      metrics: {
        triage: "one low-risk Codex/Pylon lane selected for OpenAgents repo dogfood",
        changeInspector: "base, patch, verification, promotion, and mirror refs rendered together",
        attentionQueue: "queue state names the next operator action and rollback path",
        cycleVelocity: "first measurement starts at Forge intake and ends at SU-6 mirror",
      },
    },
  ],
  workQueue: [
    {
      workRef: "work.forge.6797",
      issueRef: "#6797",
      title: "Dogfood one OpenAgents fleet lane through Forge",
      priority: "P0",
      owner: "codex-pylon-lane",
      state: "operator-ready",
      lease: "Forge lane only; GitHub is mirror after promotion",
    },
    {
      workRef: "work.forge.6769",
      issueRef: "#6769",
      title: "Separate Forge UI shell",
      priority: "P0",
      owner: "forge-ui",
      state: "shell-ready",
      lease: "held by apps/forge",
    },
    {
      workRef: "work.forge.6770",
      issueRef: "#6770",
      title: "Control-plane routes",
      priority: "P0",
      owner: "forge-api",
      state: "next",
      lease: "open",
    },
    {
      workRef: "work.forge.6771",
      issueRef: "#6771",
      title: "Smart Git intake",
      priority: "P0",
      owner: "forge-intake",
      state: "queued",
      lease: "open",
    },
  ],
  changes: [
    {
      changeRef: "change.forge.su7.openagents-codex-low-risk",
      workRef: "work.forge.6797",
      baseHead: "refs/heads/main",
      patchHead: "refs/forge/changes/openagents/codex-low-risk",
      verificationRef: "pending",
      verificationState: "receipt-required",
      state: "awaiting-smart-git-intake",
      blockers: ["blocker.forge.verification.missing_receipt_ref"],
    },
    {
      changeRef: "change.forge.shell",
      workRef: "work.forge.6769",
      baseHead: "refs/heads/main@81182403c3",
      patchHead: "refs/forge/changes/shell-preview",
      verificationRef: "pending",
      verificationState: "pending-live-run",
      state: "public-safe-preview",
      blockers: ["blocker.forge.verification.missing_receipt_ref"],
    },
    {
      changeRef: "change.forge.control-plane",
      workRef: "work.forge.6770",
      baseHead: "refs/heads/main",
      patchHead: "pending-/api/forge",
      verificationRef: "pending",
      verificationState: "blocked",
      state: "waiting-for-route-registry",
      blockers: ["control-plane-auth", "d1-writes", "blocker.forge.verification.missing_receipt_ref"],
    },
  ],
  verification: [
    {
      receiptRef: "receipt.forge.su7.su5-check-deploy",
      changeRef: "change.forge.su7.openagents-codex-low-risk",
      verdict: "required-before-promotion",
      command: "bun run --cwd apps/openagents.com check:deploy",
      baseHead: "refs/heads/main",
      headHead: "refs/forge/changes/openagents/codex-low-risk",
      executor: "owned Pylon forge-verification-runner",
      logDigest: "pending-first-live-receipt",
    },
    {
      receiptRef: "receipt.forge.shell.local",
      changeRef: "change.forge.shell",
      verdict: "pending-live-run",
      command: "bun run --cwd apps/forge test",
      baseHead: "refs/heads/main@81182403c3",
      headHead: "refs/forge/changes/shell-preview",
      executor: "apps/forge Worker test harness",
      logDigest: "local-after-implementation",
    },
    {
      receiptRef: "receipt.contract.su0",
      changeRef: "change.forge.boundary",
      verdict: "passed",
      command: "bun run --cwd packages/forge-protocol test",
      baseHead: "schema.fixture.base",
      headHead: "schema.fixture.head",
      executor: "forge-protocol schema tests",
      logDigest: "sha256-public-summary",
    },
  ],
  mergeQueue: [
    {
      position: "dogfood-lane",
      changeRef: "change.forge.su7.openagents-codex-low-risk",
      virtualHead: "refs/forge/virtual/openagents/main+codex-low-risk",
      actualHead: "refs/heads/main",
      gate: "SU-4 promotion waits for SU-5 verification receipt",
      state: "queued-after-intake",
    },
    {
      position: "nextActualPromotion",
      changeRef: "change.forge.shell",
      virtualHead: "refs/forge/virtual/main+shell",
      actualHead: "refs/heads/main",
      gate: "requires passing ForgeVerificationReceipt for current base/head",
      state: "blocked-verification-required",
    },
    {
      position: "blocked",
      changeRef: "change.forge.control-plane",
      virtualHead: "refs/forge/virtual/main+api",
      actualHead: "refs/heads/main",
      gate: "awaiting SU-2 implementation",
      state: "not-promotable",
    },
  ],
  refs: [
    {
      tenantRef: OPENAGENTS_FORGE_TENANT_REF,
      repositoryRef: OPENAGENTS_FORGE_REPOSITORY_REF,
      ref: "refs/forge/intake/openagents/codex-low-risk",
      target: "selected low-risk OpenAgents Codex/Pylon lane",
      authority: "Forge smart-Git intake",
      state: "dogfood-lane",
    },
    {
      tenantRef: OPENAGENTS_FORGE_TENANT_REF,
      repositoryRef: OPENAGENTS_FORGE_REPOSITORY_REF,
      ref: "refs/forge/mirror/github/openagents/main",
      target: "GitHub downstream visibility after SU-6",
      authority: "Forge mirror worker",
      state: "mirror-only",
    },
    {
      tenantRef: OPENAGENTS_FORGE_TENANT_REF,
      repositoryRef: OPENAGENTS_FORGE_REPOSITORY_REF,
      ref: "refs/heads/main",
      target: "GitHub mirror until SU-3",
      authority: "mirror projection",
      state: "readable",
    },
    {
      tenantRef: OPENAGENTS_FORGE_TENANT_REF,
      repositoryRef: OPENAGENTS_FORGE_REPOSITORY_REF,
      ref: OPENAGENTS_FORGE_DEFAULT_BRANCH_REF,
      target: "OpenAgentsInc/openagents default branch",
      authority: "/api/forge/refs live canonical store",
      state: "import-ready",
    },
    {
      tenantRef: OPENAGENTS_FORGE_TENANT_REF,
      repositoryRef: OPENAGENTS_FORGE_REPOSITORY_REF,
      ref: "refs/forge/changes/*",
      target: "canonical change heads",
      authority: "Forge control plane",
      state: "contracted",
    },
    {
      tenantRef: OPENAGENTS_FORGE_TENANT_REF,
      repositoryRef: OPENAGENTS_FORGE_REPOSITORY_REF,
      ref: "refs/forge/virtual/*",
      target: "virtual merge queue heads",
      authority: "Blueprint gates",
      state: "contracted",
    },
  ],
}

const escapeHtml = (value: string): string =>
  value.replace(/[&<>"']/g, character => {
    switch (character) {
      case "&":
        return "&amp;"
      case "<":
        return "&lt;"
      case ">":
        return "&gt;"
      case '"':
        return "&quot;"
      case "'":
        return "&#39;"
      default:
        return character
    }
  })

const cssDeclarations = (input: Record<string, string>): string =>
  Object.entries(input)
    .map(([name, value]) => `  ${name}: ${value};`)
    .join("\n")

const sharedTokenCss = cssDeclarations({
  ...autopilotCoreDarkCssVars(autopilotCoreDarkTokens),
  "--forge-accent": oaTokens.color.accent,
  "--forge-accent-soft": oaTokens.color.accentSoft,
  "--forge-surface": oaTokens.color.componentSurface,
  "--forge-border": oaTokens.color.componentBorderStrong,
  "--forge-text-bright": oaTokens.color.textBright,
  "--forge-text-muted": oaTokens.color.textMuted,
  "--forge-radius": "8px",
  "--forge-font-mono": oaTokens.font.mono,
  "--forge-energy": "#5ea1ff",
  "--forge-cyan": "#63e6ff",
  "--forge-green": "#49f28d",
  "--forge-warn": "#f5b73a",
  "--forge-danger": "#ff6b6b",
  "--forge-black": "#040507",
  "--forge-panel": "#080d13",
  "--forge-panel-strong": "#0d141d",
  "--forge-line": "rgba(142, 171, 230, 0.22)",
  "--forge-line-strong": "rgba(142, 171, 230, 0.38)",
})

export const forgeShellStyles = `:root {
${sharedTokenCss}
  color-scheme: dark;
}

* {
  box-sizing: border-box;
}

html,
body {
  min-height: 100%;
  margin: 0;
}

body {
  min-height: 100dvh;
  background: var(--forge-black);
  color: var(--text);
  font-family: var(--forge-font-mono), ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", monospace;
  -webkit-font-smoothing: antialiased;
  text-rendering: geometricPrecision;
}

a {
  color: inherit;
  text-decoration: none;
}

.forge-shell {
  display: grid;
  grid-template-columns: minmax(13rem, 16rem) minmax(0, 1fr);
  min-height: 100dvh;
  background: var(--forge-black);
}

.forge-rail {
  display: grid;
  align-content: start;
  gap: 1.25rem;
  min-height: 100dvh;
  padding: 1rem;
  border-right: 1px solid var(--forge-line);
  background: #05080c;
}

.forge-brand {
  display: grid;
  gap: 0.35rem;
  padding: 0.75rem 0.25rem 1rem;
  border-bottom: 1px solid var(--forge-line);
}

.forge-host,
.forge-mode,
.forge-route-api,
.forge-table-caption {
  color: var(--forge-text-muted);
  font-size: 0.72rem;
  line-height: 1.35;
}

.forge-brand-title {
  margin: 0;
  color: var(--forge-text-bright);
  font-size: clamp(2rem, 6vw, 3.8rem);
  font-weight: 650;
  letter-spacing: 0;
  line-height: 0.96;
  text-transform: uppercase;
}

.forge-tagline {
  margin: 0;
  color: var(--forge-cyan);
  font-size: 0.86rem;
  line-height: 1.45;
}

.forge-nav {
  display: grid;
  gap: 0.35rem;
}

.forge-nav-link {
  display: grid;
  gap: 0.24rem;
  min-height: 3.4rem;
  padding: 0.7rem 0.75rem;
  border: 1px solid transparent;
  border-radius: var(--forge-radius);
  color: var(--forge-text-muted);
}

.forge-nav-link:hover,
.forge-nav-link:focus-visible {
  border-color: var(--forge-line-strong);
  color: var(--forge-text-bright);
  outline: 0;
}

.forge-nav-link[aria-current="page"] {
  border-color: rgba(99, 230, 255, 0.56);
  background: var(--forge-panel-strong);
  color: var(--forge-text-bright);
}

.forge-nav-label {
  font-size: 0.86rem;
}

.forge-nav-summary {
  color: var(--forge-text-muted);
  font-size: 0.68rem;
  line-height: 1.35;
}

.forge-main {
  display: grid;
  grid-template-rows: auto 1fr;
  min-width: 0;
}

.forge-topbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
  min-height: 3.75rem;
  padding: 0.8rem 1.25rem;
  border-bottom: 1px solid var(--forge-line);
  background: #05080c;
}

.forge-topbar-left,
.forge-topbar-right {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 0.55rem;
  min-width: 0;
}

.forge-pill {
  display: inline-flex;
  align-items: center;
  min-height: 1.65rem;
  max-width: 100%;
  padding: 0.3rem 0.5rem;
  border: 1px solid var(--forge-line);
  border-radius: var(--forge-radius);
  color: var(--forge-text-muted);
  font-size: 0.72rem;
  line-height: 1.2;
  overflow-wrap: anywhere;
}

.forge-pill[data-tone="cyan"] {
  border-color: rgba(99, 230, 255, 0.42);
  color: var(--forge-cyan);
}

.forge-pill[data-tone="gold"] {
  border-color: rgba(245, 183, 58, 0.42);
  color: var(--forge-warn);
}

.forge-pill[data-tone="green"] {
  border-color: rgba(73, 242, 141, 0.42);
  color: var(--forge-green);
}

.forge-stage {
  display: grid;
  align-content: start;
  gap: 1.25rem;
  padding: 1.25rem;
}

.forge-route-head {
  display: grid;
  gap: 0.55rem;
  padding-bottom: 1rem;
  border-bottom: 1px solid var(--forge-line);
}

.forge-route-title {
  margin: 0;
  color: var(--forge-text-bright);
  font-size: clamp(1.5rem, 4vw, 2.65rem);
  font-weight: 620;
  letter-spacing: 0;
  line-height: 1.05;
}

.forge-route-copy {
  max-width: 68rem;
  margin: 0;
  color: var(--forge-text-muted);
  font-size: 0.88rem;
  line-height: 1.6;
}

.forge-grid {
  display: grid;
  gap: 1rem;
  grid-template-columns: repeat(auto-fit, minmax(min(100%, 18rem), 1fr));
}

.forge-panel {
  display: grid;
  gap: 0.85rem;
  min-width: 0;
  padding: 1rem;
  border: 1px solid var(--forge-line);
  border-radius: var(--forge-radius);
  background: var(--forge-panel);
}

.forge-panel[data-span="wide"] {
  grid-column: 1 / -1;
}

.forge-panel-head {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 0.75rem;
  min-width: 0;
}

.forge-panel-title {
  margin: 0;
  color: var(--forge-text-bright);
  font-size: 0.98rem;
  font-weight: 640;
  letter-spacing: 0;
}

.forge-metric {
  display: grid;
  gap: 0.4rem;
  min-height: 7.25rem;
  padding: 1rem;
  border: 1px solid var(--forge-line);
  border-radius: var(--forge-radius);
  background: var(--forge-panel-strong);
}

.forge-metric-value {
  color: var(--forge-text-bright);
  font-size: 1.9rem;
  font-weight: 650;
  line-height: 1;
}

.forge-metric-label {
  color: var(--forge-text-muted);
  font-size: 0.76rem;
  line-height: 1.4;
}

.forge-table-wrap {
  overflow-x: auto;
}

.forge-table {
  width: 100%;
  min-width: 48rem;
  border-collapse: collapse;
  table-layout: fixed;
}

.forge-table th,
.forge-table td {
  padding: 0.7rem 0.65rem;
  border-bottom: 1px solid var(--forge-line);
  text-align: left;
  vertical-align: top;
  overflow-wrap: anywhere;
}

.forge-table th {
  color: var(--forge-text-muted);
  font-size: 0.7rem;
  font-weight: 620;
  text-transform: uppercase;
}

.forge-table td {
  color: var(--forge-text-bright);
  font-size: 0.8rem;
  line-height: 1.45;
}

.forge-muted {
  color: var(--forge-text-muted);
}

.forge-code {
  color: var(--forge-cyan);
}

.forge-state {
  display: inline-flex;
  align-items: center;
  min-height: 1.5rem;
  padding: 0.22rem 0.42rem;
  border: 1px solid var(--forge-line);
  border-radius: var(--forge-radius);
  color: var(--forge-warn);
  font-size: 0.7rem;
  line-height: 1.15;
}

.forge-list {
  display: grid;
  gap: 0.55rem;
  margin: 0;
  padding: 0;
  list-style: none;
}

.forge-list-item {
  display: grid;
  gap: 0.35rem;
  padding: 0.75rem;
  border: 1px solid var(--forge-line);
  border-radius: var(--forge-radius);
  background: #060a0f;
}

.forge-list-kicker {
  color: var(--forge-cyan);
  font-size: 0.72rem;
}

.forge-list-title {
  color: var(--forge-text-bright);
  font-size: 0.84rem;
  line-height: 1.45;
}

.forge-lane {
  display: grid;
  gap: 1rem;
}

.forge-lane-grid {
  display: grid;
  gap: 0.75rem;
  grid-template-columns: repeat(auto-fit, minmax(min(100%, 16rem), 1fr));
}

.forge-lane-section {
  display: grid;
  gap: 0.7rem;
  min-width: 0;
  padding-top: 1rem;
  border-top: 1px solid var(--forge-line);
}

.forge-ref-card {
  display: grid;
  gap: 0.32rem;
  min-height: 6.25rem;
  padding: 0.78rem;
  border: 1px solid var(--forge-line);
  border-radius: var(--forge-radius);
  background: #060a0f;
}

.forge-ref-label {
  color: var(--forge-text-muted);
  font-size: 0.68rem;
  line-height: 1.3;
  text-transform: uppercase;
}

.forge-ref-value {
  color: var(--forge-cyan);
  font-size: 0.78rem;
  line-height: 1.45;
  overflow-wrap: anywhere;
}

.forge-runbook {
  display: grid;
  gap: 0.7rem;
  margin: 0;
  padding: 0;
  list-style: none;
}

.forge-runbook li {
  display: grid;
  gap: 0.28rem;
  padding: 0.72rem;
  border: 1px solid var(--forge-line);
  border-radius: var(--forge-radius);
  background: var(--forge-panel-strong);
}

@media (max-width: 52rem) {
  .forge-shell {
    grid-template-columns: 1fr;
  }

  .forge-rail {
    min-height: auto;
    border-right: 0;
    border-bottom: 1px solid var(--forge-line);
  }

  .forge-nav {
    grid-auto-flow: column;
    grid-auto-columns: minmax(9.75rem, 1fr);
    overflow-x: auto;
    padding-bottom: 0.2rem;
  }

  .forge-topbar {
    align-items: flex-start;
    flex-direction: column;
  }

  .forge-stage {
    padding: 1rem;
  }
}`

export const forgeLandingStyles = forgeShellStyles

const routeById = (routeId: ForgeShellRouteId): ForgeShellRoute =>
  forgeShellRoutes.find(route => route.id === routeId) ?? forgeShellRoutes[0]!

export const resolveForgeShellRoute = (pathname: string): ForgeShellRoute | null => {
  const normalized =
    pathname.length > 1 && pathname.endsWith("/")
      ? pathname.slice(0, -1)
      : pathname

  return forgeShellRoutes.find(route => route.path === normalized) ?? null
}

const renderStatus = (value: string): string =>
  `<span class="forge-state">${escapeHtml(value)}</span>`

const renderNav = (activeRoute: ForgeShellRoute): string =>
  forgeShellRoutes
    .map(route => {
      const current = route.id === activeRoute.id ? ' aria-current="page"' : ""

      return `<a class="forge-nav-link" href="${escapeHtml(route.path)}"${current}>
        <span class="forge-nav-label">${escapeHtml(route.label)}</span>
        <span class="forge-nav-summary">${escapeHtml(route.summary)}</span>
      </a>`
    })
    .join("\n")

const renderRouteHeader = (route: ForgeShellRoute): string => `<header class="forge-route-head">
  <p class="forge-route-api">${escapeHtml(route.apiPath)}</p>
  <h2 class="forge-route-title">${escapeHtml(route.label)}</h2>
  <p class="forge-route-copy">${escapeHtml(route.summary)}</p>
</header>`

const renderOverview = (): string => `<section class="forge-grid">
  <article class="forge-metric">
    <span class="forge-metric-value">${forgeShellPreviewState.workQueue.length}</span>
    <span class="forge-metric-label">issue-backed work records in the public-safe shell contract</span>
  </article>
  <article class="forge-metric">
    <span class="forge-metric-value">${forgeShellPreviewState.changes.length}</span>
    <span class="forge-metric-label">change records shaped for base/head and blocker inspection</span>
  </article>
  <article class="forge-metric">
    <span class="forge-metric-value">${forgeShellPreviewState.mergeQueue.length}</span>
    <span class="forge-metric-label">virtual merge queue lanes mapped to promotion gates</span>
  </article>
  <article class="forge-metric">
    <span class="forge-metric-value">${forgeShellPreviewState.refs.length}</span>
    <span class="forge-metric-label">ref namespaces split between Forge authority and GitHub mirror projection</span>
  </article>
  <article class="forge-metric">
    <span class="forge-metric-value">${forgeShellPreviewState.dogfoodLanes.length}</span>
    <span class="forge-metric-label">SU-7 Codex/Pylon dogfood lane prepared for Forge-only coordination</span>
  </article>
  <section class="forge-panel" data-span="wide">
    <div class="forge-panel-head">
      <h3 class="forge-panel-title">Contract Routes</h3>
      <span class="forge-table-caption">${escapeHtml(forgeShellPreviewState.apiBasePath)}</span>
    </div>
    <div class="forge-table-wrap">
      <table class="forge-table">
        <thead>
          <tr>
            <th>Surface</th>
            <th>UI Route</th>
            <th>API Route</th>
            <th>Shape</th>
          </tr>
        </thead>
        <tbody>
          ${forgeShellRoutes
            .map(
              route => `<tr>
                <td>${escapeHtml(route.label)}</td>
                <td class="forge-code">${escapeHtml(route.path)}</td>
                <td class="forge-code">${escapeHtml(route.apiPath)}</td>
                <td class="forge-muted">${escapeHtml(route.summary)}</td>
              </tr>`,
            )
            .join("\n")}
        </tbody>
      </table>
    </div>
  </section>
</section>`

const dogfoodRefCards = (
  lane: ForgeShellDogfoodLane,
): ReadonlyArray<readonly [string, string]> => [
  ["Intake", lane.intakeRef],
  ["Verification", lane.verificationRef],
  ["Queue", lane.queueRef],
  ["Promotion", lane.promotionRef],
  ["Mirror", lane.mirrorRef],
]

const renderDogfood = (): string => `<section class="forge-grid">
  ${forgeShellPreviewState.dogfoodLanes
    .map(
      lane => `<article class="forge-panel forge-lane" data-span="wide" data-forge-dogfood-lane="${escapeHtml(lane.laneRef)}">
        <div class="forge-panel-head">
          <h3 class="forge-panel-title">${escapeHtml(lane.laneRef)}</h3>
          ${renderStatus(lane.status)}
        </div>
        <p class="forge-route-copy">${escapeHtml(lane.repository)} ${escapeHtml(lane.issueRef)} is the first bounded OpenAgents Codex/Pylon lane for Forge dogfood. The UI keeps intake, verification, queue, promotion, and GitHub mirror refs visible together so GitHub stays downstream visibility only.</p>
        <div class="forge-lane-grid">
          ${dogfoodRefCards(lane)
            .map(
              ([label, value]) => `<div class="forge-ref-card">
                <span class="forge-ref-label">${escapeHtml(label)}</span>
                <span class="forge-ref-value">${escapeHtml(value)}</span>
              </div>`,
            )
            .join("\n")}
        </div>
        <section class="forge-lane-section">
          <div class="forge-panel-head">
            <h4 class="forge-panel-title">Operator Runbook</h4>
            <span class="forge-table-caption">public-safe command sequence</span>
          </div>
          <ol class="forge-runbook">
            <li>
              <span class="forge-list-kicker">1. Intake</span>
              <span class="forge-list-title">Push the selected low-risk lane through Forge smart-Git into ${escapeHtml(lane.intakeRef)}.</span>
            </li>
            <li>
              <span class="forge-list-kicker">2. Verify</span>
              <span class="forge-list-title">Require SU-5 receipt ${escapeHtml(lane.verificationRef)} from <span class="forge-code">${escapeHtml(lane.verificationCommand)}</span>.</span>
            </li>
            <li>
              <span class="forge-list-kicker">3. Queue and promote</span>
              <span class="forge-list-title">Promote only after SU-4 Blueprint gates write ${escapeHtml(lane.promotionRef)} for ${escapeHtml(lane.queueRef)}.</span>
            </li>
            <li>
              <span class="forge-list-kicker">4. Mirror</span>
              <span class="forge-list-title">Let SU-6 mirror ${escapeHtml(lane.mirrorRef)} to GitHub after Forge promotion; do not open a competing PR.</span>
            </li>
            <li>
              <span class="forge-list-kicker">Fallback</span>
              <span class="forge-list-title">${escapeHtml(lane.fallbackPath)}.</span>
            </li>
          </ol>
        </section>
        <section class="forge-lane-section">
          <div class="forge-panel-head">
            <h4 class="forge-panel-title">Workbench Lessons</h4>
            <span class="forge-table-caption">Linear-inspired operator cues</span>
          </div>
          <ul class="forge-list">
            ${lane.lessons
              .map(
                lesson => `<li class="forge-list-item">
                  <span class="forge-list-title">${escapeHtml(lesson)}</span>
                </li>`,
              )
              .join("\n")}
          </ul>
        </section>
        <section class="forge-lane-section">
          <div class="forge-panel-head">
            <h4 class="forge-panel-title">Cycle Metrics</h4>
            <span class="forge-table-caption">first lane measurement points</span>
          </div>
          <div class="forge-table-wrap">
            <table class="forge-table">
              <thead>
                <tr>
                  <th>Area</th>
                  <th>Signal</th>
                </tr>
              </thead>
              <tbody>
                ${Object.entries(lane.metrics)
                  .map(
                    ([area, signal]) => `<tr>
                      <td>${escapeHtml(area)}</td>
                      <td class="forge-muted">${escapeHtml(signal)}</td>
                    </tr>`,
                  )
                  .join("\n")}
              </tbody>
            </table>
          </div>
        </section>
      </article>`,
    )
    .join("\n")}
</section>`

const renderWorkQueue = (): string => `<section class="forge-panel" data-span="wide">
  <div class="forge-panel-head">
    <h3 class="forge-panel-title">Work Queue</h3>
    <span class="forge-table-caption">${forgeShellPreviewState.workQueue.length} records</span>
  </div>
  <div class="forge-table-wrap">
    <table class="forge-table">
      <thead>
        <tr>
          <th>Work</th>
          <th>Issue</th>
          <th>Title</th>
          <th>Priority</th>
          <th>Owner</th>
          <th>State</th>
          <th>Lease</th>
        </tr>
      </thead>
      <tbody>
        ${forgeShellPreviewState.workQueue
          .map(
            item => `<tr>
              <td class="forge-code">${escapeHtml(item.workRef)}</td>
              <td>${escapeHtml(item.issueRef)}</td>
              <td>${escapeHtml(item.title)}</td>
              <td>${escapeHtml(item.priority)}</td>
              <td>${escapeHtml(item.owner)}</td>
              <td>${renderStatus(item.state)}</td>
              <td class="forge-muted">${escapeHtml(item.lease)}</td>
            </tr>`,
          )
          .join("\n")}
      </tbody>
    </table>
  </div>
</section>`

const renderChanges = (): string => `<section class="forge-panel" data-span="wide">
  <div class="forge-panel-head">
    <h3 class="forge-panel-title">Change Inspector</h3>
    <span class="forge-table-caption">${forgeShellPreviewState.changes.length} changes</span>
  </div>
  <div class="forge-table-wrap">
    <table class="forge-table">
      <thead>
        <tr>
          <th>Change</th>
          <th>Work</th>
          <th>Base Head</th>
          <th>Patch Head</th>
          <th>Verification</th>
          <th>State</th>
          <th>Blockers</th>
        </tr>
      </thead>
      <tbody>
        ${forgeShellPreviewState.changes
          .map(
            item => `<tr>
              <td class="forge-code">${escapeHtml(item.changeRef)}</td>
              <td>${escapeHtml(item.workRef)}</td>
              <td class="forge-muted">${escapeHtml(item.baseHead)}</td>
              <td class="forge-code">${escapeHtml(item.patchHead)}</td>
              <td><span class="forge-code">${escapeHtml(item.verificationRef)}</span><br><span class="forge-muted">${escapeHtml(item.verificationState)}</span></td>
              <td>${renderStatus(item.state)}</td>
              <td class="forge-muted">${escapeHtml(item.blockers.length === 0 ? "none" : item.blockers.join(", "))}</td>
            </tr>`,
          )
          .join("\n")}
      </tbody>
    </table>
  </div>
</section>`

const renderVerification = (): string => `<section class="forge-panel" data-span="wide">
  <div class="forge-panel-head">
    <h3 class="forge-panel-title">Verification Receipts</h3>
    <span class="forge-table-caption">${forgeShellPreviewState.verification.length} receipts</span>
  </div>
  <div class="forge-table-wrap">
    <table class="forge-table">
      <thead>
        <tr>
          <th>Receipt</th>
          <th>Change</th>
          <th>Verdict</th>
          <th>Command</th>
          <th>Bound Refs</th>
          <th>Executor</th>
          <th>Log Digest</th>
        </tr>
      </thead>
      <tbody>
        ${forgeShellPreviewState.verification
          .map(
            item => `<tr>
              <td class="forge-code">${escapeHtml(item.receiptRef)}</td>
              <td>${escapeHtml(item.changeRef)}</td>
              <td>${renderStatus(item.verdict)}</td>
              <td class="forge-code">${escapeHtml(item.command)}</td>
              <td><span class="forge-muted">${escapeHtml(item.baseHead)}</span><br><span class="forge-code">${escapeHtml(item.headHead)}</span></td>
              <td>${escapeHtml(item.executor)}</td>
              <td class="forge-muted">${escapeHtml(item.logDigest)}</td>
            </tr>`,
          )
          .join("\n")}
      </tbody>
    </table>
  </div>
</section>`

const renderQueue = (): string => `<section class="forge-panel" data-span="wide">
  <div class="forge-panel-head">
    <h3 class="forge-panel-title">Virtual Merge Queue</h3>
    <span class="forge-table-caption">${forgeShellPreviewState.mergeQueue.length} lanes</span>
  </div>
  <div class="forge-table-wrap">
    <table class="forge-table">
      <thead>
        <tr>
          <th>Position</th>
          <th>Change</th>
          <th>Virtual Head</th>
          <th>Actual Head</th>
          <th>Gate</th>
          <th>State</th>
        </tr>
      </thead>
      <tbody>
        ${forgeShellPreviewState.mergeQueue
          .map(
            item => `<tr>
              <td>${escapeHtml(item.position)}</td>
              <td class="forge-code">${escapeHtml(item.changeRef)}</td>
              <td class="forge-code">${escapeHtml(item.virtualHead)}</td>
              <td>${escapeHtml(item.actualHead)}</td>
              <td class="forge-muted">${escapeHtml(item.gate)}</td>
              <td>${renderStatus(item.state)}</td>
            </tr>`,
          )
          .join("\n")}
      </tbody>
    </table>
  </div>
</section>`

const renderRefs = (): string => `<section class="forge-panel" data-span="wide">
  <div class="forge-panel-head">
    <h3 class="forge-panel-title">Canonical Refs</h3>
    <span class="forge-table-caption">${escapeHtml(OPENAGENTS_FORGE_TENANT_REF)} / ${escapeHtml(OPENAGENTS_FORGE_REPOSITORY_REF)}</span>
  </div>
  <div class="forge-table-wrap">
    <table class="forge-table">
      <thead>
        <tr>
          <th>Repository</th>
          <th>Ref</th>
          <th>Target</th>
          <th>Authority</th>
          <th>State</th>
        </tr>
      </thead>
      <tbody>
        ${forgeShellPreviewState.refs
          .map(
            item => `<tr>
              <td><span class="forge-code">${escapeHtml(item.tenantRef)}</span><br><span class="forge-muted">${escapeHtml(item.repositoryRef)}</span></td>
              <td class="forge-code">${escapeHtml(item.ref)}</td>
              <td>${escapeHtml(item.target)}</td>
              <td class="forge-muted">${escapeHtml(item.authority)}</td>
              <td>${renderStatus(item.state)}</td>
            </tr>`,
          )
          .join("\n")}
      </tbody>
    </table>
  </div>
</section>`

const renderRouteContent = (routeId: ForgeShellRouteId): string => {
  switch (routeId) {
    case "dogfood":
      return renderDogfood()
    case "work":
      return renderWorkQueue()
    case "changes":
      return renderChanges()
    case "verification":
      return renderVerification()
    case "queue":
      return renderQueue()
    case "refs":
      return renderRefs()
    case "overview":
      return renderOverview()
  }
}

export const forgeShellBody = (routeId: ForgeShellRouteId = "overview"): string => {
  const activeRoute = routeById(routeId)

  return `<main data-ui-family="forge/shell" data-forge-app="shell" data-forge-route="${escapeHtml(activeRoute.id)}" data-forge-data-mode="${escapeHtml(forgeShellPreviewState.dataMode)}" data-shared-ui-package="${escapeHtml(defaultForgeMount.uiPackage)}" class="forge-shell">
  <aside class="forge-rail" aria-label="Forge navigation">
    <section class="forge-brand">
      <p class="forge-host">${escapeHtml(defaultForgeMount.host)}</p>
      <h1 class="forge-brand-title">${escapeHtml(forgeLandingCopy.title)}</h1>
      <p class="forge-tagline">${escapeHtml(forgeLandingCopy.tagline)}</p>
    </section>
    <nav class="forge-nav">
${renderNav(activeRoute)}
    </nav>
  </aside>
  <section class="forge-main">
    <div class="forge-topbar">
      <div class="forge-topbar-left">
        <span class="forge-pill" data-tone="cyan">${escapeHtml(activeRoute.label)}</span>
        <span class="forge-pill">${escapeHtml(defaultForgeMount.runtime)}</span>
      </div>
      <div class="forge-topbar-right">
        <span class="forge-pill" data-tone="gold">${escapeHtml(FORGE_UI_WORKER_VERSION)}</span>
        <span class="forge-pill" data-tone="green">${escapeHtml(forgeShellPreviewState.dataMode)}</span>
      </div>
    </div>
    <section class="forge-stage">
      ${renderRouteHeader(activeRoute)}
      ${renderRouteContent(activeRoute.id)}
    </section>
  </section>
</main>`
}

export const forgeLandingBody = forgeShellBody

export const renderForgeShellHtml = (
  routeId: ForgeShellRouteId = "overview",
): string => {
  const route = routeById(routeId)

  return `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>${escapeHtml(route.label)} / ${escapeHtml(forgeLandingCopy.title)}</title>
  <meta name="description" content="${escapeHtml(forgeLandingCopy.tagline)}">
  <style>
${forgeShellStyles}
  </style>
</head>
<body>
${forgeShellBody(route.id)}
</body>
</html>`
}

export const renderForgeLandingHtml = (): string => renderForgeShellHtml("overview")

const htmlResponse = (body: string): Response =>
  new Response(body, {
    headers: {
      "content-type": "text/html; charset=utf-8",
      "cache-control": "no-store",
    },
  })

const jsonResponse = (body: unknown): Response =>
  Response.json(body, {
    headers: {
      "cache-control": "no-store",
    },
  })

export const forgeShellContract = () => ({
  service: "openagents-forge",
  version: FORGE_UI_WORKER_VERSION,
  mount: defaultForgeMount,
  routes: forgeShellRoutes,
  preview: forgeShellPreviewState,
})

export const handleForgeRequest = (
  request: Request,
): Effect.Effect<Response> =>
  Effect.gen(function* () {
    const url = new URL(request.url)
    const isHead = request.method === "HEAD"
    const isGet = request.method === "GET" || isHead
    const route = resolveForgeShellRoute(url.pathname)

    if (isGet && route !== null) {
      return htmlResponse(isHead ? "" : renderForgeShellHtml(route.id))
    }

    if (isGet && url.pathname === "/shell.json") {
      return jsonResponse(forgeShellContract())
    }

    if (isGet && url.pathname === "/health") {
      return jsonResponse({
        ok: true,
        service: "openagents-forge",
        version: FORGE_UI_WORKER_VERSION,
        mount: defaultForgeMount,
        shellRoutes: forgeShellRoutes.map(({ id, path, apiPath }) => ({
          id,
          path,
          apiPath,
        })),
      })
    }

    if (isGet && url.pathname === "/version") {
      return jsonResponse({
        service: "openagents-forge",
        version: FORGE_UI_WORKER_VERSION,
      })
    }

    return new Response("not found", {
      status: 404,
      headers: {
        "content-type": "text/plain; charset=utf-8",
        "cache-control": "no-store",
      },
    })
  })

export default {
  fetch(request: Request): Promise<Response> {
    return Effect.runPromise(handleForgeRequest(request))
  },
}
