import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import * as Ui from '../ui'
import type { PublicHeaderAuthState } from './publicHeader'
import * as PublicHeader from './publicHeader'
import {
  type Step,
  type ToolCall,
  type Trajectory,
  agentSteps,
  formatCost,
  formatDuration,
  formatTokens,
  shortId,
  traceDurationMs,
  traceGoal,
  traceTarget,
  traceVerdict,
  traceVideoSrc,
  verdictLabel,
  verdictTone,
  type VerdictTone,
} from './trace/atif'
import { lookupTrajectory } from './trace/sample'

// Public, shareable render of an ATIF trace at `/trace/{uuid}` (issue #6209).
//
// The shareable surface for one agent session: header (agent, model, verdict,
// duration, cost) → a vertical step timeline (user goal, then each agent step:
// narration, collapsible reasoning, the tool call + args, the observation
// result, inline screenshots) → embedded video → final metrics. No auth to
// view a shared trace.
//
// DESIGN.md: dark/pure-black, warm off-white (#f1efe8), Commit Mono, command
// surfaces + a real timeline (NOT cards). Built with Tailwind utilities via
// `Ui.className` (the public-page pattern, see `terms.ts`) + the shared
// `Ui.proVideoPane` for the embedded recording.
//
// Each timeline step carries a stable `#step-N` anchor + a "copy link to this
// step" affordance + an on-mount hash-scroll (the single best shareable-trace
// primitive: link straight to the exact step). Reasoning is a native
// `<details>` so it is collapsible without client state.

// `h-dvh overflow-auto` (NOT `min-h-dvh`): the global reset pins
// `html, body, #root` to `height: 100%; overflow: hidden` (see styles.css), so
// a `min-h-dvh` shell would grow past the clipped body and the page could never
// scroll — leaving the header stuck at the top with the content below the fold
// unreachable. A fixed `h-dvh` shell is the real scroll container, so the
// (non-sticky) header scrolls off normally. Matches docs.ts / blog.ts /
// download.ts.
const pageShellClass = 'h-dvh overflow-auto bg-[#000] text-[#f1efe8]'

const mono =
  "font-['Commit_Mono',_'Berkeley_Mono',_ui-monospace,_monospace]"

const articleClass = 'mx-auto w-full max-w-3xl px-6 py-10 sm:px-8 sm:py-14'

// ---------------------------------------------------------------------------
// Semantic accent helpers (DESIGN.md tokens, used only for functional state).
// ---------------------------------------------------------------------------

// Exported so the sibling `/trace/compare` view (issue #6211) renders verdict
// state with the exact same DESIGN.md semantic tones as this single-trace page.
export const toneTextClass = (tone: VerdictTone): string => {
  switch (tone) {
    case 'positive':
      return 'text-[#00c853]'
    case 'negative':
      return 'text-[#d32f2f]'
    case 'warning':
      return 'text-[#ff6f00]'
    case 'neutral':
      return 'text-white/60'
  }
}

export const toneBorderClass = (tone: VerdictTone): string => {
  switch (tone) {
    case 'positive':
      return 'border-[#00c853]/40'
    case 'negative':
      return 'border-[#d32f2f]/40'
    case 'warning':
      return 'border-[#ff6f00]/40'
    case 'neutral':
      return 'border-white/15'
  }
}

export const toneDotClass = (tone: VerdictTone): string => {
  switch (tone) {
    case 'positive':
      return 'bg-[#00c853]'
    case 'negative':
      return 'bg-[#d32f2f]'
    case 'warning':
      return 'bg-[#ff6f00]'
    case 'neutral':
      return 'bg-white/40'
  }
}

// ---------------------------------------------------------------------------
// Small shared atoms.
// ---------------------------------------------------------------------------

const metaLabel = <Message>(text: string): Html => {
  const h = html<Message>()
  return h.span(
    [
      Ui.className<Message>(
        'text-[0.625rem] font-semibold uppercase leading-none tracking-wide text-white/35',
      ),
    ],
    [text],
  )
}

const metaValue = <Message>(text: string, extra = ''): Html => {
  const h = html<Message>()
  return h.span(
    [Ui.className<Message>(`text-sm leading-none text-[#f1efe8] ${extra}`)],
    [text],
  )
}

// A stateless "show more / show less" clamp for long text (long reasoning
// paragraphs, long observation results). Uses the checkbox-hack so it works with
// zero client state on a server-rendered public page: a visually-hidden `peer`
// checkbox toggles `line-clamp` off and swaps the toggle label, all in CSS. The
// toggle is only emitted when the content is actually long enough to clamp
// (`CLAMP_THRESHOLD` chars) so short content never shows a pointless affordance.
export const CLAMP_THRESHOLD = 280

export const clampedText = <Message>(
  text: string,
  textClass: string,
  clampClass = 'line-clamp-4',
): Html => {
  const h = html<Message>()

  if (text.length <= CLAMP_THRESHOLD) {
    return h.p([Ui.className<Message>(textClass)], [text])
  }

  return h.div(
    [Ui.className<Message>('grid gap-1.5'), h.DataAttribute('component', 'trace-clamp')],
    [
      h.input([
        h.Type('checkbox'),
        // The peer toggle. `sr-only` keeps it reachable by keyboard (the label
        // is its accessible control) without taking visual space.
        Ui.className<Message>('peer sr-only'),
        h.Id(`clamp-${clampId(text)}`),
      ]),
      h.p(
        [Ui.className<Message>(`${textClass} ${clampClass} peer-checked:line-clamp-none`)],
        [text],
      ),
      h.label(
        [
          h.Attribute('for', `clamp-${clampId(text)}`),
          // The peer-checked variants target the nested label spans via the
          // arbitrary-child selector, so the label text swaps with the peer
          // checkbox state (peer applies to this sibling label).
          // `peer-focus-visible:*` surfaces a keyboard-focus ring on the label
          // because the actual control is the `sr-only` checkbox.
          Ui.className<Message>(
            'inline-flex w-fit cursor-pointer select-none items-center text-[0.7rem] font-semibold uppercase leading-none tracking-wide text-white/45 transition hover:text-[#f1efe8] peer-focus-visible:text-[#f1efe8] peer-focus-visible:outline peer-focus-visible:outline-1 peer-focus-visible:outline-offset-2 peer-focus-visible:outline-[#ffb400] peer-checked:[&>.trace-clamp-more]:hidden [&>.trace-clamp-less]:hidden peer-checked:[&>.trace-clamp-less]:inline',
          ),
        ],
        [
          h.span([Ui.className<Message>('trace-clamp-more')], ['Show more']),
          h.span([Ui.className<Message>('trace-clamp-less')], ['Show less']),
        ],
      ),
    ],
  )
}

// A short, stable, collision-resistant id from the clamped text, so the
// checkbox + its label `for` agree without client state. Deterministic hash.
const clampId = (text: string): string => {
  let hash = 5381
  for (let i = 0; i < text.length; i += 1) {
    hash = ((hash << 5) + hash + text.charCodeAt(i)) >>> 0
  }
  return hash.toString(36)
}

// A header meta cell: stacked label over value, in a strip (no cards).
const metaCell = <Message>(label: string, value: Html): Html => {
  const h = html<Message>()
  return h.div(
    [Ui.className<Message>('grid gap-1.5')],
    [metaLabel<Message>(label), value],
  )
}

// ---------------------------------------------------------------------------
// Header.
// ---------------------------------------------------------------------------

const header = <Message>(trajectory: Trajectory, uuid: string): Html => {
  const h = html<Message>()
  const verdict = traceVerdict(trajectory)
  const tone = verdictTone(verdict)
  const target = traceTarget(trajectory)
  const durationMs = traceDurationMs(trajectory)
  const cost = trajectory.final_metrics?.total_cost_usd
  const model =
    trajectory.agent.model_name ?? trajectory.steps[1]?.model_name ?? 'unknown'

  const verdictBadge = h.div(
    [
      Ui.className<Message>(
        `inline-flex items-center gap-2 self-start border px-3 py-1.5 ${toneBorderClass(tone)}`,
      ),
    ],
    [
      h.span(
        [
          Ui.className<Message>(
            `size-2 shrink-0 rounded-full ${toneDotClass(tone)}`,
          ),
          h.AriaHidden(true),
        ],
        [],
      ),
      h.span(
        [
          Ui.className<Message>(
            `text-xs font-semibold uppercase leading-none tracking-wide ${toneTextClass(tone)}`,
          ),
        ],
        [verdictLabel(verdict)],
      ),
    ],
  )

  const metaCells: ReadonlyArray<Html> = [
    metaCell<Message>(
      'Agent',
      metaValue<Message>(trajectory.agent.name, mono),
    ),
    metaCell<Message>('Model', metaValue<Message>(model, mono)),
    ...(target !== undefined
      ? [metaCell<Message>('Target', metaValue<Message>(target.name, mono))]
      : []),
    ...(durationMs !== undefined
      ? [
          metaCell<Message>(
            'Duration',
            metaValue<Message>(formatDuration(durationMs), mono),
          ),
        ]
      : []),
    ...(cost !== undefined
      ? [
          metaCell<Message>(
            'Cost',
            metaValue<Message>(formatCost(cost), mono),
          ),
        ]
      : []),
  ]

  return h.header(
    [Ui.className<Message>('grid gap-6 border-b border-[#222] pb-8')],
    [
      h.div(
        [
          Ui.className<Message>(
            'flex flex-wrap items-center justify-between gap-3',
          ),
        ],
        [
          h.span(
            [
              Ui.className<Message>(
                `text-[0.625rem] uppercase leading-none tracking-[0.2em] text-white/35 ${mono}`,
              ),
            ],
            [`trace · ${shortId(uuid)}`],
          ),
          verdictBadge,
        ],
      ),
      h.h1(
        [
          Ui.className<Message>(
            'm-0 text-2xl font-semibold leading-tight tracking-tight text-[#f1efe8] sm:text-3xl',
          ),
          h.Attribute('style', 'text-wrap: balance'),
        ],
        ['Agent session trace'],
      ),
      h.div(
        [
          Ui.className<Message>(
            'grid grid-cols-2 gap-x-8 gap-y-5 sm:grid-cols-3 md:grid-cols-5',
          ),
        ],
        metaCells,
      ),
    ],
  )
}

// ---------------------------------------------------------------------------
// Timeline.
// ---------------------------------------------------------------------------

// The "copy link to this step" button: copies the absolute `#step-N` URL.
// Inline `onclick` (the same no-state affordance pattern used by the login
// button in view.ts) — keeps the page a stateless public shell.
const copyLinkButton = <Message>(anchorId: string): Html => {
  const h = html<Message>()
  return h.button(
    [
      h.Type('button'),
      h.AriaLabel('Copy link to this step'),
      h.Title('Copy link to this step'),
      Ui.className<Message>(
        'inline-flex size-6 shrink-0 items-center justify-center border border-transparent text-white/25 opacity-0 transition hover:border-[#222] hover:text-[#f1efe8] focus-visible:opacity-100 focus-visible:outline focus-visible:outline-1 focus-visible:outline-[#ffb400] group-hover:opacity-100',
      ),
      h.Attribute(
        'onclick',
        `(function(b){var u=location.origin+location.pathname+'#${anchorId}';try{navigator.clipboard.writeText(u);}catch(e){}var p=b.querySelector('[data-copy-mark]');if(p){p.textContent='copied';setTimeout(function(){p.textContent='link';},1200);}})(this)`,
      ),
    ],
    [
      h.span(
        [
          h.DataAttribute('copy-mark', ''),
          Ui.className<Message>('sr-only'),
        ],
        ['link'],
      ),
      // A small link glyph.
      h.svg(
        [
          h.AriaHidden(true),
          Ui.className<Message>('size-3.5'),
          h.Xmlns('http://www.w3.org/2000/svg'),
          h.ViewBox('0 0 24 24'),
          h.Fill('none'),
          h.Attribute('stroke', 'currentColor'),
          h.Attribute('stroke-width', '2'),
          h.Attribute('stroke-linecap', 'round'),
          h.Attribute('stroke-linejoin', 'round'),
        ],
        [
          h.path(
            [
              h.D(
                'M10 13a5 5 0 0 0 7.07 0l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71',
              ),
            ],
            [],
          ),
          h.path(
            [
              h.D(
                'M14 11a5 5 0 0 0-7.07 0l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71',
              ),
            ],
            [],
          ),
        ],
      ),
    ],
  )
}

// A timeline node wrapper: the rail dot + connecting line + the step body. The
// `id` makes each step a stable deep-link anchor (`#step-N` / `#goal`).
const timelineNode = <Message>(
  options: Readonly<{
    anchorId: string
    dotClass: string
    isLast: boolean
    head: Html
    body: ReadonlyArray<Html>
  }>,
): Html => {
  const h = html<Message>()
  return h.li(
    [
      h.Id(options.anchorId),
      Ui.className<Message>('group relative grid grid-cols-[auto_1fr] gap-x-4'),
    ],
    [
      // Rail column: dot + line.
      h.div(
        [Ui.className<Message>('relative flex flex-col items-center')],
        [
          h.span(
            [
              Ui.className<Message>(
                `relative z-10 mt-1 size-2.5 shrink-0 rounded-full ring-4 ring-black ${options.dotClass}`,
              ),
              h.AriaHidden(true),
            ],
            [],
          ),
          ...(options.isLast
            ? []
            : [
                h.span(
                  [
                    Ui.className<Message>(
                      'mt-1 w-px flex-1 bg-[#222]',
                    ),
                    h.AriaHidden(true),
                  ],
                  [],
                ),
              ]),
        ],
      ),
      // Content column.
      h.div(
        [Ui.className<Message>('min-w-0 pb-8')],
        [
          h.div(
            [
              Ui.className<Message>(
                'flex items-start justify-between gap-2',
              ),
            ],
            [options.head, copyLinkButton<Message>(options.anchorId)],
          ),
          ...options.body,
        ],
      ),
    ],
  )
}

const goalNode = <Message>(goal: string): Html => {
  const h = html<Message>()
  return timelineNode<Message>({
    anchorId: 'goal',
    dotClass: 'bg-[#ffb400]',
    isLast: false,
    head: h.div(
      [Ui.className<Message>('grid gap-1')],
      [
        h.span(
          [
            Ui.className<Message>(
              `text-[0.625rem] font-semibold uppercase leading-none tracking-wide text-[#ffb400] ${mono}`,
            ),
          ],
          ['Goal'],
        ),
      ],
    ),
    body: [
      h.p(
        [Ui.className<Message>('mt-2 text-base/7 text-white/85')],
        [goal],
      ),
    ],
  })
}

// The tool-call block: function name + the (public-safe) arguments rendered as
// a compact mono key/value list.
const toolCallBlock = <Message>(call: ToolCall): Html => {
  const h = html<Message>()
  const args = Object.entries(call.arguments)

  const argRows = args.map(([key, value]) => {
    const rendered =
      typeof value === 'string' ? value : stringifyJson(value)
    return h.div(
      [Ui.className<Message>('grid grid-cols-[auto_1fr] gap-x-3')],
      [
        h.span(
          [Ui.className<Message>(`text-white/40 ${mono}`)],
          [`${key}`],
        ),
        h.span(
          [Ui.className<Message>(`break-words text-[#f1efe8] ${mono}`)],
          [rendered],
        ),
      ],
    )
  })

  return h.div(
    [
      Ui.className<Message>(
        'mt-3 border border-[#222] bg-[#010102] p-3 text-xs leading-5',
      ),
      h.DataAttribute('component', 'trace-tool-call'),
    ],
    [
      h.div(
        [Ui.className<Message>('flex items-center gap-2')],
        [
          h.span(
            [
              Ui.className<Message>(
                'text-[0.625rem] font-semibold uppercase leading-none tracking-wide text-[#2979ff]',
              ),
            ],
            ['Tool call'],
          ),
          h.span(
            [Ui.className<Message>(`text-sm text-[#f1efe8] ${mono}`)],
            [`${call.function_name}()`],
          ),
        ],
      ),
      ...(argRows.length > 0
        ? [h.div([Ui.className<Message>('mt-2.5 grid gap-1')], argRows)]
        : []),
    ],
  )
}

const stringifyJson = (value: unknown): string => {
  try {
    return JSON.stringify(value)
  } catch {
    return String(value)
  }
}

// The observation block: each result, correlated by its source call id.
const observationBlock = <Message>(
  results: Step['observation'] extends infer O
    ? O extends { results: infer R }
      ? R
      : never
    : never,
): Html => {
  const h = html<Message>()
  const rows = results.map(result => {
    const content = result.content ?? ''
    const failed = content.startsWith('FAILED')
    // Long observation dumps (e.g. a verbose FAILED result) clamp to a few
    // lines with a stateless "show more" toggle so one noisy step can't swamp
    // the timeline; short results render inline unchanged.
    return h.div(
      [Ui.className<Message>('grid gap-1')],
      [
        clampedText<Message>(
          content,
          `break-words whitespace-pre-wrap text-sm leading-6 ${failed ? 'text-[#d32f2f]' : 'text-white/75'} ${mono}`,
        ),
        ...(result.source_call_id !== undefined
          ? [
              h.span(
                [
                  Ui.className<Message>(
                    `text-[0.625rem] leading-none text-white/30 ${mono}`,
                  ),
                ],
                [`← ${result.source_call_id}`],
              ),
            ]
          : []),
      ],
    )
  })

  return h.div(
    [
      Ui.className<Message>(
        'mt-3 border-l-0 border-t border-[#222] pt-3',
      ),
      h.DataAttribute('component', 'trace-observation'),
    ],
    [
      h.span(
        [
          Ui.className<Message>(
            'text-[0.625rem] font-semibold uppercase leading-none tracking-wide text-white/40',
          ),
        ],
        ['Observation'],
      ),
      h.div([Ui.className<Message>('mt-2 grid gap-2')], rows),
    ],
  )
}

// Collapsible reasoning via native <details> (no client state needed). The last
// step's reasoning is expanded by default (auto-disclose the active step).
const reasoningBlock = <Message>(
  reasoning: string,
  expanded: boolean,
): Html => {
  const h = html<Message>()
  return h.details(
    [
      Ui.className<Message>('mt-3 border-t border-[#1d1d1d] pt-3'),
      ...(expanded ? [h.Attribute('open', 'open')] : []),
      h.DataAttribute('component', 'trace-reasoning'),
    ],
    [
      h.summary(
        [
          Ui.className<Message>(
            'cursor-pointer select-none text-[0.66rem] font-semibold uppercase leading-none tracking-wide text-white/45 hover:text-[#f1efe8] focus-visible:outline focus-visible:outline-1 focus-visible:outline-[#ffb400]',
          ),
        ],
        ['Reasoning'],
      ),
      h.div(
        [Ui.className<Message>('mt-2.5')],
        [clampedText<Message>(reasoning, 'm-0 text-sm/6 text-white/65')],
      ),
    ],
  )
}

const stepNode = <Message>(
  step: Step,
  isLast: boolean,
  index: number,
): Html => {
  const h = html<Message>()
  const anchorId = `step-${step.step_id}`
  const isDone = (step.tool_calls ?? []).some(
    call => call.function_name === 'done',
  )
  const dotClass = isDone ? 'bg-[#00c853]' : 'bg-white/55'
  const stepMetrics = step.metrics
  const tokenBits: string[] = []
  if (stepMetrics?.prompt_tokens !== undefined)
    tokenBits.push(`${formatTokens(stepMetrics.prompt_tokens)} in`)
  if (stepMetrics?.completion_tokens !== undefined)
    tokenBits.push(`${formatTokens(stepMetrics.completion_tokens)} out`)
  if (stepMetrics?.cost_usd !== undefined)
    tokenBits.push(formatCost(stepMetrics.cost_usd))

  const head = h.div(
    [Ui.className<Message>('grid min-w-0 gap-1.5')],
    [
      h.div(
        [Ui.className<Message>('flex items-center gap-2')],
        [
          h.span(
            [
              Ui.className<Message>(
                `text-[0.625rem] font-semibold uppercase leading-none tracking-wide text-white/30 ${mono}`,
              ),
            ],
            [`Step ${step.step_id}`],
          ),
          ...(step.timestamp !== undefined
            ? [
                h.span(
                  [
                    Ui.className<Message>(
                      `text-[0.625rem] leading-none text-white/25 ${mono}`,
                    ),
                  ],
                  [formatTimestamp(step.timestamp)],
                ),
              ]
            : []),
        ],
      ),
      h.p(
        [
          Ui.className<Message>(
            'm-0 break-words text-base font-medium leading-6 text-[#f1efe8]',
          ),
        ],
        [step.message],
      ),
      ...(tokenBits.length > 0
        ? [
            h.span(
              [
                Ui.className<Message>(
                  `text-[0.625rem] leading-none text-white/30 ${mono}`,
                ),
              ],
              [tokenBits.join(' · ')],
            ),
          ]
        : []),
    ],
  )

  const body: Html[] = []
  for (const call of step.tool_calls ?? []) {
    body.push(toolCallBlock<Message>(call))
  }
  if (step.observation !== undefined && step.observation.results.length > 0) {
    body.push(observationBlock<Message>(step.observation.results))
  }
  void index
  if (step.reasoning_content !== undefined) {
    body.push(reasoningBlock<Message>(step.reasoning_content, isLast))
  }

  return timelineNode<Message>({
    anchorId,
    dotClass,
    isLast,
    head,
    body,
  })
}

// Show the time-of-day from an ISO-8601 timestamp (e.g. `14:39:44Z`). Pure
// string slicing — no `Date` parsing — so it is deterministic and avoids the
// raw time-primitive boundary (the timestamp is already a normalized UTC ISO
// string from the trajectory; we never reinterpret it in a local timezone).
const ISO_TIME_RE = /^\d{4}-\d{2}-\d{2}T(\d{2}:\d{2}:\d{2})/
const formatTimestamp = (iso: string): string => {
  const match = ISO_TIME_RE.exec(iso)
  return match === null ? iso : `${match[1]}Z`
}

const timeline = <Message>(trajectory: Trajectory): Html => {
  const h = html<Message>()
  const goal = traceGoal(trajectory)
  const steps = agentSteps(trajectory)

  const nodes: Html[] = []
  if (goal !== undefined) nodes.push(goalNode<Message>(goal))
  steps.forEach((step, i) => {
    nodes.push(stepNode<Message>(step, i === steps.length - 1, i))
  })

  return h.section(
    [Ui.className<Message>('mt-10')],
    [
      sectionHeading<Message>('Timeline'),
      h.ol(
        [
          Ui.className<Message>('mt-6 flex flex-col list-none p-0'),
          h.DataAttribute('component', 'trace-timeline'),
        ],
        nodes,
      ),
    ],
  )
}

// ---------------------------------------------------------------------------
// Video + final metrics.
// ---------------------------------------------------------------------------

const sectionHeading = <Message>(text: string): Html => {
  const h = html<Message>()
  return h.h2(
    [
      Ui.className<Message>(
        'text-[0.7rem] font-semibold uppercase leading-none tracking-[0.18em] text-white/45',
      ),
    ],
    [text],
  )
}

const videoSection = <Message>(src: string): Html => {
  const h = html<Message>()
  return h.section(
    [Ui.className<Message>('mt-12')],
    [
      sectionHeading<Message>('Recording'),
      h.div(
        [Ui.className<Message>('mt-5')],
        [
          Ui.proVideoPane<Message>({
            src,
            format: 'webm',
            label: 'Session recording (public-safe).',
          }),
        ],
      ),
    ],
  )
}

const metricStat = <Message>(label: string, value: string): Html => {
  const h = html<Message>()
  return h.div(
    [
      Ui.className<Message>(
        'grid gap-2 border border-[#222] bg-[#010102] px-4 py-3.5',
      ),
    ],
    [
      h.span(
        [
          Ui.className<Message>(
            'text-[0.625rem] font-semibold uppercase leading-none tracking-wide text-white/35',
          ),
        ],
        [label],
      ),
      h.span(
        [Ui.className<Message>(`text-lg leading-none text-[#f1efe8] ${mono}`)],
        [value],
      ),
    ],
  )
}

const finalMetricsSection = <Message>(trajectory: Trajectory): Html => {
  const h = html<Message>()
  const fm = trajectory.final_metrics
  const verdict = traceVerdict(trajectory)
  const durationMs = traceDurationMs(trajectory)

  const stats: Html[] = [
    metricStat<Message>('Verdict', verdictLabel(verdict)),
    ...(durationMs !== undefined
      ? [metricStat<Message>('Duration', formatDuration(durationMs))]
      : []),
    ...(fm?.total_steps !== undefined
      ? [metricStat<Message>('Steps', String(fm.total_steps))]
      : []),
    ...(fm?.total_prompt_tokens !== undefined
      ? [
          metricStat<Message>(
            'Prompt tokens',
            formatTokens(fm.total_prompt_tokens),
          ),
        ]
      : []),
    ...(fm?.total_completion_tokens !== undefined
      ? [
          metricStat<Message>(
            'Completion tokens',
            formatTokens(fm.total_completion_tokens),
          ),
        ]
      : []),
    ...(fm?.total_cost_usd !== undefined
      ? [metricStat<Message>('Cost', formatCost(fm.total_cost_usd))]
      : []),
  ]

  return h.section(
    [Ui.className<Message>('mt-12')],
    [
      sectionHeading<Message>('Final metrics'),
      h.div(
        [
          Ui.className<Message>(
            'mt-5 grid grid-cols-2 gap-3 sm:grid-cols-3',
          ),
          h.DataAttribute('component', 'trace-final-metrics'),
        ],
        stats,
      ),
      ...(trajectory.notes !== undefined
        ? [
            h.p(
              [
                Ui.className<Message>(
                  'mt-5 max-w-prose text-xs leading-5 text-white/35',
                ),
              ],
              [trajectory.notes],
            ),
          ]
        : []),
    ],
  )
}

// On-mount, scroll to the `#step-N` in the URL hash (deep-link landing) once the
// timeline has rendered. Inert if there is no hash.
const hashScrollScript = <Message>(): Html => {
  const h = html<Message>()
  return h.script(
    [],
    [
      "(function(){function go(){if(location.hash){var el=document.getElementById(location.hash.slice(1));if(el){el.scrollIntoView({block:'center'});}}}if(document.readyState!=='loading'){go();}else{document.addEventListener('DOMContentLoaded',go);}})();",
    ],
  )
}

// ---------------------------------------------------------------------------
// Found / not-found bodies.
// ---------------------------------------------------------------------------

const traceArticle = <Message>(
  trajectory: Trajectory,
  uuid: string,
): Html => {
  const h = html<Message>()
  const videoSrc = traceVideoSrc(trajectory)

  return h.article(
    [
      Ui.className<Message>(articleClass),
      h.DataAttribute('component', 'trace-page'),
    ],
    [
      header<Message>(trajectory, uuid),
      timeline<Message>(trajectory),
      ...(videoSrc !== undefined ? [videoSection<Message>(videoSrc)] : []),
      finalMetricsSection<Message>(trajectory),
      hashScrollScript<Message>(),
    ],
  )
}

const notFoundArticle = <Message>(uuid: string): Html => {
  const h = html<Message>()
  return h.article(
    [
      Ui.className<Message>(
        `${articleClass} flex min-h-[60dvh] flex-col items-center justify-center text-center`,
      ),
      h.DataAttribute('component', 'trace-not-found'),
    ],
    [
      h.span(
        [
          Ui.className<Message>(
            `text-[0.625rem] uppercase leading-none tracking-[0.2em] text-white/30 ${mono}`,
          ),
        ],
        ['trace · not found'],
      ),
      h.h1(
        [
          Ui.className<Message>(
            'mt-4 text-2xl font-semibold tracking-tight text-[#f1efe8] sm:text-3xl',
          ),
        ],
        ['No trace at this link'],
      ),
      h.p(
        [
          Ui.className<Message>(
            `mt-3 max-w-md text-sm leading-6 text-white/50 ${mono}`,
          ),
        ],
        [`No shared trace matched ${shortId(uuid)}.`],
      ),
      h.p(
        [Ui.className<Message>('mt-2 max-w-md text-sm leading-6 text-white/45')],
        [
          'The link may be wrong, or the trace was never shared. Traces are immutable once shared.',
        ],
      ),
      h.a(
        [
          h.Href('/'),
          Ui.className<Message>(
            'mt-8 inline-flex items-center border border-[#222] px-4 py-2 text-sm text-[#f1efe8] transition hover:border-[#333] hover:bg-[#080808] focus-visible:outline focus-visible:outline-1 focus-visible:outline-[#ffb400]',
          ),
        ],
        ['Back to OpenAgents'],
      ),
    ],
  )
}

// A bounded skeleton for the loading state (used while a real read-API fetch is
// in flight, once wired). Exported for the harness + future loading wiring.
export const skeletonArticle = <Message>(): Html => {
  const h = html<Message>()
  const bar = (w: string): Html =>
    h.div(
      [Ui.className<Message>(`h-3 ${w} animate-pulse rounded bg-white/10`)],
      [],
    )
  return h.article(
    [
      Ui.className<Message>(articleClass),
      h.DataAttribute('component', 'trace-skeleton'),
      h.AriaBusy(true),
      h.AriaLabel('Loading trace'),
    ],
    [
      h.div(
        [Ui.className<Message>('grid gap-4 border-b border-[#222] pb-8')],
        [bar('w-24'), bar('w-2/3'), bar('w-1/2')],
      ),
      h.div(
        [Ui.className<Message>('mt-10 grid gap-6')],
        [bar('w-1/3'), bar('w-full'), bar('w-5/6'), bar('w-3/4')],
      ),
    ],
  )
}

export type TraceRouteLike = Readonly<{ _tag: 'Trace'; uuid: string }>

// The page view. Public, no auth. Renders the committed sample for the known
// uuid; an honest not-found body otherwise. When the worker read API lands,
// `lookupTrajectory` becomes an async fetch + decode and the loading state uses
// `skeletonArticle`; this render branch is unchanged.
export const view = <Message>(
  route: TraceRouteLike,
  authState: PublicHeaderAuthState<Message>,
): Html => {
  const h = html<Message>()
  const trajectory = lookupTrajectory(route.uuid)

  const body =
    trajectory !== undefined
      ? traceArticle<Message>(trajectory, route.uuid)
      : notFoundArticle<Message>(route.uuid)

  return h.div(
    [Ui.className<Message>(pageShellClass)],
    [PublicHeader.view(authState), body],
  )
}

// The document title for a trace, used by view.ts. Keep it shareable-friendly.
export const title = (route: TraceRouteLike): string => {
  const trajectory = lookupTrajectory(route.uuid)
  if (trajectory === undefined) return 'Trace not found - OpenAgents'
  const verdict = verdictLabel(traceVerdict(trajectory))
  const target = traceTarget(trajectory)
  return target !== undefined
    ? `Trace: ${target.name} (${verdict}) - OpenAgents`
    : `Trace (${verdict}) - OpenAgents`
}
