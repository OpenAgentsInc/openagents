import { Effect } from 'effect'

import { methodNotAllowed } from './http/responses'
import { currentEpochMillis } from './runtime-primitives'
import {
  type TokenUsageLedgerShape,
  makeD1TokenUsageLedger,
} from './token-usage-ledger'

// `/lander4` — the business-facing landing experiment from the site-speed
// lane, aimed at the "Agents that work." services motion (ROADMAP_AFTER
// AW-0). Same server-rendered speed architecture as /lander2 (one HTML
// document, inline CSS, SSR token total, no bundle), styled as a subtler,
// friendlier take on the StarCraft palette: soft slate near-blacks, restrained
// energy-blue accents, generous whitespace, no WebGL.
//
// Copy discipline: the headline, subhead, trust paragraph, and registry line
// reuse the LIVE `/business` page copy verbatim (already through the copy
// gates). "Talk to Sales" is an owner-directed (2026-07-02) navigation label
// for the existing /business intake — a link label, not a new capability
// claim. Unlisted, noindex measurement surface.

type Lander4RouteInput = Readonly<{
  OPENAGENTS_DB?: D1Database
  ledger?: TokenUsageLedgerShape
  nowMs?: () => number
}>

const formatTokens = (total: number): string => total.toLocaleString('en-US')

const LANDER4_CSS = `
*{box-sizing:border-box;margin:0;padding:0}
html{height:100%}
body{
  min-height:100%;background:#0a0e14;color:#e8e9ec;
  font-family:ui-sans-serif,-apple-system,"SF Pro Text",Inter,system-ui,sans-serif;
  -webkit-font-smoothing:antialiased;line-height:1.6}
.mono{font-family:"Berkeley Mono",ui-monospace,SFMono-Regular,Menlo,monospace}
.backdrop{position:fixed;inset:0;pointer-events:none;
  background-image:
    radial-gradient(ellipse 70% 50% at 50% 0%,rgba(58,123,255,0.08),transparent 65%),
    repeating-linear-gradient(0deg,rgba(58,123,255,0.022) 0 1px,transparent 1px 32px),
    repeating-linear-gradient(90deg,rgba(58,123,255,0.022) 0 1px,transparent 1px 32px)}
header{position:relative;z-index:1;display:flex;justify-content:space-between;align-items:center;
  padding:20px 28px;max-width:1080px;margin:0 auto}
.brand{font-weight:650;letter-spacing:-0.01em;color:#fff;text-decoration:none;font-size:17px}
.pill{display:flex;gap:8px;align-items:center;border:1px solid rgba(58,123,255,0.25);
  background:rgba(17,22,29,0.7);padding:7px 12px;font-size:11px;letter-spacing:0.07em;
  text-transform:uppercase;color:rgba(232,233,236,0.65);text-decoration:none;border-radius:3px}
.pill b{font-weight:600;color:#fff;font-variant-numeric:tabular-nums}
.pill .dot{width:5px;height:5px;border-radius:50%;background:#3a7bff;
  box-shadow:0 0 6px rgba(58,123,255,0.7)}
main{position:relative;z-index:1;max-width:1080px;margin:0 auto;padding:64px 28px 80px}
.eyebrow{font-size:12px;letter-spacing:0.14em;text-transform:uppercase;color:#8fb6ff;margin-bottom:18px}
h1{font-size:clamp(2.4rem,6vw,4rem);line-height:1.08;font-weight:700;letter-spacing:-0.02em;
  color:#fff;max-width:16ch}
.sub{margin-top:22px;font-size:clamp(1.05rem,1.6vw,1.25rem);color:rgba(232,233,236,0.82);max-width:62ch}
.trust{margin-top:16px;font-size:0.98rem;color:rgba(232,233,236,0.6);max-width:66ch}
.cta{margin-top:36px;display:flex;gap:14px;flex-wrap:wrap}
.cta a{padding:13px 26px;font-size:14px;font-weight:600;text-decoration:none;border-radius:3px;
  transition:filter 120ms ease,border-color 120ms ease}
.cta .primary{background:#3a7bff;color:#fff;box-shadow:0 0 24px rgba(58,123,255,0.28)}
.cta .primary:hover{filter:brightness(1.12)}
.cta .secondary{border:1px solid rgba(143,182,255,0.4);color:#cfe0ff;background:rgba(17,22,29,0.6)}
.cta .secondary:hover{border-color:#4fd0ff;color:#fff}
.cta a:focus-visible{outline:2px solid #4fd0ff;outline-offset:2px}
.cards{margin-top:72px;display:grid;gap:16px;grid-template-columns:repeat(auto-fit,minmax(260px,1fr))}
.card{border:1px solid rgba(58,123,255,0.16);background:rgba(15,20,27,0.72);padding:24px;border-radius:4px}
.card h2{font-size:13px;letter-spacing:0.1em;text-transform:uppercase;color:#8fb6ff;
  margin-bottom:10px;font-weight:600}
.card p{font-size:0.96rem;color:rgba(232,233,236,0.78)}
footer{position:relative;z-index:1;max-width:1080px;margin:0 auto;padding:0 28px 48px;
  font-size:0.85rem;color:rgba(232,233,236,0.45)}
footer a{color:#8fb6ff;text-decoration:none}
footer a:hover{color:#cfe0ff}
`.trim()

const LANDER4_COUNTER_SCRIPT = `
(function(){
  var node=document.getElementById("tokens-served");
  if(!node||!window.fetch)return;
  function tick(){
    fetch("/api/public/khala-tokens-served").then(function(r){return r.json()})
      .then(function(d){
        if(d&&typeof d.tokensServed==="number"){
          var next=d.tokensServed.toLocaleString("en-US");
          if(next.length>=node.textContent.length)node.textContent=next;
        }
      }).catch(function(){})
  }
  setInterval(tick,5000);
})();
`.trim()

export const renderLander4Html = (tokensServed: number | null): string => {
  const display = tokensServed === null ? '—' : formatTokens(tokensServed)
  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="robots" content="noindex">
<title>OpenAgents — Agents that work.</title>
<link rel="icon" href="/favicon.svg" type="image/svg+xml">
<style>${LANDER4_CSS}</style>
</head>
<body>
<div class="backdrop" aria-hidden="true"></div>
<header>
<a class="brand" href="/">OpenAgents</a>
<a class="pill mono" href="/stats"><span class="dot" aria-hidden="true"></span>Tokens Served:&nbsp;<b id="tokens-served">${display}</b></a>
</header>
<main>
<p class="eyebrow mono">OpenAgents Business</p>
<h1>Agents that work.</h1>
<p class="sub">Hire agents from the OpenAgents network to get real work done — software built fast, campaigns drafted, batches processed — delivered with verifiable receipts.</p>
<p class="trust">Start with a fast quick win we can deliver in days, then put recurring work on Autopilot as trust builds. Every accepted outcome ties to evidence; every paid run is scoped with a receipt plan up front; a human-review gate sits before anything ships, sends, or spends.</p>
<div class="cta">
<a class="primary" href="/business">Talk to Sales</a>
<a class="secondary" href="/stats">See the live network</a>
</div>
<div class="cards">
<div class="card">
<h2 class="mono">Software built fast</h2>
<p>A coding agent takes a written objective, works in your repo, runs your verification command, and hands back a reviewable change with evidence.</p>
</div>
<div class="card">
<h2 class="mono">Receipts, not vibes</h2>
<p>Every accepted outcome ties to evidence; every paid run is scoped with a receipt plan up front.</p>
</div>
<div class="card">
<h2 class="mono">A human gate</h2>
<p>A human-review gate sits before anything ships, sends, or spends.</p>
</div>
</div>
</main>
<footer>Availability is grounded in our public <a href="/docs/product-promises">product-promise registry</a> — we say so in writing and scope the smallest honest version.</footer>
<script>${LANDER4_COUNTER_SCRIPT}</script>
</body>
</html>
`
}

export const handleLander4Page = (
  request: Request,
  input: Lander4RouteInput,
) => {
  if (request.method !== 'GET') {
    return Effect.succeed(methodNotAllowed(['GET']))
  }
  const ledger =
    input.ledger ?? makeD1TokenUsageLedger(input.OPENAGENTS_DB as D1Database)
  const nowMs = input.nowMs ?? currentEpochMillis
  const startedAt = nowMs()
  const respond = (tokensServed: number | null): Response => {
    const d1Ms = nowMs() - startedAt
    return new Response(renderLander4Html(tokensServed), {
      headers: {
        'content-type': 'text/html; charset=utf-8',
        'cache-control': 'no-store',
        'server-timing': `d1;dur=${d1Ms}`,
      },
      status: 200,
    })
  }
  return ledger.readPublicTokensServed().pipe(
    Effect.map(aggregate => respond(aggregate.tokensServed)),
    Effect.catch(() => Effect.succeed(respond(null))),
  )
}
