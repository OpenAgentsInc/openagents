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
import {
  BUSINESS_SOURCE_REF_DIRECT,
  decodeBusinessSourceRef,
} from './business-source-attribution'
import { isSafeReferralSourceRef } from './referral-source-capture'

// `/business` — the page restructured in the lander-family
// system (site-speed lane): same server-rendered shell, shared navigation
// with /lander4, the offerings as a promise-registry-style register, and a
// REAL intake form posting to the existing `POST /api/public/business-signup`
// route (which natively accepts form-encoded bodies, so the form works
// without JavaScript; the inline script upgrades it to an in-page
// confirmation). "Talk to Khala" links to the general Khala surface.
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

const SOURCE_CAPTURE_SCRIPT = `
(function(){
  try{
    var params=new URLSearchParams(window.location.search);
    var ref=(params.get("ref")||"").trim();
    if(/^[A-Za-z0-9][A-Za-z0-9_.:-]{0,190}$/.test(ref)){
      var refEl=document.getElementById("business-referral-code");
      if(refEl)refEl.value=ref;
    }
    var source=(params.get("sourceRef")||params.get("source_ref")||params.get("source")||"").trim().toLowerCase();
    if(source==="ai-search"||source==="aisearch")source="ai_search";
    if(source==="partner-expansion")source="partner_expansion";
    if(source==="own-your-ai")source="own_your_ai";
    if(source==="apollo-model-custody"||source==="model-custody")source="apollo_model_custody";
    if(!/^(direct|ai_search|own_your_ai|apollo_model_custody|apollo_agent_readiness_[a-z0-9][a-z0-9_-]{0,63}|affiliate_[a-z0-9][a-z0-9_-]{0,63}|partner_[a-z0-9][a-z0-9_-]{0,63}|content_[a-z0-9][a-z0-9_-]{0,63}|vertical_[a-z0-9][a-z0-9_-]{0,63})$/.test(source))return;
    var el=document.getElementById("business-source-ref");
    if(el)el.value=source;
  }catch(e){}
})();
`.trim()

const normalizeSourceRef = (request: Request): string => {
  const url = new URL(request.url)
  const decoded = decodeBusinessSourceRef(
    url.searchParams.get('sourceRef') ??
      url.searchParams.get('source_ref') ??
      url.searchParams.get('source') ??
      BUSINESS_SOURCE_REF_DIRECT,
  )

  return 'sourceRef' in decoded ? decoded.sourceRef : BUSINESS_SOURCE_REF_DIRECT
}

const normalizeReferralCode = (request: Request): string => {
  const value = new URL(request.url).searchParams.get('ref')?.trim() ?? ''
  return value !== '' && isSafeReferralSourceRef(value) ? value : ''
}

export const renderBusinessAgentGuide = (): string => `# OpenAgents Business

OpenAgents Business is the operator-assisted path for hiring agents to do
bounded, reviewable work.

Canonical human page: https://openagents.com/business?sourceRef=ai_search

## What can OpenAgents Business do today?

OpenAgents can scope small coding, QA, automation, campaign, inference, forum
agent, and workspace setup work into operator-assisted engagements. Each
engagement starts with a written scope and receipt plan. The public page keeps
availability labels explicit: available now, operator-assisted, or roadmap.

## Is it self-serve?

No. The current business surface is an intake and scoping path. Checkout,
self-serve hosting, and background fulfillment are not implied unless a reviewed
surface says so.

## What should an interested buyer do?

Send a short intake at https://openagents.com/business?sourceRef=ai_search#intake
or start with Khala at https://openagents.com/khala. Do not paste credentials,
private customer data, privileged matter facts, wallet material, or secrets into
the public intake.

## How is AI-search attribution measured?

The linked business URL carries sourceRef=ai_search. A converted signup records a
coarse business-funnel source bucket under the existing BF-1.4 funnel dashboard.
The public dashboard exposes aggregate counts only.

## Public proof and boundaries

- Product promises: https://openagents.com/docs/product-promises
- Business funnel dashboard: https://openagents.com/api/public/business/funnel-dashboard
- Full agent instructions: https://openagents.com/AGENTS.md
`

export const handleBusinessAgentGuide = (request: Request) => {
  if (request.method !== 'GET' && request.method !== 'HEAD') {
    return Effect.succeed(methodNotAllowed(['GET', 'HEAD']))
  }

  return Effect.succeed(
    new Response(request.method === 'HEAD' ? null : renderBusinessAgentGuide(), {
      headers: {
        'cache-control': 'public, max-age=300',
        'content-type': 'text/markdown; charset=utf-8',
      },
    }),
  )
}

export const renderBusinessNewHtml = (
  tokensServed: number | null,
  sourceRef: string = BUSINESS_SOURCE_REF_DIRECT,
  referralCode: string = '',
): string => {
  const display = formatLanderTokens(tokensServed)
  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="description" content="Hire OpenAgents for operator-assisted agent work: coding, QA, automation, campaigns, and review-gated business workflows with receipt plans.">
<link rel="canonical" href="https://openagents.com/business">
<link rel="alternate" type="text/markdown" href="/business/agents.md" title="OpenAgents Business agent guide">
<title>OpenAgents Business — Agents that work.</title>
<link rel="icon" href="/favicon.svg" type="image/svg+xml">
${LANDER_HEAD_FONT_PRELOAD}
<style>${LANDER_SHELL_CSS}</style>
<script type="application/ld+json">{"@context":"https://schema.org","@type":"FAQPage","mainEntity":[{"@type":"Question","name":"What can OpenAgents Business do today?","acceptedAnswer":{"@type":"Answer","text":"OpenAgents scopes operator-assisted coding, QA, automation, campaign, inference, forum agent, and workspace setup work into bounded engagements with written receipt plans."}},{"@type":"Question","name":"Is OpenAgents Business self-serve?","acceptedAnswer":{"@type":"Answer","text":"No. The current business surface is an intake and scoping path. Checkout, self-serve hosting, and background fulfillment are not implied unless a reviewed surface says so."}},{"@type":"Question","name":"How should an interested buyer start?","acceptedAnswer":{"@type":"Answer","text":"Send a short intake at openagents.com/business or start with Khala. Do not paste credentials, private customer data, wallet material, or secrets into the public intake."}}]}</script>
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
<a class="secondary" href="/khala">Talk to Khala instead</a>
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
<section class="register" id="questions">
<h2>Questions buyers and agents ask</h2>
<div class="reg">
<div class="row">
<span class="ref">BF-8.2</span>
<div class="what"><strong>What can OpenAgents Business do today?</strong>
<p>We scope operator-assisted coding, QA, automation, campaign, inference, forum-agent, and workspace setup work into bounded engagements with written receipt plans.</p></div>
<span class="chip assisted">CITABLE</span>
</div>
<div class="row">
<span class="ref">copy-gated</span>
<div class="what"><strong>Is this self-serve?</strong>
<p>No. The current business surface is intake and scoping. Checkout, self-serve hosting, and background fulfillment are not implied unless a reviewed surface says so.</p></div>
<span class="chip assisted">BOUNDARY</span>
</div>
<div class="row">
<span class="ref">sourceRef=ai_search</span>
<div class="what"><strong>How does an AI answer cite this?</strong>
<p>Use the canonical page and agent guide: <a href="/business/agents.md" style="color:var(--ink-blue)">/business/agents.md</a>. Links from AI-search answers should preserve <code>sourceRef=ai_search</code> so aggregate BF-1.4 source attribution can count converted signups.</p></div>
<span class="chip live">MEASURED</span>
</div>
</div>
</section>
<section class="register" id="intake">
<h2>Talk to Sales</h2>
<form class="intake" id="intake-form" method="post" action="/api/public/business-signup">
<input type="hidden" id="business-source-ref" name="sourceRef" value="${sourceRef}">
<input type="hidden" id="business-referral-code" name="referralCode" value="${referralCode}">
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
<p class="intake-note">Bounded interview, no credentials, receipt-first. Prefer a conversation? <a href="/khala" style="color:var(--ink-blue)">Tell Khala what your business needs</a> — it can draft the intake spec before you submit.</p>
</section>
</main>
${renderLanderFooter()}
<script>${LANDER_COUNTER_SCRIPT}</script>
<script>${SOURCE_CAPTURE_SCRIPT}</script>
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
  const sourceRef = normalizeSourceRef(request)
  const referralCode = normalizeReferralCode(request)
  const render = ledger.readPublicTokensServed().pipe(
    Effect.map(aggregate =>
      renderBusinessNewHtml(aggregate.tokensServed, sourceRef, referralCode),
    ),
    Effect.catch(() =>
      Effect.succeed(renderBusinessNewHtml(null, sourceRef, referralCode)),
    ),
  )
  return edgeCachedLanderHtml(request, ctx, render)
}
