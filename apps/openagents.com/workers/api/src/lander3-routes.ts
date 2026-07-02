import { Effect } from 'effect'

import { methodNotAllowed } from './http/responses'
import { currentEpochMillis } from './runtime-primitives'
import {
  type TokenUsageLedgerShape,
  makeD1TokenUsageLedger,
} from './token-usage-ledger'

// `/lander3` — lander2's server-rendered architecture (instant paint: inline
// CSS grid backdrop, SSR token total, no main bundle) PLUS the real Three.js
// landing-squares hero, loaded lazily: after `load` + idle the page
// dynamically imports the self-contained `/assets/lander3-scene.js` module
// (built by apps/web/vite.lander3.config.ts), mounts the same scene the SPA
// landing page uses, and fades the canvas in over the CSS grid once its first
// frames have rendered. The expensive part never touches the paint path:
// reduced-motion and Save-Data users simply keep the grid.
//
// Copy discipline: identical strings to /lander2 (existing landing/home copy
// verbatim). Unlisted, noindex — a site-speed-lane measurement surface.

type Lander3RouteInput = Readonly<{
  OPENAGENTS_DB?: D1Database
  ledger?: TokenUsageLedgerShape
  nowMs?: () => number
}>

const formatTokens = (total: number): string => total.toLocaleString('en-US')

const LANDER3_CSS = `
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
#scene{position:fixed;inset:0;opacity:0;transition:opacity 900ms ease}
#scene.ready{opacity:1}
#scene-mount{position:absolute;inset:0}
.pill{position:fixed;top:16px;left:16px;z-index:2;display:flex;gap:8px;align-items:baseline;
  border:1px solid rgba(58,123,255,0.35);background:rgba(12,15,19,0.85);
  padding-block:8px;padding-inline:14px;font-size:12px;letter-spacing:0.08em;text-transform:uppercase;
  color:rgba(241,239,232,0.75);text-decoration:none}
.pill b{font-weight:600;color:#fff;font-variant-numeric:tabular-nums}
.pill .dot{width:6px;height:6px;border-radius:50%;background:#3a7bff;
  box-shadow:0 0 8px rgba(58,123,255,0.85);align-self:center}
main{position:relative;z-index:1;display:flex;flex-direction:column;gap:40px;
  align-items:center;justify-content:center;height:100dvh;text-align:center;padding-block:24px;padding-inline:24px}
h1{font-size:clamp(2.6rem,9vw,6rem);font-weight:600;letter-spacing:-0.02em;color:#fff;
  text-shadow:0 0 42px rgba(58,123,255,0.35)}
nav{display:flex;flex-wrap:wrap;gap:16px;justify-content:center}
nav a{border:1px solid rgba(58,123,255,0.5);color:#cfe0ff;background:rgba(12,15,19,0.7);
  padding-block:12px;padding-inline:22px;font-size:13px;letter-spacing:0.1em;text-transform:uppercase;
  text-decoration:none;transition:border-color 120ms ease,color 120ms ease}
nav a:hover{border-color:#4fd0ff;color:#fff}
nav a:focus-visible{outline:2px solid #4fd0ff;outline-offset:2px}
@media (prefers-reduced-motion:no-preference){
  .dot{animation:oaPulse 2.4s ease-in-out infinite}
  @keyframes oaPulse{0%,100%{opacity:1}50%{opacity:0.45}}
}
`.trim()

// Counter refresher — identical to /lander2's.
const LANDER3_COUNTER_SCRIPT = `
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

// Lazy hero loader: never before `load`, never on reduced-motion or
// Save-Data, always fail-soft back to the CSS grid. The scene module resolves
// only after its first frames rendered, so `.ready` never fades in a blank
// canvas.
const LANDER3_SCENE_SCRIPT = `
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

export const renderLander3Html = (tokensServed: number | null): string => {
  const display = tokensServed === null ? '—' : formatTokens(tokensServed)
  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="robots" content="noindex">
<title>OpenAgents</title>
<link rel="icon" href="/favicon.svg" type="image/svg+xml">
<style>${LANDER3_CSS}</style>
</head>
<body>
<div class="backdrop" aria-hidden="true"></div>
<div id="scene" aria-hidden="true"><div id="scene-mount"></div></div>
<a class="pill" href="/stats"><span class="dot" aria-hidden="true"></span>Tokens Served:&nbsp;<b id="tokens-served">${display}</b></a>
<main>
<h1>OpenAgents</h1>
<nav>
<a href="/khala">WHAT IS KHALA?</a>
<a href="/tassadar">JOIN THE TASSADAR TRAINING RUN</a>
</nav>
</main>
<script>${LANDER3_COUNTER_SCRIPT}</script>
<script>${LANDER3_SCENE_SCRIPT}</script>
</body>
</html>
`
}

export const handleLander3Page = (
  request: Request,
  input: Lander3RouteInput,
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
    return new Response(renderLander3Html(tokensServed), {
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
