import { AiElements } from '@openagentsinc/ui'
import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import * as Ui from '../ui'
import type { PublicHeaderAuthState } from './publicHeader'
import * as PublicHeader from './publicHeader'
import {
  type BlobRef,
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
  traceScreenshotBlobSrcs,
  traceTarget,
  traceVerdict,
  traceVideoBlobSrc,
  trajectoryToMarkdown,
  verdictLabel,
  verdictTone,
  type VerdictTone,
} from './trace/atif'
import { SAMPLE_TRACE_UUID, lookupTrajectory } from './trace/sample'

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

// ---------------------------------------------------------------------------
// Markdown prose rendering.
// ---------------------------------------------------------------------------

// Agent narration (`message`), the user `goal`, model `reasoning_content`, and
// prose observations (agent sub-reports) all carry real Markdown — `**bold**`,
// `## headings`, `-` lists, `` `code` ``, fenced blocks. Render them through the
// shared `@openagentsinc/ui` AI Elements `response` renderer, the exact same
// streaming-tolerant, dark-only, no-innerHTML Markdown renderer every chat
// surface (Khala, Autopilot onboarding) uses. This keeps one Markdown
// implementation for the whole product and inherits its safe-href guard +
// auto-escaping (a malformed model reply can only ever produce text nodes).
//
// Tool-call args + file paths + tool stdout are deliberately NOT routed here:
// they are code/commands, kept as monospace `<pre>` (see `clampedPre`).
const prose = <Message>(markdown: string, extraClass = ''): Html => {
  const h = html<Message>()
  // The response renderer ships its own `responseClass` (grid gap, off-white
  // ink, overflow-wrap). Wrap it so callers can add timeline spacing without
  // re-styling the renderer internals.
  return h.div(
    [Ui.className<Message>(extraClass)],
    [AiElements.response<Message>({ markdown })],
  )
}

// A heuristic: does this observation result read as agent prose (a sub-report
// to render as Markdown), or as raw tool stdout (a file listing, grep output,
// git output — render verbatim in a monospace `<pre>`)? Prose tends to use
// Markdown structure (headings, bold, bullet prose); stdout tends to be many
// short lines, tab/colon-delimited rows, or path-like tokens. We bias toward
// `<pre>` (the safe, faithful default for machine output) and only treat content
// as prose when it shows clear Markdown structure AND reads like sentences.
const looksLikeProse = (content: string): boolean => {
  const trimmed = content.trim()
  if (trimmed === '') return false
  // Strong Markdown structure signals (a heading, or bold emphasis in running
  // text) that tool stdout effectively never emits.
  const hasHeading = /^#{1,6}\s+\S/m.test(trimmed)
  const hasBold = /\*\*[^*\n]+\*\*/.test(trimmed)
  if (!hasHeading && !hasBold) return false
  // Guard against stdout that merely contains a `#` or `*`: require some prose
  // line length (a real sentence) somewhere, not just short tokens/paths.
  const lines = trimmed.split('\n')
  const hasSentence = lines.some(
    line => line.trim().length > 60 && /[.:!?]\s|\S\s\S+\s\S+\s\S+/.test(line),
  )
  return hasSentence
}

// A stateless monospace "show more / show less" clamp for long verbatim text:
// tool-call arg values (commands, prompts, diffs) and tool stdout. Same
// checkbox-hack as `clampedText` (zero client state, keyboard-focusable,
// reduced-motion aware), but the content is a real `<pre>` so newlines,
// indentation, and code formatting are preserved exactly. Short content renders
// as a plain inline `<pre>` with no toggle.
export const PRE_CLAMP_THRESHOLD = 200

export const clampedPre = <Message>(
  text: string,
  preClass: string,
  clampClass = 'max-h-32',
): Html => {
  const h = html<Message>()

  const preBase = `m-0 overflow-x-auto whitespace-pre-wrap break-words ${preClass}`

  if (text.length <= PRE_CLAMP_THRESHOLD) {
    return h.pre([Ui.className<Message>(preBase)], [text])
  }

  const id = `clamp-${clampId(text)}`
  return h.div(
    [
      Ui.className<Message>('grid gap-1.5'),
      h.DataAttribute('component', 'trace-clamp'),
    ],
    [
      h.input([
        h.Type('checkbox'),
        Ui.className<Message>('peer sr-only'),
        h.Id(id),
      ]),
      // The clamped `<pre>`: a max-height window + a soft fade hint at the
      // bottom edge (a linear-gradient mask) so the truncation reads as "more
      // below", not a hard cut. `peer-checked` lifts both the height cap and the
      // fade. The mask is purely cosmetic and degrades gracefully.
      h.pre(
        [
          Ui.className<Message>(
            `${preBase} ${clampClass} overflow-y-hidden [mask-image:linear-gradient(to_bottom,#000_60%,transparent)] peer-checked:max-h-none peer-checked:overflow-y-auto peer-checked:[mask-image:none]`,
          ),
        ],
        [text],
      ),
      h.label(
        [
          h.Attribute('for', id),
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

// Markdown prose, collapsed by default past a few lines with the same stateless
// show-more toggle, for long prose observations (agent sub-reports). Short prose
// renders inline. The clamp uses a `max-h` window (Markdown renders to varied
// block elements, so a line-clamp on a single element does not apply); the
// checkbox-hack lifts the cap. Mirrors `clampedPre`'s affordance for visual
// consistency across the timeline.
const clampedProse = <Message>(markdown: string): Html => {
  const h = html<Message>()

  if (markdown.length <= CLAMP_THRESHOLD) {
    return prose<Message>(markdown)
  }

  const id = `clamp-${clampId(markdown)}`
  return h.div(
    [
      Ui.className<Message>('grid gap-1.5'),
      h.DataAttribute('component', 'trace-clamp'),
    ],
    [
      h.input([
        h.Type('checkbox'),
        Ui.className<Message>('peer sr-only'),
        h.Id(id),
      ]),
      h.div(
        [
          Ui.className<Message>(
            'relative max-h-48 overflow-hidden [mask-image:linear-gradient(to_bottom,#000_70%,transparent)] peer-checked:max-h-none peer-checked:overflow-visible peer-checked:[mask-image:none]',
          ),
        ],
        [AiElements.response<Message>({ markdown })],
      ),
      h.label(
        [
          h.Attribute('for', id),
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

// A header meta cell: stacked label over value, in a full-width strip (no
// cards). `flex-1` + a min width lets the cells share the row and use the
// available width (#6223); `basis-0` keeps the share even regardless of value
// length.
const metaCell = <Message>(label: string, value: Html): Html => {
  const h = html<Message>()
  return h.div(
    [Ui.className<Message>('grid flex-1 basis-28 content-start gap-1.5')],
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
          h.div(
            [Ui.className<Message>('flex items-center gap-2')],
            [copyMarkdownButton<Message>(trajectory), verdictBadge],
          ),
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
      // Full-width meta strip (#6223): a single horizontal row of cells that
      // uses the available width, divided by hairline borders, instead of a
      // narrow stacked column. `flex-1` lets each cell share the row evenly; it
      // wraps to a second line only when the viewport is genuinely too narrow.
      h.div(
        [
          Ui.className<Message>(
            'flex flex-wrap items-stretch gap-x-8 gap-y-5 border-t border-[#1d1d1d] pt-5',
          ),
          h.DataAttribute('component', 'trace-meta'),
        ],
        metaCells,
      ),
    ],
  )
}

// The "Copy all as Markdown" button (#6223): serializes the WHOLE trajectory to
// clean Markdown and copies it to the clipboard. Stateless inline `onclick` (the
// same no-framework affordance pattern as the per-step copy-link button), with
// the serialized Markdown embedded as a data attribute so no client state or
// fetch is needed. The label swaps to "Copied" for a beat on success.
const copyMarkdownButton = <Message>(trajectory: Trajectory): Html => {
  const h = html<Message>()
  const markdown = trajectoryToMarkdown(trajectory)
  return h.button(
    [
      h.Type('button'),
      h.AriaLabel('Copy the whole trace as Markdown'),
      h.Title('Copy the whole trace as Markdown'),
      h.DataAttribute('component', 'trace-copy-markdown'),
      h.DataAttribute('markdown', markdown),
      Ui.className<Message>(
        `inline-flex items-center gap-1.5 border border-[#222] px-3 py-1.5 text-[0.7rem] font-semibold uppercase leading-none tracking-wide text-white/55 transition hover:border-[#333] hover:text-[#f1efe8] focus-visible:outline focus-visible:outline-1 focus-visible:outline-[#ffb400] ${mono}`,
      ),
      h.Attribute(
        'onclick',
        `(function(b){var m=b.getAttribute('data-markdown')||'';try{navigator.clipboard.writeText(m);}catch(e){}var l=b.querySelector('[data-copy-label]');if(l){var t=l.textContent;l.textContent='Copied';setTimeout(function(){l.textContent=t;},1400);}})(this)`,
      ),
    ],
    [
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
          h.rect(
            [
              h.Attribute('x', '9'),
              h.Attribute('y', '9'),
              h.Attribute('width', '13'),
              h.Attribute('height', '13'),
              h.Attribute('rx', '2'),
            ],
            [],
          ),
          h.path(
            [h.D('M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1')],
            [],
          ),
        ],
      ),
      h.span([h.DataAttribute('copy-label', '')], ['Copy Markdown']),
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
    body: [prose<Message>(goal, 'mt-2 text-white/85')],
  })
}

// The tool-call block: function name + the (public-safe) arguments rendered as
// a compact mono key/value list.
const toolCallBlock = <Message>(call: ToolCall): Html => {
  const h = html<Message>()
  const args = Object.entries(call.arguments)

  // Arg values are code/commands (a multi-line Bash `command`, a giant Agent()
  // `prompt`, a diff `old_string`/`new_string`, a file path) — kept monospace,
  // NEVER Markdown. A short single-line value renders inline next to its key; a
  // long or multi-line value collapses by default in a clamped `<pre>` so one
  // giant arg can't dump full-height and swamp the timeline.
  const argRows = args.map(([key, value]) => {
    const rendered =
      typeof value === 'string' ? value : stringifyJson(value)
    const isLong =
      rendered.length > PRE_CLAMP_THRESHOLD || rendered.includes('\n')

    if (!isLong) {
      return h.div(
        [Ui.className<Message>('grid grid-cols-[auto_1fr] gap-x-3')],
        [
          h.span([Ui.className<Message>(`text-white/40 ${mono}`)], [`${key}`]),
          h.span(
            [Ui.className<Message>(`break-words text-[#f1efe8] ${mono}`)],
            [rendered],
          ),
        ],
      )
    }

    // Long value: key on its own line, then a collapsible monospace block.
    return h.div(
      [Ui.className<Message>('grid gap-1')],
      [
        h.span([Ui.className<Message>(`text-white/40 ${mono}`)], [`${key}`]),
        clampedPre<Message>(rendered, `text-[#f1efe8] ${mono} text-xs leading-5`),
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

    // An observation result is EITHER an agent sub-report (prose Markdown — an
    // Agent() tool returning a written summary) OR raw tool stdout (a file
    // listing, grep output, git output). Prose renders through the shared
    // Markdown renderer, collapsed past a few lines; stdout renders verbatim in
    // a clamped monospace `<pre>` so newlines and columns stay aligned and the
    // (sometimes huge) dump can't swamp the timeline. A failed result is always
    // shown verbatim in the negative tone.
    const bodyNode =
      !failed && looksLikeProse(content)
        ? clampedProse<Message>(content)
        : clampedPre<Message>(
            content,
            `text-sm leading-6 ${failed ? 'text-[#d32f2f]' : 'text-white/75'} ${mono}`,
          )

    return h.div(
      [Ui.className<Message>('grid gap-1.5')],
      [
        bodyNode,
        ...(result.source_call_id !== undefined
          ? [
              h.span(
                [
                  Ui.className<Message>(
                    `text-[0.625rem] leading-none text-white/25 ${mono}`,
                  ),
                ],
                [`↳ ${result.source_call_id}`],
              ),
            ]
          : []),
      ],
    )
  })

  return h.div(
    [
      Ui.className<Message>('mt-3 border-t border-[#222] pt-3'),
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
      h.div([Ui.className<Message>('mt-2.5 grid gap-3')], rows),
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
      // Reasoning is model prose — Markdown, collapsed past a few lines with the
      // same stateless show-more toggle the rest of the timeline uses.
      h.div(
        [Ui.className<Message>('mt-2.5 text-white/65')],
        [clampedProse<Message>(reasoning)],
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
      // The agent narration carries Markdown (`**bold**`, `##`, lists, code) on
      // real steps — render it as prose, not raw monospace. The `text-[#f1efe8]`
      // + medium weight keeps narration the visually-dominant line of the step.
      prose<Message>(
        step.message,
        'break-words text-[#f1efe8] [&_p]:font-medium [&_p]:leading-6',
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
  blobRefs?: ReadonlyArray<BlobRef>,
): Html => {
  const h = html<Message>()
  // #6223: prefer the envelope blobRefs for the recording (a real ingested
  // trace), falling back to the committed sample's bundled clip. Screenshots
  // come only from blobRefs. The recording renders INLINE near the top (right
  // after the header) so the visual evidence leads, not trails, the timeline.
  const videoSrc = traceVideoBlobSrc(uuid, blobRefs, trajectory)
  const screenshots = traceScreenshotBlobSrcs(uuid, blobRefs)

  return h.article(
    [
      Ui.className<Message>(articleClass),
      h.DataAttribute('component', 'trace-page'),
    ],
    [
      header<Message>(trajectory, uuid),
      ...(videoSrc !== undefined ? [videoSection<Message>(videoSrc)] : []),
      ...(screenshots.length > 0
        ? [screenshotsSection<Message>(screenshots)]
        : []),
      timeline<Message>(trajectory),
      finalMetricsSection<Message>(trajectory),
      hashScrollScript<Message>(),
    ],
  )
}

// Inline screenshot evidence (#6223). Renders each blob-served image in a
// compact framed strip. Lazy-loaded; a broken/absent endpoint degrades to an
// empty frame rather than failing the page.
const screenshotsSection = <Message>(
  shots: ReadonlyArray<{ src: string; caption: string | undefined }>,
): Html => {
  const h = html<Message>()
  return h.section(
    [Ui.className<Message>('mt-12'), h.DataAttribute('component', 'trace-screenshots')],
    [
      sectionHeading<Message>('Screenshots'),
      h.div(
        [
          Ui.className<Message>(
            'mt-5 grid grid-cols-1 gap-4 sm:grid-cols-2',
          ),
        ],
        shots.map(shot =>
          h.figure(
            [Ui.className<Message>('m-0 grid gap-2')],
            [
              h.img([
                h.Src(shot.src),
                h.Attribute('loading', 'lazy'),
                h.Alt(shot.caption ?? 'Trace screenshot'),
                Ui.className<Message>(
                  'w-full border border-[#222] bg-[#010102]',
                ),
              ]),
              ...(shot.caption !== undefined
                ? [
                    h.figcaption(
                      [
                        Ui.className<Message>(
                          `m-0 text-[0.7rem] leading-5 text-white/40 ${mono}`,
                        ),
                      ],
                      [shot.caption],
                    ),
                  ]
                : []),
            ],
          ),
        ),
      ),
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

// The live read-state the page renders from (issue #6209). Mirrors the
// `loggedOut` model's `TraceModel` union structurally so this leaf page does not
// import the model (which would create an import cycle: the model already imports
// `./trace/atif` + `./trace/sample`). The page reads only the discriminant + the
// decoded trajectory; states it does not recognise fall back to the committed
// sample lookup, so the page is robust to model evolution.
export type TraceLoadState =
  | Readonly<{ _tag: 'TraceIdle' }>
  | Readonly<{ _tag: 'TraceLoading'; uuid: string }>
  | Readonly<{
      _tag: 'TraceLoaded'
      uuid: string
      trajectory: Trajectory
      // Public-safe envelope blob refs (#6223): the trace's recording +
      // screenshots in R2. Optional so a stale/older load state without them
      // still renders (the video section just omits).
      blobRefs?: ReadonlyArray<BlobRef>
    }>
  | Readonly<{ _tag: 'TraceNotFound'; uuid: string }>
  | Readonly<{ _tag: 'TraceFailed'; uuid: string; error: string }>

// Resolve the body to render for a given route + optional live load state.
// Precedence: the committed sample uuid always renders the committed sample (a
// clean, network-free fallback for its one known uuid). Otherwise the live load
// state drives the render — loading → skeleton, loaded → article, not-found /
// failed → the honest not-found body. With no load state (e.g. the pure-render
// test harness, or a stale model), fall back to the committed sample lookup so a
// real uuid that has no live state yet still 404s honestly.
const traceBody = <Message>(
  route: TraceRouteLike,
  loadState: TraceLoadState | undefined,
): Html => {
  if (route.uuid === SAMPLE_TRACE_UUID) {
    const sample = lookupTrajectory(route.uuid)
    return sample !== undefined
      ? traceArticle<Message>(sample, route.uuid)
      : notFoundArticle<Message>(route.uuid)
  }

  if (loadState !== undefined && stateMatchesUuid(loadState, route.uuid)) {
    switch (loadState._tag) {
      case 'TraceLoading':
        return skeletonArticle<Message>()
      case 'TraceLoaded':
        return traceArticle<Message>(
          loadState.trajectory,
          route.uuid,
          loadState.blobRefs,
        )
      case 'TraceNotFound':
      case 'TraceFailed':
        return notFoundArticle<Message>(route.uuid)
      case 'TraceIdle':
        break
    }
  }

  // No usable live state for this uuid: fall back to the committed sample lookup
  // (only the sample uuid matches; every other uuid 404s honestly).
  const trajectory = lookupTrajectory(route.uuid)
  return trajectory !== undefined
    ? traceArticle<Message>(trajectory, route.uuid)
    : notFoundArticle<Message>(route.uuid)
}

// A load state is only authoritative for the route it was loaded for. A state
// carrying a different uuid is stale (fast navigation) and is ignored.
const stateMatchesUuid = (
  loadState: TraceLoadState,
  uuid: string,
): boolean =>
  loadState._tag === 'TraceIdle' ? true : loadState.uuid === uuid

// The page view. Public, no auth. The committed sample uuid renders the
// committed sample; every real uuid renders from the live read state fetched
// from `GET /api/traces/{uuid}` (loading → skeleton, loaded → article,
// 404/not-public/error → the honest not-found body).
export const view = <Message>(
  route: TraceRouteLike,
  authState: PublicHeaderAuthState<Message>,
  loadState?: TraceLoadState,
): Html => {
  const h = html<Message>()
  return h.div(
    [Ui.className<Message>(pageShellClass)],
    [PublicHeader.view(authState), traceBody<Message>(route, loadState)],
  )
}

// The document title for a trace, used by view.ts. Keep it shareable-friendly.
// The committed sample resolves synchronously; a live-loaded trajectory resolves
// from the load state; anything else (loading, not-found, no state) uses the
// generic title so the tab never claims a verdict it cannot yet prove.
export const title = (
  route: TraceRouteLike,
  loadState?: TraceLoadState,
): string => {
  const trajectory =
    route.uuid === SAMPLE_TRACE_UUID
      ? lookupTrajectory(route.uuid)
      : loadState !== undefined &&
          loadState._tag === 'TraceLoaded' &&
          loadState.uuid === route.uuid
        ? loadState.trajectory
        : undefined

  if (trajectory === undefined) {
    if (
      loadState !== undefined &&
      (loadState._tag === 'TraceNotFound' || loadState._tag === 'TraceFailed') &&
      loadState.uuid === route.uuid
    ) {
      return 'Trace not found - OpenAgents'
    }
    // Loading or unknown: a neutral, honest title.
    return route.uuid === SAMPLE_TRACE_UUID
      ? 'Trace not found - OpenAgents'
      : 'Trace - OpenAgents'
  }

  const verdict = verdictLabel(traceVerdict(trajectory))
  const target = traceTarget(trajectory)
  return target !== undefined
    ? `Trace: ${target.name} (${verdict}) - OpenAgents`
    : `Trace (${verdict}) - OpenAgents`
}
