// Shared shell for the server-rendered lander family (/lander4,
// /business-new): one inline stylesheet, one header/nav, one footer, one
// counter refresher — pure string builders, no HTTP concerns.
//
// Design system (impeccable pass, 2026-07-02): the house face carries the
// voice. Headlines are Berkeley Mono Bold — the same licensed webfont the SPA
// ships at /fonts/ — not a generic system sans; the terminal period of
// "Agents that work." is set in energy blue as the one brand mark. Sections
// are registers (bordered rows with mono refs and status chips, echoing the
// public promise registry), never identical card grids. StarCraft-Protoss
// stays subtle: soft slate near-blacks, a single energized hairline under the
// header, restrained #3a7bff accents.

export type LanderNavKey = 'home' | 'business'

const NAV_ITEMS: ReadonlyArray<{
  readonly key: LanderNavKey | 'stats' | 'docs'
  readonly href: string
  readonly label: string
}> = [
  { href: '/business-new', key: 'business', label: 'Business' },
  { href: '/stats', key: 'stats', label: 'Network' },
  { href: '/docs', key: 'docs', label: 'Docs' },
]

export const LANDER_SHELL_CSS = `
*{box-sizing:border-box;margin:0;padding:0}
html{height:100%}
@font-face{font-family:"Berkeley Mono";font-style:normal;font-weight:400;font-display:swap;
  src:url("/fonts/BerkeleyMono-Regular.woff2") format("woff2")}
@font-face{font-family:"Berkeley Mono";font-style:normal;font-weight:700;font-display:swap;
  src:url("/fonts/BerkeleyMono-Bold.woff2") format("woff2")}
:root{
  --bg:#0a0e14;--panel:#0f141b;--panel-2:#11161d;
  --ink:#e6e9ee;--ink-dim:#b9bfc9;--ink-faint:#8b93a1;
  --blue:#3a7bff;--cyan:#4fd0ff;--ink-blue:#8fb6ff;
  --line:rgba(58,123,255,0.16);--line-strong:rgba(58,123,255,0.34);
  --mono:"Berkeley Mono",ui-monospace,SFMono-Regular,Menlo,monospace;
  --sans:ui-sans-serif,-apple-system,"SF Pro Text",system-ui,sans-serif}
body{min-height:100%;background:var(--bg);color:var(--ink);font-family:var(--sans);
  -webkit-font-smoothing:antialiased;line-height:1.6}
.mono{font-family:var(--mono)}
.backdrop{position:fixed;inset:0;pointer-events:none;z-index:0;
  background-image:
    radial-gradient(ellipse 70% 46% at 50% 0%,rgba(58,123,255,0.07),transparent 62%),
    repeating-linear-gradient(0deg,rgba(58,123,255,0.02) 0 1px,transparent 1px 34px),
    repeating-linear-gradient(90deg,rgba(58,123,255,0.02) 0 1px,transparent 1px 34px)}
.shell{position:relative;z-index:1;max-width:1060px;margin:0 auto;padding:0 clamp(20px,4vw,32px)}
header.site{display:flex;justify-content:space-between;align-items:center;gap:18px;
  padding-block:18px 16px}
.brand{font-family:var(--mono);font-weight:700;font-size:16px;letter-spacing:-0.01em;
  color:#fff;text-decoration:none}
.brand em{font-style:normal;color:var(--blue)}
nav.site{display:flex;gap:4px;align-items:center}
nav.site a{font-family:var(--mono);font-size:12.5px;color:var(--ink-dim);text-decoration:none;
  padding:7px 11px;border-radius:2px;letter-spacing:0.02em}
nav.site a:hover{color:#fff;background:rgba(58,123,255,0.08)}
nav.site a[aria-current="page"]{color:var(--cyan)}
nav.site a:focus-visible,.cta a:focus-visible,a.pill:focus-visible{outline:2px solid var(--cyan);outline-offset:2px}
.pill{display:flex;gap:8px;align-items:center;border:1px solid var(--line-strong);
  background:rgba(15,20,27,0.8);padding:6px 11px;font-family:var(--mono);font-size:11px;
  letter-spacing:0.05em;color:var(--ink-dim);text-decoration:none;border-radius:2px;white-space:nowrap}
.pill b{font-weight:700;color:#fff;font-variant-numeric:tabular-nums}
.pill .dot{width:5px;height:5px;border-radius:50%;background:var(--blue);
  box-shadow:0 0 6px rgba(58,123,255,0.8);flex:none}
.rule{position:relative;z-index:1;height:1px;border:0;background:linear-gradient(90deg,transparent,var(--line-strong) 18%,var(--line-strong) 82%,transparent);
  box-shadow:0 0 12px rgba(58,123,255,0.25);margin:0}
main{padding:clamp(52px,9vh,96px) 0 clamp(48px,8vh,80px)}
h1{font-family:var(--mono);font-weight:700;font-size:clamp(2.1rem,5.4vw,3.9rem);
  line-height:1.12;letter-spacing:-0.025em;color:#fff;max-width:18ch;text-wrap:balance}
h1 .mark{color:var(--blue)}
.sub{margin-top:24px;font-size:clamp(1.02rem,1.5vw,1.2rem);color:var(--ink);max-width:60ch;text-wrap:pretty}
.trust{margin-top:14px;font-size:0.95rem;color:var(--ink-dim);max-width:64ch;text-wrap:pretty}
.cta{margin-top:38px;display:flex;gap:12px;flex-wrap:wrap}
.cta a{font-family:var(--mono);font-size:13.5px;font-weight:700;text-decoration:none;
  padding:13px 24px;border-radius:2px;letter-spacing:0.01em;
  transition:filter 140ms cubic-bezier(0.22,1,0.36,1),border-color 140ms cubic-bezier(0.22,1,0.36,1)}
.cta .primary{background:var(--blue);color:#fff;box-shadow:0 0 22px rgba(58,123,255,0.3)}
.cta .primary:hover{filter:brightness(1.14)}
.cta .secondary{border:1px solid var(--line-strong);color:var(--ink-blue);background:rgba(15,20,27,0.6)}
.cta .secondary:hover{border-color:var(--cyan);color:#fff}
section.register{margin-top:clamp(64px,10vh,104px)}
section.register>h2{font-family:var(--mono);font-size:13px;font-weight:700;letter-spacing:0.04em;
  color:var(--ink-blue);margin-bottom:14px}
.reg{border:1px solid var(--line);background:rgba(15,20,27,0.55)}
.reg .row{display:grid;grid-template-columns:minmax(170px,215px) 1fr auto;gap:18px;
  align-items:baseline;padding:18px 20px}
.reg .row+.row{border-top:1px solid var(--line)}
.reg .ref{font-family:var(--mono);font-size:11.5px;color:var(--cyan);overflow-wrap:anywhere;line-height:1.5}
.reg .what{min-width:0}
.reg .what strong{display:block;color:#fff;font-weight:600;font-size:0.98rem;margin-bottom:3px}
.reg .what p{font-size:0.92rem;color:var(--ink-dim);max-width:58ch}
.reg .chip{font-family:var(--mono);font-size:10.5px;letter-spacing:0.06em;padding:4px 8px;
  border-radius:2px;white-space:nowrap;border:1px solid}
.chip.live{color:#7ef0b2;border-color:rgba(126,240,178,0.35);background:rgba(126,240,178,0.06)}
.chip.assisted{color:var(--ink-blue);border-color:var(--line-strong);background:rgba(58,123,255,0.07)}
@media (max-width:640px){.reg .row{grid-template-columns:1fr;gap:8px}.reg .chip{justify-self:start}}
form.intake{border:1px solid var(--line);background:rgba(15,20,27,0.55);padding:clamp(20px,3vw,30px);
  display:grid;gap:16px;grid-template-columns:repeat(auto-fit,minmax(240px,1fr));max-width:820px}
form.intake .wide{grid-column:1/-1}
form.intake label{display:block}
form.intake .lab{font-family:var(--mono);font-size:11px;letter-spacing:0.05em;color:var(--ink-blue);
  margin-bottom:6px;display:block}
form.intake input[type="text"],form.intake input[type="email"],form.intake input[type="tel"],
form.intake input[type="url"],form.intake textarea{
  width:100%;background:#0b1017;border:1px solid var(--line-strong);border-radius:2px;
  color:var(--ink);font-family:var(--mono);font-size:13.5px;padding:11px 12px;line-height:1.45}
form.intake textarea{min-height:110px;resize:vertical}
form.intake input:focus-visible,form.intake textarea:focus-visible{
  outline:2px solid var(--cyan);outline-offset:1px;border-color:var(--cyan)}
form.intake input::placeholder,form.intake textarea::placeholder{color:var(--ink-faint)}
form.intake .check{display:flex;gap:9px;align-items:center;font-size:0.9rem;color:var(--ink-dim)}
form.intake .check input{accent-color:var(--blue);width:15px;height:15px}
form.intake button{font-family:var(--mono);font-size:13.5px;font-weight:700;color:#fff;
  background:var(--blue);border:0;border-radius:2px;padding:13px 26px;cursor:pointer;
  box-shadow:0 0 22px rgba(58,123,255,0.3);justify-self:start;
  transition:filter 140ms cubic-bezier(0.22,1,0.36,1)}
form.intake button:hover{filter:brightness(1.14)}
form.intake button:focus-visible{outline:2px solid var(--cyan);outline-offset:2px}
form.intake button[disabled]{filter:saturate(0.4) brightness(0.8);cursor:default}
.intake-note{font-size:0.88rem;color:var(--ink-faint);margin-top:10px;max-width:64ch}
.intake-result{font-family:var(--mono);font-size:13px;padding:12px 14px;border-radius:2px;display:none}
.intake-result.ok{display:block;color:#7ef0b2;border:1px solid rgba(126,240,178,0.35);background:rgba(126,240,178,0.06)}
.intake-result.err{display:block;color:#ffb3ad;border:1px solid rgba(255,140,130,0.4);background:rgba(255,140,130,0.07)}
footer.site{padding:26px 0 44px;font-size:0.85rem;color:var(--ink-faint)}
footer.site a{color:var(--ink-blue);text-decoration:none}
footer.site a:hover{color:var(--cyan)}
.rise{opacity:0;transform:translateY(10px);
  animation:oaRise 560ms cubic-bezier(0.22,1,0.36,1) forwards}
.rise.d1{animation-delay:70ms}.rise.d2{animation-delay:140ms}.rise.d3{animation-delay:210ms}
@keyframes oaRise{to{opacity:1;transform:none}}
@media (prefers-reduced-motion:reduce){
  .rise{animation:none;opacity:1;transform:none}
  .dot{animation:none}}
@media (prefers-reduced-motion:no-preference){
  .dot{animation:oaPulse 2.4s ease-in-out infinite}
  @keyframes oaPulse{0%,100%{opacity:1}50%{opacity:0.45}}}
`.trim()

export const LANDER_HEAD_FONT_PRELOAD = `<link rel="preload" href="/fonts/BerkeleyMono-Bold.woff2" as="font" type="font/woff2" crossorigin>
<link rel="preload" href="/fonts/BerkeleyMono-Regular.woff2" as="font" type="font/woff2" crossorigin>`

export const renderLanderHeader = (
  active: LanderNavKey,
  tokensDisplay: string,
): string => {
  const links = NAV_ITEMS.map(item => {
    const current = item.key === active ? ' aria-current="page"' : ''
    return `<a href="${item.href}"${current}>${item.label}</a>`
  }).join('\n')
  return `<header class="site shell">
<a class="brand" href="/lander4">OpenAgents<em>.</em></a>
<nav class="site" aria-label="Primary">
${links}
</nav>
<a class="pill" href="/stats"><span class="dot" aria-hidden="true"></span>Tokens Served:&nbsp;<b id="tokens-served">${tokensDisplay}</b></a>
</header>
<hr class="rule">`
}

export const renderLanderFooter = (): string =>
  `<footer class="site shell">Availability is grounded in our public <a href="/docs/product-promises">product-promise registry</a> — we say so in writing and scope the smallest honest version.</footer>`

// Progressive enhancement only: keeps the SSR total live via the existing
// public scalar endpoint.
export const LANDER_COUNTER_SCRIPT = `
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

export const formatLanderTokens = (tokensServed: number | null): string =>
  tokensServed === null ? '—' : tokensServed.toLocaleString('en-US')
