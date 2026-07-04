import { Effect } from 'effect'

import { edgeCachedLanderHtml } from './lander-page-cache'
import type { OpenAgentsWorkerExecutionContext } from './runtime'
import {
  LANDER_COUNTER_SCRIPT,
  LANDER_HEAD_FONT_PRELOAD,
  LANDER_SHELL_CSS,
  formatLanderTokens,
  renderLanderFooter,
  renderLanderHeader,
} from './lander-shell'
import { methodNotAllowed } from './http/responses'
import {
  type TokenUsageLedgerShape,
} from './token-usage-ledger'
import {
  tokenUsageLedgerFromRouteInput,
  type TokenLedgerRouteEnvSlice,
} from './token-ledger-store'

// `/lander4` — the business-facing landing experiment from the site-speed
// lane ("Agents that work.", ROADMAP_AFTER AW-0), v2 after the impeccable
// design pass: headlines in the house Berkeley Mono Bold (the licensed
// webfonts the SPA already serves from /fonts/), the blue terminal period as
// the one brand mark, and a promise-registry-style capability register
// instead of a card grid. Server-rendered, edge-cached for 20 s (the browser
// never caches; the inline refresher keeps the counter live).
//
// Copy discipline: headline, subhead, trust paragraph, register bodies, and
// the footer line reuse the LIVE /business page copy verbatim. "Talk to
// Sales" is the owner-directed (2026-07-02) label for the business intake.
// Unlisted, noindex measurement surface.

type Lander4RouteInput = TokenLedgerRouteEnvSlice &
  Readonly<{
  OPENAGENTS_DB?: D1Database
  ledger?: TokenUsageLedgerShape
}>

export const renderLander4Html = (tokensServed: number | null): string => {
  const display = formatLanderTokens(tokensServed)
  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="robots" content="noindex">
<title>OpenAgents — Agents that work.</title>
<link rel="icon" href="/favicon.svg" type="image/svg+xml">
${LANDER_HEAD_FONT_PRELOAD}
<style>${LANDER_SHELL_CSS}</style>
</head>
<body>
<div class="backdrop" aria-hidden="true"></div>
${renderLanderHeader('home', display)}
<main class="shell">
<h1 class="rise">Agents that work<span class="mark">.</span></h1>
<p class="sub rise d1">Hire agents from the OpenAgents network to get real work done — software built fast, campaigns drafted, batches processed — delivered with verifiable receipts.</p>
<p class="trust rise d2">Start with a fast quick win we can deliver in days, then put recurring work on Autopilot as trust builds. Every accepted outcome ties to evidence; every paid run is scoped with a receipt plan up front; a human-review gate sits before anything ships, sends, or spends.</p>
<div class="cta rise d3">
<a class="primary" href="/business-new">Talk to Sales</a>
<a class="secondary" href="/stats">See the live network</a>
</div>
<section class="register">
<h2>What we can do</h2>
<div class="reg">
<div class="row">
<span class="ref">business.coding_quick_win.v1</span>
<div class="what"><strong>Software built fast</strong>
<p>A coding agent takes a written objective, works in your repo, runs your verification command, and hands back a reviewable change with evidence.</p></div>
<span class="chip assisted">OPERATOR-ASSISTED</span>
</div>
<div class="row">
<span class="ref">metrics.khala_tokens_served_public.v1</span>
<div class="what"><strong>A live, public network</strong>
<p>Network activity is published as exact public counters — the same number ticking at the top of this page.</p></div>
<span class="chip live">LIVE</span>
</div>
<div class="row">
<span class="ref">business.intake_quick_win_offering.v1</span>
<div class="what"><strong>A human gate</strong>
<p>Every paid run is scoped with a receipt plan up front; a human-review gate sits before anything ships, sends, or spends.</p></div>
<span class="chip assisted">OPERATOR-ASSISTED</span>
</div>
</div>
</section>
</main>
${renderLanderFooter()}
<script>${LANDER_COUNTER_SCRIPT}</script>
</body>
</html>
`
}

export const handleLander4Page = (
  request: Request,
  input: Lander4RouteInput,
  ctx?: OpenAgentsWorkerExecutionContext,
) => {
  if (request.method !== 'GET') {
    return Effect.succeed(methodNotAllowed(['GET']))
  }
  const ledger =
    input.ledger ?? tokenUsageLedgerFromRouteInput(input)
  const render = ledger.readPublicTokensServed().pipe(
    Effect.map(aggregate => renderLander4Html(aggregate.tokensServed)),
    // Ledger failure renders the placeholder page rather than a 500; the
    // inline refresher fills the number as soon as the endpoint answers.
    Effect.catch(() => Effect.succeed(renderLander4Html(null))),
  )
  return edgeCachedLanderHtml(request, ctx, render)
}
