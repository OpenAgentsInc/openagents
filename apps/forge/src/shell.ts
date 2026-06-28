import { Effect } from "effect"

import { themeCss } from "@openagentsinc/ui/tokens"

export type ForgeRoute =
  | "queue"
  | "changes"
  | "verification"
  | "merge"
  | "refs"

type Provenance = "contract-placeholder" | "static-boundary"

type NavItem = Readonly<{
  route: ForgeRoute
  label: string
  count: number
}>

type QueueItem = Readonly<{
  ref: string
  title: string
  repo: string
  state: "ready" | "running" | "blocked" | "review"
  priority: string
  verification: string
}>

type ChangeFile = Readonly<{
  path: string
  adds: number
  removes: number
  status: "modified" | "added" | "deleted"
}>

type VerificationCheck = Readonly<{
  name: string
  command: string
  state: "passed" | "running" | "queued" | "blocked"
  receipt: string
}>

type MergeCandidate = Readonly<{
  ref: string
  title: string
  state: "ready" | "waiting" | "blocked"
  gate: string
}>

type GitRef = Readonly<{
  name: string
  sha: string
  role: string
  freshness: string
}>

const provenance: Provenance = "contract-placeholder"

const navItems: ReadonlyArray<NavItem> = [
  { route: "queue", label: "Work queue", count: 12 },
  { route: "changes", label: "Change inspector", count: 7 },
  { route: "verification", label: "Verification", count: 5 },
  { route: "merge", label: "Merge queue", count: 3 },
  { route: "refs", label: "Git refs", count: 9 },
]

const queueItems: ReadonlyArray<QueueItem> = [
  {
    ref: "forge.work.public.6769",
    title: "Separate Forge shell app",
    repo: "OpenAgentsInc/openagents",
    state: "running",
    priority: "P0",
    verification: "typecheck + app tests + deploy gate",
  },
  {
    ref: "forge.work.public.6745",
    title: "Forge API contract boundary",
    repo: "OpenAgentsInc/openagents",
    state: "ready",
    priority: "P0",
    verification: "schema drift guard",
  },
  {
    ref: "forge.work.public.6759",
    title: "Landing app extraction",
    repo: "OpenAgentsInc/openagents",
    state: "review",
    priority: "P1",
    verification: "route smoke",
  },
  {
    ref: "forge.work.public.api-next",
    title: "Public-safe shell hydration",
    repo: "OpenAgentsInc/openagents",
    state: "blocked",
    priority: "P1",
    verification: "NEEDS-API",
  },
]

const changeFiles: ReadonlyArray<ChangeFile> = [
  {
    path: "apps/forge/src/index.ts",
    adds: 64,
    removes: 0,
    status: "added",
  },
  {
    path: "apps/forge/src/shell.ts",
    adds: 520,
    removes: 0,
    status: "added",
  },
  {
    path: "apps/forge/wrangler.jsonc",
    adds: 48,
    removes: 0,
    status: "added",
  },
  {
    path: "package.json",
    adds: 2,
    removes: 1,
    status: "modified",
  },
]

const verificationChecks: ReadonlyArray<VerificationCheck> = [
  {
    name: "Forge typecheck",
    command: "bun run --cwd apps/forge typecheck",
    state: "passed",
    receipt: "forge.verify.typecheck.placeholder",
  },
  {
    name: "Forge shell tests",
    command: "bun run --cwd apps/forge test",
    state: "passed",
    receipt: "forge.verify.test.placeholder",
  },
  {
    name: "OpenAgents deploy gate",
    command: "bun run check:deploy",
    state: "running",
    receipt: "forge.verify.deploy.placeholder",
  },
  {
    name: "Production smoke",
    command: "curl -fsS https://forge.openagents.com/health",
    state: "queued",
    receipt: "forge.verify.production-smoke.pending",
  },
]

const mergeCandidates: ReadonlyArray<MergeCandidate> = [
  {
    ref: "merge.queue.public.6769",
    title: "FORGE SU-1B shell",
    state: "ready",
    gate: "local verification required",
  },
  {
    ref: "merge.queue.public.6745",
    title: "Forge contract schemas",
    state: "waiting",
    gate: "API schema owner review",
  },
  {
    ref: "merge.queue.public.docs",
    title: "Boundary documentation",
    state: "blocked",
    gate: "canonical runbook link missing",
  },
]

const gitRefs: ReadonlyArray<GitRef> = [
  {
    name: "origin/main",
    sha: "efde001",
    role: "deploy base",
    freshness: "pinned checkout",
  },
  {
    name: "apps/forge",
    sha: "contract-shell",
    role: "worker app",
    freshness: "local slice",
  },
  {
    name: "forge.openagents.com",
    sha: "wrangler env production",
    role: "public host",
    freshness: "deploy target",
  },
  {
    name: "openagents.com/forge",
    sha: "historical",
    role: "source material",
    freshness: "not canonical",
  },
]

const routeSet = new Set<ForgeRoute>(
  navItems.map(item => item.route),
)

export const normalizeForgeRoute = (pathname: string): ForgeRoute => {
  const segment = pathname.split("/").filter(Boolean)[0]
  return routeSet.has(segment as ForgeRoute) ? (segment as ForgeRoute) : "queue"
}

const escapeHtml = (value: string): string =>
  value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;")

const stateTone = (state: string): string => {
  switch (state) {
    case "passed":
    case "ready":
      return "good"
    case "running":
    case "review":
      return "info"
    case "queued":
    case "waiting":
      return "warn"
    case "blocked":
      return "bad"
    default:
      return "neutral"
  }
}

const nav = (activeRoute: ForgeRoute): string => navItems
  .map(item => `
    <a class="forge-nav-item${item.route === activeRoute ? " is-active" : ""}" href="/${item.route}" data-forge-route="${item.route}" aria-current="${item.route === activeRoute ? "page" : "false"}">
      <span>${escapeHtml(item.label)}</span>
      <span class="forge-count">${item.count}</span>
    </a>
  `)
  .join("")

const queuePanel = (): string => `
  <section class="forge-panel forge-panel-large" data-forge-panel="work-queue">
    <div class="forge-panel-heading">
      <div>
        <p class="forge-kicker">Work queue</p>
        <h2>Runnable backlog</h2>
      </div>
      <span class="forge-pill" data-forge-provenance="${provenance}">${provenance}</span>
    </div>
    <div class="forge-table" role="table" aria-label="Forge work queue">
      ${queueItems.map(item => `
        <div class="forge-row" role="row" data-forge-work-ref="${escapeHtml(item.ref)}" data-forge-work-state="${item.state}">
          <div>
            <strong>${escapeHtml(item.title)}</strong>
            <span>${escapeHtml(item.ref)}</span>
          </div>
          <div>${escapeHtml(item.repo)}</div>
          <div><span class="forge-state forge-state-${stateTone(item.state)}">${escapeHtml(item.state)}</span></div>
          <div>${escapeHtml(item.priority)}</div>
          <div>${escapeHtml(item.verification)}</div>
        </div>
      `).join("")}
    </div>
  </section>
`

const changePanel = (): string => `
  <section class="forge-panel" data-forge-panel="change-inspector">
    <div class="forge-panel-heading">
      <div>
        <p class="forge-kicker">Change inspector</p>
        <h2>Patch surface</h2>
      </div>
      <span class="forge-pill">${changeFiles.length} files</span>
    </div>
    <div class="forge-file-list">
      ${changeFiles.map(file => `
        <div class="forge-file" data-forge-file="${escapeHtml(file.path)}" data-forge-file-status="${file.status}">
          <span>${escapeHtml(file.path)}</span>
          <span class="forge-diff">+${file.adds} -${file.removes}</span>
        </div>
      `).join("")}
    </div>
  </section>
`

const verificationPanel = (): string => `
  <section class="forge-panel" data-forge-panel="verification-state">
    <div class="forge-panel-heading">
      <div>
        <p class="forge-kicker">Verification</p>
        <h2>Receipt ladder</h2>
      </div>
      <span class="forge-pill">${verificationChecks.length} gates</span>
    </div>
    <div class="forge-checks">
      ${verificationChecks.map(check => `
        <div class="forge-check" data-forge-verification="${escapeHtml(check.name)}" data-forge-verification-state="${check.state}">
          <span class="forge-dot forge-dot-${stateTone(check.state)}" aria-hidden="true"></span>
          <div>
            <strong>${escapeHtml(check.name)}</strong>
            <code>${escapeHtml(check.command)}</code>
          </div>
          <span>${escapeHtml(check.receipt)}</span>
        </div>
      `).join("")}
    </div>
  </section>
`

const mergePanel = (): string => `
  <section class="forge-panel" data-forge-panel="merge-queue">
    <div class="forge-panel-heading">
      <div>
        <p class="forge-kicker">Merge queue</p>
        <h2>Release candidates</h2>
      </div>
      <span class="forge-pill">${mergeCandidates.length} candidates</span>
    </div>
    <div class="forge-stack">
      ${mergeCandidates.map(candidate => `
        <div class="forge-card" data-forge-merge-ref="${escapeHtml(candidate.ref)}" data-forge-merge-state="${candidate.state}">
          <div>
            <strong>${escapeHtml(candidate.title)}</strong>
            <span>${escapeHtml(candidate.ref)}</span>
          </div>
          <span class="forge-state forge-state-${stateTone(candidate.state)}">${escapeHtml(candidate.state)}</span>
          <p>${escapeHtml(candidate.gate)}</p>
        </div>
      `).join("")}
    </div>
  </section>
`

const refsPanel = (): string => `
  <section class="forge-panel" data-forge-panel="git-ref-views">
    <div class="forge-panel-heading">
      <div>
        <p class="forge-kicker">Git refs</p>
        <h2>Checkout projection</h2>
      </div>
      <span class="forge-pill">${gitRefs.length} refs</span>
    </div>
    <div class="forge-ref-map" aria-label="Forge git reference map">
      ${gitRefs.map((ref, index) => `
        <div class="forge-ref" data-forge-ref="${escapeHtml(ref.name)}">
          <span class="forge-ref-node" aria-hidden="true">${index + 1}</span>
          <div>
            <strong>${escapeHtml(ref.name)}</strong>
            <code>${escapeHtml(ref.sha)}</code>
          </div>
          <span>${escapeHtml(ref.role)}</span>
          <span>${escapeHtml(ref.freshness)}</span>
        </div>
      `).join("")}
    </div>
  </section>
`

const routePanel = (activeRoute: ForgeRoute): string => {
  switch (activeRoute) {
    case "queue":
      return queuePanel()
    case "changes":
      return changePanel()
    case "verification":
      return verificationPanel()
    case "merge":
      return mergePanel()
    case "refs":
      return refsPanel()
  }
}

const adjacentPanels = (activeRoute: ForgeRoute): string => {
  const panels = [
    activeRoute === "changes" ? queuePanel() : changePanel(),
    activeRoute === "verification" ? mergePanel() : verificationPanel(),
    activeRoute === "refs" ? mergePanel() : refsPanel(),
  ]
  return panels.join("")
}

const styles = (): string => `
${themeCss()}
:root {
  color-scheme: dark;
  --forge-bg: #050607;
  --forge-panel: #0c0f13;
  --forge-panel-2: #101317;
  --forge-line: #263241;
  --forge-line-soft: rgba(143, 182, 255, 0.18);
  --forge-text: #eef4ff;
  --forge-body: #c9d2dd;
  --forge-muted: #aeb9c6;
  --forge-faint: #7e8a98;
  --forge-blue: #3a7bff;
  --forge-cyan: #4fd0ff;
  --forge-green: #2bd576;
  --forge-yellow: #f5c542;
  --forge-red: #ff7070;
}
* { box-sizing: border-box; }
html { min-height: 100%; background: var(--forge-bg); }
body {
  margin: 0;
  min-height: 100vh;
  background:
    radial-gradient(circle at 18% 8%, rgba(58, 123, 255, 0.16), transparent 28rem),
    linear-gradient(180deg, #050607 0%, #0a0d12 100%);
  color: var(--forge-body);
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  letter-spacing: 0;
}
a { color: inherit; }
.forge-shell {
  min-height: 100vh;
  display: grid;
  grid-template-columns: 264px minmax(0, 1fr);
}
.forge-sidebar {
  position: sticky;
  top: 0;
  height: 100vh;
  border-right: 1px solid var(--forge-line);
  background: rgba(5, 6, 7, 0.92);
  padding: 24px 16px;
}
.forge-brand {
  display: grid;
  gap: 8px;
  padding: 0 8px 20px;
  border-bottom: 1px solid var(--forge-line);
}
.forge-brand strong {
  color: var(--forge-text);
  font-size: 18px;
  font-weight: 650;
}
.forge-brand span,
.forge-panel-heading p,
.forge-row span,
.forge-card span,
.forge-check span,
.forge-ref span {
  color: var(--forge-muted);
}
.forge-nav {
  display: grid;
  gap: 6px;
  margin-top: 18px;
}
.forge-nav-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  min-height: 40px;
  padding: 0 10px;
  border: 1px solid transparent;
  border-radius: 6px;
  color: var(--forge-muted);
  text-decoration: none;
}
.forge-nav-item:hover,
.forge-nav-item.is-active {
  border-color: var(--forge-line-soft);
  background: rgba(58, 123, 255, 0.08);
  color: var(--forge-text);
}
.forge-count,
.forge-pill,
.forge-state {
  display: inline-flex;
  min-height: 22px;
  align-items: center;
  border: 1px solid var(--forge-line);
  border-radius: 999px;
  padding: 0 8px;
  font-size: 12px;
}
.forge-main {
  min-width: 0;
  padding: 24px;
}
.forge-topbar {
  display: flex;
  flex-wrap: wrap;
  gap: 16px;
  align-items: end;
  justify-content: space-between;
  margin-bottom: 18px;
}
.forge-title {
  display: grid;
  gap: 8px;
  max-width: 72ch;
}
.forge-kicker {
  margin: 0;
  color: #8fb6ff;
  font-size: 12px;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}
h1,
h2,
p {
  margin: 0;
}
h1 {
  color: var(--forge-text);
  font-size: 28px;
  line-height: 1.12;
  font-weight: 680;
  text-wrap: balance;
}
h2 {
  color: var(--forge-text);
  font-size: 16px;
  line-height: 1.2;
  font-weight: 650;
}
.forge-title p:last-child {
  max-width: 70ch;
  line-height: 1.55;
}
.forge-toolbar {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}
.forge-tool {
  min-height: 36px;
  border: 1px solid var(--forge-line);
  border-radius: 6px;
  background: var(--forge-panel);
  color: var(--forge-text);
  padding: 0 12px;
  font: inherit;
}
.forge-grid {
  display: grid;
  grid-template-columns: minmax(0, 1.35fr) minmax(320px, 0.65fr);
  gap: 16px;
  align-items: start;
}
.forge-side-stack {
  display: grid;
  gap: 16px;
}
.forge-panel {
  min-width: 0;
  border: 1px solid var(--forge-line);
  border-radius: 8px;
  background: rgba(12, 15, 19, 0.94);
}
.forge-panel-heading {
  display: flex;
  gap: 12px;
  align-items: start;
  justify-content: space-between;
  padding: 16px;
  border-bottom: 1px solid var(--forge-line);
}
.forge-table,
.forge-file-list,
.forge-checks,
.forge-stack,
.forge-ref-map {
  display: grid;
}
.forge-row {
  display: grid;
  grid-template-columns: minmax(220px, 1.5fr) minmax(160px, 1fr) 96px 56px minmax(180px, 1fr);
  gap: 12px;
  align-items: center;
  min-height: 64px;
  padding: 12px 16px;
  border-bottom: 1px solid rgba(38, 50, 65, 0.72);
}
.forge-row:last-child,
.forge-file:last-child,
.forge-check:last-child,
.forge-ref:last-child {
  border-bottom: 0;
}
.forge-row > div:first-child,
.forge-card > div,
.forge-check > div,
.forge-ref > div {
  display: grid;
  gap: 4px;
  min-width: 0;
}
strong {
  color: var(--forge-text);
  font-weight: 650;
}
code {
  color: #d7e2f0;
  font-family: inherit;
  overflow-wrap: anywhere;
}
.forge-state-good,
.forge-dot-good { color: var(--forge-green); border-color: rgba(43, 213, 118, 0.45); }
.forge-state-info,
.forge-dot-info { color: var(--forge-cyan); border-color: rgba(79, 208, 255, 0.45); }
.forge-state-warn,
.forge-dot-warn { color: var(--forge-yellow); border-color: rgba(245, 197, 66, 0.45); }
.forge-state-bad,
.forge-dot-bad { color: var(--forge-red); border-color: rgba(255, 112, 112, 0.45); }
.forge-file,
.forge-check,
.forge-ref {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 12px;
  align-items: center;
  min-height: 52px;
  padding: 12px 16px;
  border-bottom: 1px solid rgba(38, 50, 65, 0.72);
}
.forge-diff {
  color: var(--forge-green);
}
.forge-check {
  grid-template-columns: 14px minmax(0, 1fr) minmax(120px, auto);
}
.forge-dot {
  width: 8px;
  height: 8px;
  border: 1px solid currentColor;
  border-radius: 50%;
  background: currentColor;
}
.forge-card {
  display: grid;
  gap: 10px;
  padding: 14px 16px;
  border-bottom: 1px solid rgba(38, 50, 65, 0.72);
}
.forge-card:last-child {
  border-bottom: 0;
}
.forge-card p {
  color: var(--forge-muted);
  line-height: 1.45;
}
.forge-ref {
  grid-template-columns: 28px minmax(0, 1fr) minmax(96px, auto) minmax(120px, auto);
}
.forge-ref-node {
  display: grid;
  place-items: center;
  width: 24px;
  height: 24px;
  border: 1px solid rgba(58, 123, 255, 0.5);
  border-radius: 50%;
  color: var(--forge-cyan);
}
@media (max-width: 980px) {
  .forge-shell {
    grid-template-columns: 1fr;
  }
  .forge-sidebar {
    position: static;
    height: auto;
    border-right: 0;
    border-bottom: 1px solid var(--forge-line);
  }
  .forge-nav {
    grid-template-columns: repeat(auto-fit, minmax(170px, 1fr));
  }
  .forge-grid {
    grid-template-columns: 1fr;
  }
}
@media (max-width: 760px) {
  .forge-main {
    padding: 16px;
  }
  .forge-row,
  .forge-file,
  .forge-check,
  .forge-ref {
    grid-template-columns: 1fr;
    align-items: start;
  }
  .forge-topbar {
    align-items: stretch;
  }
  .forge-toolbar {
    width: 100%;
  }
  .forge-tool {
    flex: 1 1 140px;
  }
}
@media (prefers-reduced-motion: reduce) {
  *,
  *::before,
  *::after {
    scroll-behavior: auto !important;
    transition-duration: 0.01ms !important;
    animation-duration: 0.01ms !important;
  }
}
`

export const renderForgeShell = (input: {
  route: ForgeRoute
  generatedAt: string
}): string => {
  const activeLabel = navItems.find(item => item.route === input.route)?.label ?? "Work queue"
  return `<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>Forge · ${escapeHtml(activeLabel)}</title>
    <style>${styles()}</style>
  </head>
  <body>
    <div class="forge-shell" data-forge-app-shell="true" data-forge-active-route="${input.route}" data-forge-generated-at="${escapeHtml(input.generatedAt)}" data-forge-provenance="${provenance}">
      <aside class="forge-sidebar">
        <div class="forge-brand">
          <strong>Forge</strong>
          <span>Separate app shell · forge.openagents.com</span>
        </div>
        <nav class="forge-nav" aria-label="Forge shell routes">
          ${nav(input.route)}
        </nav>
      </aside>
      <main class="forge-main">
        <header class="forge-topbar">
          <div class="forge-title">
            <p class="forge-kicker">Contract shell</p>
            <h1>${escapeHtml(activeLabel)}</h1>
            <p>Public-safe operator surface for queueing work, inspecting changes, tracking verification receipts, staging merge candidates, and checking git references.</p>
          </div>
          <div class="forge-toolbar" aria-label="Forge actions">
            <button class="forge-tool" type="button" data-forge-action="refresh">Refresh</button>
            <button class="forge-tool" type="button" data-forge-action="filter">Filter</button>
            <button class="forge-tool" type="button" data-forge-action="export">Export</button>
          </div>
        </header>
        <div class="forge-grid">
          ${routePanel(input.route)}
          <div class="forge-side-stack">
            ${adjacentPanels(input.route)}
          </div>
        </div>
      </main>
    </div>
  </body>
</html>`
}

export const renderForgeShellEffect = (input: {
  route: ForgeRoute
  generatedAt: string
}): Effect.Effect<string> => Effect.succeed(renderForgeShell(input))
