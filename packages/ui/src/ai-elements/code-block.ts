import { clsx } from 'clsx'
import { Schema } from 'effect'
import type { Attribute, Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import { eyebrowClass, metaClass, statusDotClass, titleClass } from '../primitives'
import { aiElementBase } from './base'

const MODULE_ID = 'code-block'

// Ported from the autopilot3 code-block markup + Maud `AI_CODE_BLOCK_*`
// contracts, in the kit's dark-only palette. The run/test result panel is
// modeled on the Maud `test-results` contract (status + summary + duration).
export const codeBlockClass =
  'group grid w-full overflow-hidden border border-[#222] bg-[#010102] text-[#f1efe8]'
export const codeBlockHeaderClass =
  'flex items-center justify-between gap-2 border-b border-[#222] bg-[#030303] px-3 py-2 text-white/35'
export const codeBlockFilenameClass = 'font-mono text-[0.75rem] text-white/60'
export const codeBlockLanguageClass = clsx(
  eyebrowClass,
  'text-white/35',
)
export const codeBlockBodyClass =
  'm-0 overflow-x-auto px-3 py-2.5 font-mono text-[0.8125rem] leading-[1.45] text-[#f1efe8]'
export const codeBlockRunResultClass =
  'grid gap-1 border-t border-[#222] bg-[#030303] px-3 py-2.5'
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

// A framed code surface with an optional filename/language header and an
// optional run/test result panel.
export const codeBlock = <Message>(input: {
  props: CodeBlockProps
  result?: CodeBlockRunResultProps
  headerActions?: ReadonlyArray<Html>
  attrs?: ReadonlyArray<Attribute<Message>>
}): Html => {
  const h = html<Message>()
  const props = Schema.decodeUnknownSync(CodeBlockProps)(input.props)
  const showHeader =
    props.filename !== undefined ||
    props.language !== undefined ||
    input.headerActions !== undefined

  return h.div(
    [
      ...(input.attrs ?? []),
      aiElementBase<Message>(MODULE_ID, 'CodeBlock'),
      h.Class(codeBlockClass),
    ],
    [
      showHeader
        ? codeBlockHeader<Message>({
            ...(props.filename === undefined ? {} : { filename: props.filename }),
            ...(props.language === undefined ? {} : { language: props.language }),
            ...(input.headerActions === undefined
              ? {}
              : { actions: input.headerActions }),
          })
        : null,
      h.pre(
        [
          aiElementBase<Message>(MODULE_ID, 'CodeBlockBody'),
          h.Class(codeBlockBodyClass),
        ],
        [h.code([], [props.code])],
      ),
      input.result === undefined
        ? null
        : codeBlockRunResult<Message>(input.result),
    ],
  )
}
