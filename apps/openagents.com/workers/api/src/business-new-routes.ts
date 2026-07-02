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
  makeD1TokenUsageLedger,
} from './token-usage-ledger'

// `/business-new` — the /business page restructured in the lander-family
// system (site-speed lane): same server-rendered shell, shared navigation
// with /lander4, the offerings as a promise-registry-style register, and a
// REAL intake form posting to the existing `POST /api/public/business-signup`
// route (which natively accepts form-encoded bodies, so the form works
// without JavaScript; the inline script upgrades it to an in-page
// confirmation). "Talk to Khala" links to the live /business interview.
//
// Copy discipline: hero, trust, offering titles/bodies/caveats, and promise
// refs reuse the LIVE /business page copy verbatim; field labels match the
// signup route's actual contract. Unlisted, noindex experiment surface.

type BusinessNewRouteInput = Readonly<{
  OPENAGENTS_DB?: D1Database
  ledger?: TokenUsageLedgerShape
}>

const INTAKE_SCRIPT = `
(function(){
  var form=document.getElementById("intake-form");
  if(!form||!window.fetch)return;
  var result=document.getElementById("intake-result");
  var button=form.querySelector("button");
  form.addEventListener("submit",function(event){
    event.preventDefault();
    button.disabled=true;
    result.className="intake-result wide";
    fetch("/api/public/business-signup",{method:"POST",body:new FormData(form)})
      .then(function(r){return r.json().then(function(d){return{ok:r.ok,d:d}})})
      .then(function(out){
        if(out.ok){
          result.textContent="Received — we'll follow up at the email you gave. You can also start the Khala interview any time.";
          result.className="intake-result wide ok";
          form.reset();
        }else{
          result.textContent=(out.d&&out.d.reason)||"That didn't go through — check the required fields and try again.";
          result.className="intake-result wide err";
        }
      })
      .catch(function(){
        result.textContent="Network hiccup — nothing was lost; try again.";
        result.className="intake-result wide err";
      })
      .then(function(){button.disabled=false});
  });
})();
`.trim()

export const renderBusinessNewHtml = (tokensServed: number | null): string => {
  const display = formatLanderTokens(tokensServed)
  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="robots" content="noindex">
<title>OpenAgents Business — Agents that work.</title>
<link rel="icon" href="/favicon.svg" type="image/svg+xml">
${LANDER_HEAD_FONT_PRELOAD}
<style>${LANDER_SHELL_CSS}</style>
</head>
<body>
<div class="backdrop" aria-hidden="true"></div>
${renderLanderHeader('business', display)}
<main class="shell">
<h1 class="rise">Agents that work<span class="mark">.</span></h1>
<p class="sub rise d1">Hire agents from the OpenAgents network to get real work done — software built fast, campaigns drafted, batches processed — delivered with verifiable receipts.</p>
<p class="trust rise d2">Start with a fast quick win we can deliver in days, then put recurring work on Autopilot as trust builds. Every accepted outcome ties to evidence; every paid run is scoped with a receipt plan up front; a human-review gate sits before anything ships, sends, or spends.</p>
<div class="cta rise d3">
<a class="primary" href="#intake">Talk to Sales</a>
<a class="secondary" href="/business">Talk to Khala instead</a>
</div>
<section class="register">
<h2>What we can do</h2>
<div class="reg">
<div class="row">
<span class="ref">business.coding_quick_win.v1</span>
<div class="what"><strong>Coding &amp; agent work</strong>
<p>A coding agent takes a written objective, works in your repo, runs your verification command, and hands back a reviewable change with evidence. Quick win: fix a failing test suite, refactor a messy module, or add one feature with passing tests.</p></div>
<span class="chip assisted">OPERATOR-ASSISTED</span>
</div>
<div class="row">
<span class="ref">inference.gateway_credits_business.v1</span>
<div class="what"><strong>Inference / AI on tap</strong>
<p>Open-weight model inference through OpenAgents, with a bounded free taste and scoped paid usage.</p></div>
<span class="chip assisted">OPERATOR-ASSISTED</span>
</div>
<div class="row">
<span class="ref">metrics.khala_tokens_served_public.v1</span>
<div class="what"><strong>A live, public network</strong>
<p>Network activity is published as exact public counters — the same number ticking at the top of this page.</p></div>
<span class="chip live">LIVE</span>
</div>
</div>
<p class="intake-note">An honest menu of what OpenAgents can deliver. Availability is grounded in our public product-promise registry — shipped now, operator-assisted with a caveat, or planned roadmap. We say so in writing and scope the smallest honest version.</p>
</section>
<section class="register" id="intake">
<h2>Talk to Sales</h2>
<form class="intake" id="intake-form" method="post" action="/api/public/business-signup">
<label class="check wide optin"><input type="checkbox" name="requestSlackChannel" value="true">Set up a shared Slack channel</label>
<label><span class="lab">Business name *</span>
<input type="text" name="businessName" required maxlength="200" autocomplete="organization"></label>
<label><span class="lab">Contact email *</span>
<input type="email" name="contactEmail" required maxlength="320" autocomplete="email"></label>
<label><span class="lab">Phone *</span>
<input type="tel" name="phone" required maxlength="80" autocomplete="tel"></label>
<label><span class="lab">Website</span>
<input type="url" name="website" maxlength="320" placeholder="https://" autocomplete="url"></label>
<label class="wide"><span class="lab">What do you need?</span>
<textarea name="helpWith" maxlength="2000" placeholder="A stuck task, a repetitive grind, software you wish existed."></textarea></label>
<div class="intake-result wide" id="intake-result" role="status"></div>
<button type="submit">Send it</button>
</form>
<p class="intake-note">Bounded interview, no credentials, receipt-first. Prefer a conversation? <a href="/business" style="color:var(--ink-blue)">Tell Khala what your business needs</a> — it runs a short interview and drafts your intake spec.</p>
</section>
</main>
${renderLanderFooter()}
<script>${LANDER_COUNTER_SCRIPT}</script>
<script>${INTAKE_SCRIPT}</script>
</body>
</html>
`
}

export const handleBusinessNewPage = (
  request: Request,
  input: BusinessNewRouteInput,
  ctx?: OpenAgentsWorkerExecutionContext,
) => {
  if (request.method !== 'GET') {
    return Effect.succeed(methodNotAllowed(['GET']))
  }
  const ledger =
    input.ledger ?? makeD1TokenUsageLedger(input.OPENAGENTS_DB as D1Database)
  const render = ledger.readPublicTokensServed().pipe(
    Effect.map(aggregate => renderBusinessNewHtml(aggregate.tokensServed)),
    Effect.catch(() => Effect.succeed(renderBusinessNewHtml(null))),
  )
  return edgeCachedLanderHtml(request, ctx, render)
}
