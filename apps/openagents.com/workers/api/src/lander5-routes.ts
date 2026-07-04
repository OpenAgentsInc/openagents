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

// `/lander5` — lander4's business page with lander3's lazy Three.js hero,
// dimmed: the real landing-squares scene loads after `load`+idle and fades in
// BEHIND a ~90% near-black scrim, so the constellation reads as a quiet
// living texture under the business copy — the "we build this" pop without
// stealing focus from "Agents that work." Same SSR speed properties: the
// scene never touches the paint path; reduced-motion / Save-Data users keep
// the static grid.

type Lander5RouteInput = TokenLedgerRouteEnvSlice &
  Readonly<{
  OPENAGENTS_DB?: D1Database
  ledger?: TokenUsageLedgerShape
}>

// The scene layer sits above the static grid backdrop and below all content.
// The scrim is part of the layer, so the fade-in reveals an already-dimmed
// scene — never a bright flash.
const LANDER5_SCENE_CSS = `
#scene{position:fixed;inset:0;z-index:0;opacity:0;transition:opacity 1200ms ease}
#scene.ready{opacity:1}
#scene-mount{position:absolute;inset:0}
#scene .scrim{position:absolute;inset:0;background:rgba(7,10,15,0.9);pointer-events:none}
@media (prefers-reduced-motion:reduce){#scene{transition:none}}
`.trim()

const LANDER5_SCENE_SCRIPT = `
(function(){
  try{
    if(window.matchMedia&&matchMedia("(prefers-reduced-motion: reduce)").matches)return;
    if(navigator.connection&&navigator.connection.saveData)return;
  }catch(e){}
  function boot(){
    var idle=window.requestIdleCallback||function(f){setTimeout(f,250)};
    idle(function(){
      import("/assets/lander3-scene.js").then(function(m){
        var mount=document.getElementById("scene-mount");
        if(!mount)return null;
        performance.mark("oa:hero:import-done");
        return m.mountLander3Scene(mount);
      }).then(function(handle){
        if(!handle)return;
        performance.mark("oa:hero:first-frame");
        document.getElementById("scene").classList.add("ready");
      }).catch(function(){})
    });
  }
  if(document.readyState==="complete")boot();
  else addEventListener("load",boot);
})();
`.trim()

export const renderLander5Html = (tokensServed: number | null): string => {
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
<style>${LANDER_SHELL_CSS}
${LANDER5_SCENE_CSS}</style>
</head>
<body>
<div class="backdrop" aria-hidden="true"></div>
<div id="scene" aria-hidden="true"><div id="scene-mount"></div><div class="scrim"></div></div>
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
<script>${LANDER5_SCENE_SCRIPT}</script>
</body>
</html>
`
}

export const handleLander5Page = (
  request: Request,
  input: Lander5RouteInput,
  ctx?: OpenAgentsWorkerExecutionContext,
) => {
  if (request.method !== 'GET') {
    return Effect.succeed(methodNotAllowed(['GET']))
  }
  const ledger =
    input.ledger ?? tokenUsageLedgerFromRouteInput(input)
  const render = ledger.readPublicTokensServed().pipe(
    Effect.map(aggregate => renderLander5Html(aggregate.tokensServed)),
    Effect.catch(() => Effect.succeed(renderLander5Html(null))),
  )
  return edgeCachedLanderHtml(request, ctx, render)
}
