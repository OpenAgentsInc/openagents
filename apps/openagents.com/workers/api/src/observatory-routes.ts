import { Effect } from 'effect'

import { methodNotAllowed } from './http/responses'
import {
  LANDER_HEAD_FONT_PRELOAD,
  LANDER_SHELL_CSS,
  renderLanderFooter,
} from './lander-shell'
import {
  type ObservatoryPublicTraceProjection,
  admitObservatoryProjectionForPublicRead,
  observatoryProjectionDigestMatches,
  openAgentsDesktopMvpPublicTrace,
} from './observatory-public-trace'

export const OPENAGENTS_DESKTOP_MVP_OBSERVATORY_PATH =
  '/observer/traces/openagents-desktop-codex-workroom-mvp' as const

const TRACE_CSS = `
.trace-head{display:grid;grid-template-columns:minmax(0,1fr) auto;gap:24px;align-items:end}
.trace-head h1{margin-block:0;margin-inline:0;font-size:clamp(2rem,5vw,4.7rem)}
.eyebrow,.snapshot,.criterion-ref,.fact-label,.state,.artifact-kind{font-family:var(--mono)}
.eyebrow{font-size:11px;letter-spacing:.08em;color:var(--cyan);text-transform:uppercase}
.lede{max-width:70ch;color:var(--ink-dim);font-size:1rem;text-wrap:pretty}
.snapshot{border:1px solid var(--line);padding-block:12px;padding-inline:14px;color:var(--ink-faint);font-size:11px}
.snapshot strong{display:block;color:var(--ink);font-weight:400;margin-block-start:4px}
.principle{margin-block-start:28px;border-inline-start:2px solid var(--cyan);padding-inline:16px 0;color:var(--ink)}
.criteria{display:grid;gap:18px;margin-block-start:42px}
.criterion{border:1px solid var(--line);background:rgba(15,20,27,.6);padding-block:22px;padding-inline:22px}
.criterion-head{display:flex;gap:16px;align-items:baseline;justify-content:space-between}
.criterion h2{margin-block:0;margin-inline:0;color:#fff;font-size:1.05rem;font-weight:500}
.criterion-ref{color:var(--cyan);font-size:11px;white-space:nowrap}
.facts{display:grid;grid-template-columns:repeat(4,minmax(0,1fr));gap:1px;background:var(--line);margin-block-start:18px;border:1px solid var(--line)}
.fact{background:#0b1017;padding-block:15px;padding-inline:15px;min-height:145px}
.fact-label{font-size:10px;letter-spacing:.08em;color:var(--ink-faint);text-transform:uppercase}
.state{display:block;margin-block-start:8px;font-size:12px;color:#fff}
.state.pending{color:#ffd98a}.state.missing,.state.blocked,.state.not-run{color:#ffb3ad}
.fact p{font-size:.82rem;color:var(--ink-dim);margin-block:9px 0;margin-inline:0;line-height:1.45}
.fact .refs{color:var(--ink-faint);font-size:11px}
.artifacts{margin-block-start:16px;border-block-start:1px solid var(--line);padding-block-start:14px}
.artifacts h3{font-family:var(--mono);font-size:10px;letter-spacing:.08em;color:var(--ink-faint);text-transform:uppercase;margin-block:0 9px;margin-inline:0}
.artifacts ul{display:flex;gap:8px 18px;flex-wrap:wrap;list-style:none;margin-block:0;margin-inline:0;padding-block:0;padding-inline:0}
.artifacts a{color:var(--ink-blue);font-size:.82rem;text-decoration:none}
.artifact-kind{color:var(--ink-faint);font-size:10px;margin-inline-start:5px}
.boundary{margin-block-start:42px;color:var(--ink-faint);font-size:.83rem;max-width:72ch}
@media(max-width:900px){.facts{grid-template-columns:repeat(2,minmax(0,1fr))}}
@media(max-width:620px){.trace-head{grid-template-columns:1fr}.facts{grid-template-columns:1fr}.criterion-head{align-items:flex-start;flex-direction:column-reverse;gap:7px}}
`.trim()

const escapeHtml = (value: string): string =>
  value
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#39;')

const renderRefs = (refs: ReadonlyArray<string>): string =>
  refs.length === 0
    ? '<p class="refs">No refs recorded.</p>'
    : `<p class="refs">${refs.map(escapeHtml).join(' · ')}</p>`

const stateClass = (state: string): string =>
  escapeHtml(state.toLowerCase().replaceAll('_', '-'))

const renderCriterion = (
  criterion: ObservatoryPublicTraceProjection['criteria'][number],
): string => `
<article class="criterion" id="${escapeHtml(criterion.criterionRef)}">
  <header class="criterion-head">
    <h2>${escapeHtml(criterion.title)}</h2>
    <span class="criterion-ref">${escapeHtml(criterion.criterionRef)}</span>
  </header>
  <div class="facts" aria-label="Criterion facts">
    <section class="fact" data-fact="mapped">
      <span class="fact-label">Mapped</span>
      <strong class="state ${stateClass(criterion.mapped.state)}">${escapeHtml(criterion.mapped.state)}</strong>
      <p>${escapeHtml(criterion.mapped.note)}</p>
      ${renderRefs(criterion.mapped.obligationRefs)}
    </section>
    <section class="fact" data-fact="executable">
      <span class="fact-label">Executable</span>
      <strong class="state ${stateClass(criterion.executable.state)}">${escapeHtml(criterion.executable.state)}</strong>
      <p>${escapeHtml(criterion.executable.note)}</p>
      ${renderRefs([
        ...criterion.executable.adapterRefs,
        ...criterion.executable.oracleRefs,
        ...criterion.executable.falsifierRefs,
      ])}
    </section>
    <section class="fact" data-fact="observed">
      <span class="fact-label">Observed</span>
      <strong class="state ${stateClass(criterion.observed.state)}">${escapeHtml(criterion.observed.state)}</strong>
      <p>${escapeHtml(criterion.observed.note)}</p>
      ${renderRefs(criterion.observed.receiptRefs)}
    </section>
    <section class="fact" data-fact="accepted">
      <span class="fact-label">Accepted</span>
      <strong class="state ${stateClass(criterion.accepted.state)}">${escapeHtml(criterion.accepted.state)}</strong>
      <p>${escapeHtml(criterion.accepted.note)}</p>
      ${renderRefs(criterion.accepted.dispositionRefs)}
    </section>
  </div>
  <section class="artifacts">
    <h3>Related Artifacts — locations only, never verdicts</h3>
    <ul>${criterion.relatedArtifacts
      .map(
        artifact =>
          `<li><a href="${escapeHtml(artifact.url)}" rel="noopener">${escapeHtml(artifact.label)}</a><span class="artifact-kind">${escapeHtml(artifact.kind)}</span></li>`,
      )
      .join('')}</ul>
  </section>
</article>`

export const renderObservatoryTraceHtml = (
  projection: ObservatoryPublicTraceProjection,
): string => `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>${escapeHtml(projection.projectLabel)} — Observatory</title>
<meta name="description" content="Criterion-level assurance trace for ${escapeHtml(projection.projectLabel)}.">
${LANDER_HEAD_FONT_PRELOAD}
<style>${LANDER_SHELL_CSS}
${TRACE_CSS}</style>
</head>
<body>
<div class="backdrop" aria-hidden="true"></div>
<div class="masthead"><header class="site shell">
<a class="brand" href="/">OpenAgents<em>.</em></a>
<nav class="site" aria-label="Primary"><a href="/observer">Observer</a><a href="${OPENAGENTS_DESKTOP_MVP_OBSERVATORY_PATH}" aria-current="page">Observatory</a><a href="/docs/product-promises">Promises</a></nav>
<span class="pill"><span class="dot" aria-hidden="true"></span>Reviewed&nbsp;<b>public snapshot</b></span>
</header><hr class="rule"></div>
<main class="shell">
<section class="trace-head">
  <div><p class="eyebrow">Observatory · ${escapeHtml(projection.assuranceProtocol)} protocol</p><h1>${escapeHtml(projection.projectLabel)}<span class="mark">.</span></h1></div>
  <div class="snapshot">Projection <strong>${escapeHtml(projection.projectionRef)}</strong>Generated <strong>${escapeHtml(projection.generatedAt)}</strong></div>
</section>
<p class="lede">A public-safe, human-reviewed projection of criterion evidence. Each criterion reports four independent facts. Missing, blocked, not run, and pending remain visible.</p>
<p class="principle"><strong>Criterion facts, not a score.</strong> Mapped, executable, observed, and accepted are never blended into a percentage or rounded up to green.</p>
<section class="criteria" aria-label="Acceptance criterion traces">${projection.criteria.map(renderCriterion).join('')}</section>
<p class="boundary">Publication boundary: private traces never cross this route. Unlisted traces require their exact link. Public traces appear only after explicit opt-in and a review bound to the projected artifact digest. This snapshot is evidence-only and grants no merge, deploy, spend, settlement, or public-claim authority.</p>
</main>
${renderLanderFooter()}
</body>
</html>`

const notFound = (): Response =>
  new Response('Not found', {
    headers: { 'cache-control': 'no-store', 'content-type': 'text/plain' },
    status: 404,
  })

export const handleObservatoryTracePage = (
  request: Request,
  candidate: unknown = openAgentsDesktopMvpPublicTrace,
) => {
  if (request.method !== 'GET' && request.method !== 'HEAD') {
    return Effect.succeed(methodNotAllowed(['GET', 'HEAD']))
  }

  let projection: ObservatoryPublicTraceProjection | undefined
  try {
    projection = admitObservatoryProjectionForPublicRead(candidate, 'exact')
  } catch {
    return Effect.succeed(notFound())
  }
  const expectedPath =
    projection === undefined
      ? undefined
      : `/observer/traces/${encodeURIComponent(projection.projectRef)}`
  if (
    projection === undefined ||
    new URL(request.url).pathname !== expectedPath
  ) {
    return Effect.succeed(notFound())
  }

  return Effect.promise(async () => {
    if (!(await observatoryProjectionDigestMatches(projection))) {
      return notFound()
    }
    return new Response(
      request.method === 'HEAD' ? null : renderObservatoryTraceHtml(projection),
      {
        headers: {
          'cache-control': 'no-store',
          'content-type': 'text/html; charset=utf-8',
          'x-robots-tag':
            projection.visibility === 'unlisted'
              ? 'noindex, nofollow'
              : 'index, follow',
        },
        status: 200,
      },
    )
  })
}
