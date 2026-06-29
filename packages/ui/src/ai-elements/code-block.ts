import { clsx } from 'clsx'
import { Schema } from 'effect'
import type { Attribute, Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import { iconView } from '../icon'
import { eyebrowClass, metaClass, statusDotClass, titleClass } from '../primitives'
import { aiElementBase } from './base'
import {
  type CodeToken,
  codeTokenClass,
  tokenizeCodeLines,
} from './code-highlight'

const MODULE_ID = 'code-block'

// Ported (patterns, not vendored) from the React/Tailwind AI Elements
// `code-block` and the autopilot3 markup, dressed in the house Protoss
// "glowing blue / white / black" language (DESIGN.md): a sunken blue-black
// surface, a thin energized border with a blue glow halo, cool-blue syntax
// tokens, a filename/language header, an optional line-number gutter, a copy
// button, and an optional run/test result panel (modeled on the Maud
// `test-results` contract: status + summary + duration).

// The framed surface. The blue border + soft outer glow echo the scene's
// energy without re-implementing bloom (DESIGN.md "glow halo").
export const codeBlockClass = clsx(
  'group grid w-full overflow-hidden rounded-lg border border-[#1d2a44]',
  'bg-[#05080e] text-[#d7e2f0]',
  'shadow-[0_0_28px_-10px_rgba(58,123,255,0.5)]',
)
export const codeBlockHeaderClass =
  'flex items-center justify-between gap-2 border-b border-[#16233b] bg-[#0a111d] px-3 py-2 text-white/40'
export const codeBlockFilenameClass = 'font-mono text-[0.75rem] text-[#aecbff]'
export const codeBlockLanguageClass = clsx(eyebrowClass, 'text-[#7aa2ff]')
export const codeBlockBodyClass =
  'm-0 overflow-x-auto py-2.5 font-mono text-[0.8125rem] leading-[1.45] text-[#d7e2f0]'
export const codeBlockLineClass = 'flex min-h-[1.45em] px-3'
export const codeBlockGutterClass =
  'w-9 shrink-0 select-none pr-4 text-right tabular-nums text-[#34507f]'
export const codeBlockLineCodeClass = 'min-w-0 flex-1 whitespace-pre'
export const codeBlockCopyButtonClass = clsx(
  'inline-flex items-center gap-1.5 rounded-md border border-[#1d2a44]',
  'bg-[#0b1322]/70 px-2 py-1 font-mono text-[0.6875rem] uppercase tracking-[0.08em]',
  'text-[#8fb6ff] transition-colors duration-200 ease-out',
  'hover:border-[#3a7bff]/55 hover:text-white hover:shadow-[0_0_18px_-6px_rgba(58,123,255,0.55)]',
  'focus-visible:outline focus-visible:outline-2 focus-visible:outline-[#3a7bff] focus-visible:outline-offset-2',
  'cursor-pointer',
)
export const codeBlockRunResultClass =
  'grid gap-1 border-t border-[#16233b] bg-[#0a111d] px-3 py-2.5'
export const codeBlockRunHeaderClass = 'flex items-center gap-2'
export const codeBlockRunOutputClass =
  'm-0 overflow-x-auto font-mono text-[0.75rem] leading-[1.45] text-white/60'

export const RunStatus = Schema.Literals([
  'pending',
  'running',
  'passed',
  'failed',
])
export type RunStatus = typeof RunStatus.Type

export const CodeBlockProps = Schema.Struct({
  code: Schema.String,
  language: Schema.optional(Schema.String),
  filename: Schema.optional(Schema.String),
})
export type CodeBlockProps = typeof CodeBlockProps.Type

export const CodeBlockRunResultProps = Schema.Struct({
  status: RunStatus,
  summary: Schema.optional(Schema.String),
  duration: Schema.optional(Schema.String),
  output: Schema.optional(Schema.String),
})
export type CodeBlockRunResultProps = typeof CodeBlockRunResultProps.Type

const runStatusTone = (status: RunStatus) => {
  if (status === 'passed') {
    return 'positive' as const
  }

  if (status === 'failed') {
    return 'negative' as const
  }

  if (status === 'running') {
    return 'info' as const
  }

  return 'neutral' as const
}

const runStatusLabel = (status: RunStatus): string => {
  switch (status) {
    case 'pending':
      return 'Pending'
    case 'running':
      return 'Running'
    case 'passed':
      return 'Passed'
    case 'failed':
      return 'Failed'
  }
}

// A self-describing copy affordance. The button is static markup: it carries a
// `data-oa-code-copy` hook and the host activates it (the web app ships an
// `oa-code-copy-scope` controller that copies the sibling
// `[data-oa-code-source]` text and flips the label to "Copied"). Keeping the
// behavior in the host keeps this library component pure and SSR/test-safe.
export const codeBlockCopyButton = <Message>(): Html => {
  const h = html<Message>()

  return h.button(
    [
      aiElementBase<Message>(MODULE_ID, 'CodeBlockCopyButton'),
      h.Type('button'),
      h.DataAttribute('oa-code-copy', ''),
      h.AriaLabel('Copy code'),
      h.Class(codeBlockCopyButtonClass),
    ],
    [
      h.span(
        [h.DataAttribute('oa-code-copy-icon', 'copy'), h.Class('inline-flex')],
        [iconView<Message>('Copy', 'size-3.5 text-current')],
      ),
      h.span(
        [h.DataAttribute('oa-code-copy-icon', 'check'), h.Class('hidden')],
        [iconView<Message>('Check', 'size-3.5 text-[#4fd0ff]')],
      ),
      h.span([h.DataAttribute('oa-code-copy-label', '')], ['Copy']),
    ],
  )
}

export const codeBlockHeader = <Message>(input: {
  filename?: string
  language?: string
  actions?: ReadonlyArray<Html>
}): Html => {
  const h = html<Message>()

  return h.div(
    [
      aiElementBase<Message>(MODULE_ID, 'CodeBlockHeader'),
      h.Class(codeBlockHeaderClass),
    ],
    [
      h.div(
        [h.Class('flex min-w-0 items-center gap-2')],
        [
          input.filename === undefined
            ? null
            : h.span([h.Class(codeBlockFilenameClass)], [input.filename]),
          input.language === undefined
            ? null
            : h.span([h.Class(codeBlockLanguageClass)], [input.language]),
        ],
      ),
      input.actions === undefined
        ? null
        : h.div([h.Class('flex items-center gap-1')], input.actions),
    ],
  )
}

const codeLineView = <Message>(
  tokens: ReadonlyArray<CodeToken>,
  index: number,
  showLineNumbers: boolean,
): Html => {
  const h = html<Message>()

  const codeSpan = h.span(
    [h.Class(codeBlockLineCodeClass)],
    tokens.map(token =>
      h.span([h.Class(codeTokenClass(token.kind))], [token.text]),
    ),
  )

  return h.span(
    [h.Class(codeBlockLineClass)],
    showLineNumbers
      ? [
          h.span(
            [h.AriaHidden(true), h.Class(codeBlockGutterClass)],
            [String(index + 1)],
          ),
          codeSpan,
        ]
      : [codeSpan],
  )
}

// The syntax-highlighted body. Source is tokenized to typed spans (never
// `innerHTML`) and rendered line-by-line. A hidden, byte-faithful copy of the
// original source rides along under `data-oa-code-source` so the copy
// controller always yields pristine text regardless of highlighting,
// line-number gutters, or soft wrapping.
export const codeBlockBody = <Message>(input: {
  code: string
  language?: string
  showLineNumbers?: boolean
}): Html => {
  const h = html<Message>()
  const lines = tokenizeCodeLines(input.code, input.language)
  const showLineNumbers = input.showLineNumbers ?? false

  return h.pre(
    [
      aiElementBase<Message>(MODULE_ID, 'CodeBlockBody'),
      h.Class(codeBlockBodyClass),
    ],
    [
      h.span(
        [h.Class('hidden'), h.DataAttribute('oa-code-source', '')],
        [input.code],
      ),
      h.code(
        [h.Class('grid')],
        lines.map((tokens, index) =>
          codeLineView<Message>(tokens, index, showLineNumbers),
        ),
      ),
    ],
  )
}

// Run / test result panel. Renders a status dot + label, an optional summary
// and duration, and optional raw output.
export const codeBlockRunResult = <Message>(
  props: CodeBlockRunResultProps,
): Html => {
  const h = html<Message>()
  const decoded = Schema.decodeUnknownSync(CodeBlockRunResultProps)(props)

  return h.div(
    [
      aiElementBase<Message>(MODULE_ID, 'CodeBlockRunResult'),
      h.Class(codeBlockRunResultClass),
    ],
    [
      h.div(
        [h.Class(codeBlockRunHeaderClass)],
        [
          h.span([h.Class(statusDotClass(runStatusTone(decoded.status)))], []),
          h.span([h.Class(titleClass)], [runStatusLabel(decoded.status)]),
          decoded.duration === undefined
            ? null
            : h.span([h.Class(metaClass)], [decoded.duration]),
        ],
      ),
      decoded.summary === undefined
        ? null
        : h.p([h.Class(metaClass)], [decoded.summary]),
      decoded.output === undefined
        ? null
        : h.pre([h.Class(codeBlockRunOutputClass)], [decoded.output]),
    ],
  )
}

// A framed, syntax-highlighted code surface with an optional filename/language
// header, a copy button, optional line numbers, and an optional run/test result
// panel. `copy` and the syntax highlighting are on by default; the header
// appears whenever there is a filename, language, copy button, or caller action
// to show.
export const codeBlock = <Message>(input: {
  props: CodeBlockProps
  result?: CodeBlockRunResultProps
  headerActions?: ReadonlyArray<Html>
  showLineNumbers?: boolean
  copy?: boolean
  attrs?: ReadonlyArray<Attribute<Message>>
}): Html => {
  const h = html<Message>()
  const props = Schema.decodeUnknownSync(CodeBlockProps)(input.props)
  const showCopy = input.copy ?? true

  const actions: ReadonlyArray<Html> = [
    ...(input.headerActions ?? []),
    ...(showCopy ? [codeBlockCopyButton<Message>()] : []),
  ]
  const showHeader =
    props.filename !== undefined ||
    props.language !== undefined ||
    actions.length > 0

  return h.div(
    [
      ...(input.attrs ?? []),
      aiElementBase<Message>(MODULE_ID, 'CodeBlock'),
      h.DataAttribute('oa-code-block', ''),
      h.Class(codeBlockClass),
    ],
    [
      showHeader
        ? codeBlockHeader<Message>({
            ...(props.filename === undefined ? {} : { filename: props.filename }),
            ...(props.language === undefined ? {} : { language: props.language }),
            ...(actions.length > 0 ? { actions } : {}),
          })
        : null,
      codeBlockBody<Message>({
        code: props.code,
        ...(props.language === undefined ? {} : { language: props.language }),
        ...(input.showLineNumbers === undefined
          ? {}
          : { showLineNumbers: input.showLineNumbers }),
      }),
      input.result === undefined
        ? null
        : codeBlockRunResult<Message>(input.result),
    ],
  )
}
