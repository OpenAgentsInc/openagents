import { Effect } from 'effect'

import { methodNotAllowed } from './http/responses'
import {
  type TokenUsageLedgerShape,
  makeD1TokenUsageLedger,
} from './token-usage-ledger'

// `/lander2` — the server-rendered landing-page experiment from the site-speed
// lane (docs/fable/2026-07-02-site-speed-lane-spec.md, phase P5 candidate
// "static/inline first-paint hero"). The production landing page is a ~2.7 KB
// SPA shell whose first paint waits on a 1.07 MB (brotli) bundle; this route
// renders the same fold — tokens-served pill, wordmark, the two existing CTAs
// — as ONE HTML document with inline critical CSS and the token total read
// from the ledger at render time. A tiny inline script keeps the number fresh
// via the existing public scalar endpoint; with JS disabled the page is still
// complete and correct.
//
// Copy discipline: every visible string reuses existing landing/home copy
// verbatim ("Tokens Served:", "OpenAgents", "WHAT IS KHALA?", "JOIN THE
// TASSADAR TRAINING RUN"). This is an unlisted measurement surface, not a new
// claim surface.

type Lander2RouteInput = Readonly<{
  OPENAGENTS_DB?: D1Database
  ledger?: TokenUsageLedgerShape
}>

const formatTokens = (total: number): string => total.toLocaleString('en-US')

// Inline critical CSS on the khala palette (energy blue #3a7bff, cyan
// #4fd0ff, tinted near-blacks per root DESIGN.md). The glow-square backdrop is
// a pure-CSS approximation of the landing squares scene — two layered
// repeating gradients, zero JS, zero WebGL.
const LANDER2_CSS = `
*{box-sizing:border-box;margin:0;padding:0}
html,body{height:100%}
body{
  background:#05070b;color:#f1efe8;
  font-family:"Berkeley Mono",ui-monospace,SFMono-Regular,Menlo,monospace;
  -webkit-font-smoothing:antialiased;overflow:hidden}
.backdrop{position:fixed;inset:0;
  background-image:
    radial-gradient(ellipse 80% 60% at 50% 42%,rgba(58,123,255,0.14),transparent 60%),
    repeating-linear-gradient(0deg,rgba(58,123,255,0.05) 0 2px,transparent 2px 26px),
    repeating-linear-gradient(90deg,rgba(58,123,255,0.05) 0 2px,transparent 2px 26px);
}
.backdrop::after{content:"";position:absolute;inset:0;
  background:radial-gradient(ellipse 60% 45% at 50% 45%,transparent 30%,rgba(5,7,11,0.85) 100%)}
.pill{position:fixed;top:16px;left:16px;z-index:2;display:flex;gap:8px;align-items:baseline;
  border:1px solid rgba(58,123,255,0.35);background:rgba(12,15,19,0.85);
  padding:8px 14px;font-size:12px;letter-spacing:0.08em;text-transform:uppercase;
  color:rgba(241,239,232,0.75);text-decoration:none}
.pill b{font-weight:600;color:#fff;font-variant-numeric:tabular-nums}
.pill .dot{width:6px;height:6px;border-radius:50%;background:#3a7bff;
  box-shadow:0 0 8px rgba(58,123,255,0.85);align-self:center}
main{position:relative;z-index:1;display:flex;flex-direction:column;gap:40px;
  align-items:center;justify-content:center;height:100dvh;text-align:center;padding:24px}
h1{font-size:clamp(2.6rem,9vw,6rem);font-weight:600;letter-spacing:-0.02em;color:#fff;
  text-shadow:0 0 42px rgba(58,123,255,0.35)}
nav{display:flex;flex-wrap:wrap;gap:16px;justify-content:center}
nav a{border:1px solid rgba(58,123,255,0.5);color:#cfe0ff;background:rgba(12,15,19,0.7);
  padding:12px 22px;font-size:13px;letter-spacing:0.1em;text-transform:uppercase;
  text-decoration:none;transition:border-color 120ms ease,color 120ms ease}
nav a:hover{border-color:#4fd0ff;color:#fff}
nav a:focus-visible{outline:2px solid #4fd0ff;outline-offset:2px}
@media (prefers-reduced-motion:no-preference){
  .dot{animation:oaPulse 2.4s ease-in-out infinite}
  @keyframes oaPulse{0%,100%{opacity:1}50%{opacity:0.45}}
}
`.trim()

// Progressive enhancement only: refresh the number from the existing public
// scalar endpoint. The SSR value is already correct; this keeps it live. No
// bundle, no framework, ~0.4 KB.
const LANDER2_SCRIPT = `
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

export const renderLander2Html = (tokensServed: number | null): string => {
  // Ledger failure renders the same em-dash placeholder the SPA uses; the
  // inline refresher fills it as soon as the scalar endpoint answers.
  const display = tokensServed === null ? '\u2014' : formatTokens(tokensServed)
  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="robots" content="noindex">
<title>OpenAgents</title>
<link rel="icon" href="/favicon.svg" type="image/svg+xml">
<style>${LANDER2_CSS}</style>
</head>
<body>
<div class="backdrop" aria-hidden="true"></div>
<a class="pill" href="/stats"><span class="dot" aria-hidden="true"></span>Tokens Served:&nbsp;<b id="tokens-served">${display}</b></a>
<main>
<h1>OpenAgents</h1>
<nav>
<a href="/khala">WHAT IS KHALA?</a>
<a href="/tassadar">JOIN THE TASSADAR TRAINING RUN</a>
</nav>
</main>
<script>${LANDER2_SCRIPT}</script>
</body>
</html>
`
}

export const handleLander2Page = (
  request: Request,
  input: Lander2RouteInput,
) => {
  if (request.method !== 'GET') {
    return Effect.succeed(methodNotAllowed(['GET']))
  }
  const ledger =
    input.ledger ?? makeD1TokenUsageLedger(input.OPENAGENTS_DB as D1Database)
  const startedAt = Date.now()
  const respond = (tokensServed: number | null): Response => {
    const d1Ms = Date.now() - startedAt
    return new Response(renderLander2Html(tokensServed), {
      headers: {
        'content-type': 'text/html; charset=utf-8',
        // Deliberately uncached in v1 so the lab harness measures true
        // worker-render + D1 cost; edge caching is a later, measured step.
        'cache-control': 'no-store',
        'server-timing': `d1;dur=${d1Ms}`,
      },
      status: 200,
    })
  }
  return ledger.readPublicTokensServed().pipe(
    Effect.map(aggregate => respond(aggregate.tokensServed)),
    // The document registry requires an error-free Effect; a ledger failure
    // degrades to the placeholder page rather than a 500.
    Effect.catch(() => Effect.succeed(respond(null))),
  )
}
