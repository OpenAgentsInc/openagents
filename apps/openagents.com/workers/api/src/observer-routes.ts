import { Effect } from 'effect'

import { methodNotAllowed } from './http/responses'
import {
  LANDER_HEAD_FONT_PRELOAD,
  LANDER_SHELL_CSS,
  renderLanderFooter,
} from './lander-shell'

// `/observer` — the Observer product landing page, in the server-rendered
// lander-family system (lander-shell.ts): shared Berkeley Mono / Protoss-blue
// shell CSS, register rows with mono refs and status chips (echoing the
// public promise registry), no client bundle, no D1 read.
//
// Content source of truth: docs/assurance/ (OBSERVER_PRODUCT_PLAN.md,
// README.md, ASSURANCE_SPEC.md, PRODUCTSPEC_EVIDENCE_LOOP.md). Copy
// discipline: implemented-vs-planned state mirrors the docs/assurance
// status table verbatim in meaning — planned surfaces stay visibly unbuilt,
// and nothing on the page is a public product promise (the promise registry
// remains the only claim authority). The subject-binding specimen quotes the
// real first dogfood target from docs/assurance/README.md.
//
// CSS discipline: page-specific styles use logical longhands only
// (padding-block/padding-inline), enforced by lander-css-policy.test.ts.

const OBSERVER_PAGE_CSS = `
.sub strong{color:#fff;font-weight:400}
.chain{border:1px solid var(--line);background:rgba(15,20,27,0.55)}
.chain .stage{position:relative;display:grid;grid-template-columns:minmax(180px,230px) 1fr;gap:18px;
  padding-block:18px;padding-inline:20px 20px;align-items:baseline}
.chain .stage+.stage{border-top:1px solid var(--line)}
.chain .stage .name{font-family:var(--mono);font-size:13px;font-weight:700;color:#fff}
.chain .stage .name .owns{display:block;font-weight:400;font-size:11px;color:var(--ink-faint);
  letter-spacing:0.04em;margin-top:3px}
.chain .stage .role{font-size:0.92rem;color:var(--ink-dim);max-width:60ch}
.chain .stage .role strong{color:var(--ink);font-weight:400}
.chain .stage .idx{position:absolute;top:18px;right:20px;font-family:var(--mono);
  font-size:10.5px;color:var(--ink-faint);letter-spacing:0.08em}
.verdicts{display:flex;gap:8px;flex-wrap:wrap;margin-top:10px}
.verdicts span{font-family:var(--mono);font-size:10.5px;letter-spacing:0.06em;
  padding-block:4px;padding-inline:8px;border:1px solid;border-radius:2px}
.verdicts .v-c{color:#7ef0b2;border-color:rgba(126,240,178,0.35);background:rgba(126,240,178,0.06)}
.verdicts .v-r{color:#ffb3ad;border-color:rgba(255,140,130,0.4);background:rgba(255,140,130,0.07)}
.verdicts .v-i{color:#ffd98a;border-color:rgba(255,211,120,0.4);background:rgba(255,211,120,0.07)}
.chip.bad{color:#ffb3ad;border-color:rgba(255,140,130,0.4);background:rgba(255,140,130,0.07)}
.chip.law{color:var(--cyan);border-color:rgba(79,208,255,0.35);background:rgba(79,208,255,0.06)}
.chip.design{color:#ffd98a;border-color:rgba(255,211,120,0.4);background:rgba(255,211,120,0.07)}
.chip.planned{color:var(--ink-faint);border-color:var(--line);background:transparent}
.verdict-line{margin-top:22px;font-size:clamp(1rem,1.6vw,1.15rem);color:var(--ink);max-width:66ch;text-wrap:pretty}
.verdict-line em{color:var(--cyan);font-style:normal}
.specimen{border:1px solid var(--line-strong);background:#0b1017}
.specimen .cap{padding-block:9px;padding-inline:18px;border-bottom:1px solid var(--line);
  font-family:var(--mono);font-size:11px;letter-spacing:0.04em;color:var(--ink-faint)}
.specimen .cap b{color:var(--cyan);font-weight:400}
.specimen pre{margin-block:0;margin-inline:0;padding-block:16px;padding-inline:18px;overflow-x:auto;
  font-family:var(--mono);font-size:12.5px;line-height:1.75;color:var(--ink-dim)}
.specimen pre .f{color:var(--ink-faint)}
.specimen pre .val{color:var(--ink)}
.specimen pre .hl{color:var(--cyan)}
.reg-note{font-size:0.9rem;color:var(--ink-dim);max-width:64ch;margin-top:14px;text-wrap:pretty}
.reg-note a{color:var(--ink-blue);text-decoration:none}
.reg-note a:hover{color:var(--cyan)}
.tracks{display:grid;grid-template-columns:3fr 2fr;gap:20px;align-items:start}
.track{border:1px solid var(--line);background:rgba(15,20,27,0.55);
  padding-block:22px 24px;padding-inline:24px}
.track.first{border-color:var(--line-strong);background:rgba(58,123,255,0.05)}
.track .when{font-family:var(--mono);font-size:11px;letter-spacing:0.06em;color:var(--cyan)}
.track.later .when{color:var(--ink-faint)}
.track h3{font-family:var(--mono);font-size:1.05rem;color:#fff;margin-top:8px}
.track ul{margin-top:12px;padding-inline:1.2em 0;color:var(--ink-dim);font-size:0.92rem}
.track li{margin-top:6px}
.track .note{margin-top:16px;font-size:0.85rem;color:var(--ink-faint);text-wrap:pretty}
@media (max-width:720px){
  .chain .stage{grid-template-columns:1fr;gap:8px}
  .chain .stage .idx{position:static;order:-1}
  .tracks{grid-template-columns:1fr}}
`.trim()

export const renderObserverHtml = (): string => `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Observer — OpenAgents</title>
<meta name="description" content="Observer turns a product spec into a reviewed proof design, compiles it into an immutable verification manifest, and runs it through real QA tools. Built on AssuranceSpec. In development at OpenAgents, dogfooded on our own systems in the open.">
${LANDER_HEAD_FONT_PRELOAD}
<style>${LANDER_SHELL_CSS}
${OBSERVER_PAGE_CSS}</style>
</head>
<body>
<div class="backdrop" aria-hidden="true"></div>
<div class="masthead">
<header class="site shell">
<a class="brand" href="/">OpenAgents<em>.</em></a>
<nav class="site" aria-label="Primary">
<a href="/observer" aria-current="page">Observer</a>
<a href="/business">Business</a>
<a href="/forum">Forum</a>
<a href="/docs/product-promises">Promises</a>
</nav>
<a class="pill" href="https://github.com/OpenAgentsInc/openagents/tree/main/docs/assurance" rel="noopener"><span class="dot" aria-hidden="true"></span>AssuranceSpec&nbsp;<b>open design</b></a>
</header>
<hr class="rule">
</div>
<main class="shell">
<h1 class="rise">Observer<span class="mark">.</span></h1>
<p class="sub rise d1">Proof-design for software built by agents. Observer turns an accepted product spec into a <strong>reviewed proof design</strong>, compiles it into an <strong>immutable verification manifest</strong>, and runs it through real QA tools — so when the checks go green, the green <strong>means what it claims</strong>.</p>
<p class="trust rise d2">Built on <strong>AssuranceSpec</strong>, a framework-neutral standard for committed verification intent. In development at OpenAgents — designed in the open, dogfooded on our own desktop app before anyone else's code.</p>
<div class="cta rise d3">
<a class="primary" href="https://github.com/OpenAgentsInc/openagents/blob/main/docs/assurance/ASSURANCE_SPEC.md" rel="noopener">Read the AssuranceSpec design</a>
<a class="secondary" href="https://github.com/OpenAgentsInc/openagents/blob/main/docs/assurance/OBSERVER_PRODUCT_PLAN.md" rel="noopener">Observer product plan</a>
</div>

<section class="register" id="problem">
<h2>THE FAILURE MODE — A PASSING SUITE IS NOT PROOF</h2>
<div class="reg">
<div class="row">
<span class="ref">false_green.fixture_assert</span>
<div class="what"><strong>The test asserts the fixture</strong>
<p>It proves the mock behaves like the mock — not that the product behaves like the spec.</p></div>
<span class="chip bad">FALSE GREEN</span>
</div>
<div class="row">
<span class="ref">false_green.impl_mirror</span>
<div class="what"><strong>The test mirrors the implementation</strong>
<p>Written after the code, it inherits the code's assumptions — including the wrong ones.</p></div>
<span class="chip bad">FALSE GREEN</span>
</div>
<div class="row">
<span class="ref">false_green.mocked_seam</span>
<div class="what"><strong>The real seam is never exercised</strong>
<p>Both sides of a client–server or renderer–host boundary are faked, so integration defects pass clean.</p></div>
<span class="chip bad">FALSE GREEN</span>
</div>
<div class="row">
<span class="ref">false_green.coverage_theater</span>
<div class="what"><strong>Coverage stands in for behavior</strong>
<p>Line coverage measures execution, not correctness — activity dressed up as evidence.</p></div>
<span class="chip bad">FALSE GREEN</span>
</div>
<div class="row">
<span class="ref">false_green.round_up</span>
<div class="what"><strong>Everything rounds up to green</strong>
<p>Skipped, stale, flaky, and inconclusive results quietly disappear into a passing summary.</p></div>
<span class="chip bad">FALSE GREEN</span>
</div>
</div>
<p class="verdict-line">Teams cannot reliably answer, criterion by criterion, whether the exact release artifact behaves as designed. Evidence links help you find the test — they cannot tell you it was the right test. <em>A link is not a verdict.</em></p>
</section>

<section class="register" id="pipeline">
<h2>HOW IT WORKS — SPECS BOUND TO REAL SYSTEM BYTES</h2>
<div class="chain">
<div class="stage">
<span class="name">Product Spec<span class="owns">commits intent</span></span>
<span class="role">The durable what/why, with stable acceptance-criterion IDs. It indexes evidence; it never grades it.</span>
<span class="idx">01</span>
</div>
<div class="stage">
<span class="name">Assurance Spec<span class="owns">commits proof design</span></span>
<span class="role">A separately reviewed companion mapping every criterion to <strong>risk-appropriate oracles, real seams, environment tiers, and falsifiers</strong> — admitted before the implementation can bias what the tests say.</span>
<span class="idx">02</span>
</div>
<div class="stage">
<span class="name">Observer<span class="owns">plans, reviews, compiles</span></span>
<span class="role">Typed semantic planning proposes obligations; human review admits them; deterministic compilation makes no clock, network, or model calls — <strong>identical inputs yield byte-identical output</strong>.</span>
<span class="idx">03</span>
</div>
<div class="stage">
<span class="name">Assurance Manifest<span class="owns">immutable verification graph</span></span>
<span class="role">The compiled lockfile of resolved verification units and dependency gates, <strong>bound to exact bytes</strong>: spec revisions, document digests, intent digests, environment profiles, adapter locks. No mutable status lives inside it.</span>
<span class="idx">04</span>
</div>
<div class="stage">
<span class="name">Execution<span class="owns">native tools + QA Swarm</span></span>
<span class="role">Your existing test frameworks, browsers, devices, property suites, and formal checkers run the manifest — one universal runner is explicitly a non-goal. Swarm agents explore beyond it and distill findings into deterministic regressions.</span>
<span class="idx">05</span>
</div>
<div class="stage">
<span class="name">Assurance Receipts<span class="owns">exact observed evidence</span></span>
<span class="role">Every run records provenance — spec, source, command, target, seed, adapter, and artifact digests — with a typed verdict. Receipts feed the evidence index by reference; they never revise the spec.
<span class="verdicts"><span class="v-c">CONFIRMED</span><span class="v-r">REFUTED</span><span class="v-i">INCONCLUSIVE</span></span>
</span>
<span class="idx">06</span>
</div>
</div>
</section>

<section class="register" id="laws">
<h2>WHAT KEEPS THE PROOF HONEST</h2>
<div class="reg">
<div class="row">
<span class="ref">oracle.falsifier_required</span>
<div class="what"><strong>Every required oracle names a falsifier</strong>
<p>The intended candidate must pass and a known-bad candidate must be refuted. An oracle that accepts both is ruled unsound.</p></div>
<span class="chip law">LAW</span>
</div>
<div class="row">
<span class="ref">seam.real_both_sides</span>
<div class="what"><strong>Seams are exercised for real</strong>
<p>A declared cross-process or client–server boundary drives both real sides, or cites a qualifying end-to-end receipt. Mock-only rows do not satisfy it.</p></div>
<span class="chip law">LAW</span>
</div>
<div class="row">
<span class="ref">gate.planned_red_is_honest</span>
<div class="what"><strong>Planned red is honest red</strong>
<p>Checks designed before the code can be red without blocking trunk. Once activated, required negative or unknown states block their gate.</p></div>
<span class="chip law">LAW</span>
</div>
<div class="row">
<span class="ref">status.nothing_rounds_up</span>
<div class="what"><strong>Nothing rounds up to green</strong>
<p>Skipped, stale, flaky, unavailable, and inconclusive results stay visible as exactly what they are.</p></div>
<span class="chip law">LAW</span>
</div>
<div class="row">
<span class="ref">coverage.advisory_only</span>
<div class="what"><strong>Coverage is advisory</strong>
<p>Criterion traceability, execution evidence, and explored frontier are separate ledgers. Line coverage never stands in for behavior.</p></div>
<span class="chip law">LAW</span>
</div>
<div class="row">
<span class="ref">authority.contained</span>
<div class="what"><strong>Authority is contained</strong>
<p>No green manifest or run grants merge, deploy, spend, or public-claim authority. Release decisions stay separately owned.</p></div>
<span class="chip law">LAW</span>
</div>
</div>
</section>

<section class="register" id="dogfood">
<h2>DOGFOOD — OUR OWN SYSTEMS FIRST, IN THE OPEN</h2>
<div class="specimen">
<div class="cap">subject binding — first dogfood target · <b>docs/assurance</b>, public repository</div>
<pre><span class="f">product_spec:</span> <span class="val">docs/mvp/openagents-codex-workroom-mvp.product-spec.md</span>
<span class="f">format:</span>       <span class="val">ProductSpec 0.1</span>
<span class="f">revision:</span>     <span class="val">6</span>
<span class="f">sha256:</span>       <span class="hl">fba7963334eb736582003e7d903d0e57164e7fecb2c158c302af7fb23e3f6ef1</span>
<span class="f">criteria:</span>     <span class="val">CW-AC-01 … CW-AC-18</span>
<span class="f">proposal:</span>     <span class="val">18/18 obligations generated — all still marked needs_design</span></pre>
</div>
<p class="reg-note">That binding is real, quoted from the repository — Observer's first subject is the OpenAgents Desktop MVP spec itself. The design documents, format tooling, and generated first proposal are public in <a href="https://github.com/OpenAgentsInc/openagents/tree/main/docs/assurance" rel="noopener">docs/assurance</a>.</p>
</section>

<section class="register" id="status">
<h2>STATUS — REPORTED THE WAY OBSERVER WOULD REPORT IT</h2>
<div class="reg">
<div class="row">
<span class="ref">packages/assurance-spec</span>
<div class="what"><strong>AssuranceSpec format tooling</strong>
<p>Bounded proposal profile — parser, serializer, structural validator, and CLI — implemented and tested in the open repository.</p></div>
<span class="chip live">REAL TODAY</span>
</div>
<div class="row">
<span class="ref">docs/mvp · assurance-spec.md</span>
<div class="what"><strong>First Assurance Spec proposal</strong>
<p>Generated against the MVP spec: structurally valid and deliberately non-executable. Every obligation awaits proof design and review.</p></div>
<span class="chip live">REAL TODAY</span>
</div>
<div class="row">
<span class="ref">spec → proposal</span>
<div class="what"><strong>Deterministic proposal generation</strong>
<p>A coverage skeleton from the exact spec bytes — no semantic proof inference is claimed.</p></div>
<span class="chip live">REAL TODAY</span>
</div>
<div class="row">
<span class="ref">planner · admission</span>
<div class="what"><strong>Semantic planning and review flow</strong>
<p>Typed planning whose output stays reviewable proposal material — never compiler output, never admitted policy.</p></div>
<span class="chip design">IN DESIGN</span>
</div>
<div class="row">
<span class="ref">compiler · manifest</span>
<div class="what"><strong>Deterministic manifest compiler</strong>
<p>Byte-stable compilation with golden-byte conformance fixtures, plus a self-hosting Assurance Spec for Observer itself.</p></div>
<span class="chip design">IN DESIGN</span>
</div>
<div class="row">
<span class="ref">adapters · receipts</span>
<div class="what"><strong>Native adapters and normalized receipts</strong>
<p>Unit, browser, device, property, resilience, security, accessibility, and formal adapters emitting exact Assurance Receipts.</p></div>
<span class="chip planned">PLANNED</span>
</div>
<div class="row">
<span class="ref">qa-swarm</span>
<div class="what"><strong>QA Swarm execution and exploration</strong>
<p>Sharded manifest runs within declared budgets; undistillable exploration stays inconclusive instead of disappearing.</p></div>
<span class="chip planned">PLANNED</span>
</div>
<div class="row">
<span class="ref">observatory</span>
<div class="what"><strong>Hosted Observatory</strong>
<p>A possible multi-project evidence surface — named here as an idea, not a current product claim.</p></div>
<span class="chip planned">PLANNED</span>
</div>
</div>
<p class="reg-note">This page holds itself to the standard it describes: planned surfaces stay visibly unbuilt, and nothing here is a public product promise. Public claims live only in the <a href="/docs/product-promises">product-promise registry</a>.</p>
</section>

<section class="register" id="roadmap">
<h2>WHAT'S COMING</h2>
<div class="tracks">
<div class="track first">
<span class="when">FIRST — LOCAL &amp; OPEN SOURCE</span>
<h3>The proof contract, free</h3>
<ul>
<li>validate product specs, assurance specs, and environment profiles</li>
<li>compile deterministic assurance manifests</li>
<li>run obligations through your own native test tools</li>
<li>local JSON/HTML evidence reports</li>
<li>bring-your-own model for proposal and exploration</li>
<li>no OpenAgents account required</li>
</ul>
<p class="note">Existing tests are imported before anything is generated — Observer rebuilds the harness around a proof-design control file; it does not discard the harnesses that already work.</p>
</div>
<div class="track later">
<span class="when">LATER — HOSTED</span>
<h3>Managed evidence, when it earns it</h3>
<ul>
<li>managed browser, OS, and device matrices</li>
<li>parallel QA Swarm exploration</li>
<li>retained encrypted evidence and trends</li>
<li>opt-in shareable evidence pages</li>
</ul>
<p class="note">The paid service sells managed environments, compute, and evidence retention. It does not hold the basic proof contract hostage.</p>
</div>
</div>
</section>
</main>
${renderLanderFooter()}
</body>
</html>
`

export const handleObserverPage = (request: Request) => {
  if (request.method !== 'GET' && request.method !== 'HEAD') {
    return Effect.succeed(methodNotAllowed(['GET', 'HEAD']))
  }
  const headers = {
    'cache-control': 'no-store',
    'content-type': 'text/html; charset=utf-8',
  }
  return Effect.succeed(
    new Response(request.method === 'HEAD' ? null : renderObserverHtml(), {
      headers,
      status: 200,
    }),
  )
}
