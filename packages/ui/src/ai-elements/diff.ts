import { clsx } from 'clsx'
import { Schema } from 'effect'
import type { Attribute, Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import { aiElementBase } from './base'
import { codeBlockCopyButton } from './code-block'
import { codeTokenClass, tokenizeCode } from './code-highlight'

const MODULE_ID = 'diff'

// A unified-diff surface, structurally a sibling of the code block (DESIGN.md
// Protoss glow frame) but coloured for change: GitHub/Zed/VSCode-style subtle
// green/red full-line tints with an old|new line-number gutter and a +/- sign,
// plus per-line syntax highlighting reused from the code-block tokenizer.

export const diffClass = clsx(
  'group grid w-full overflow-hidden rounded-lg border border-[#1d2a44]',
  'bg-[#05080e] text-[#d7e2f0]',
  'shadow-[0_0_28px_-10px_rgba(58,123,255,0.5)]',
)
export const diffHeaderClass =
  'flex items-center justify-between gap-2 border-b border-[#16233b] bg-[#0a111d] px-3 py-2'
export const diffFilenameClass = 'min-w-0 truncate font-mono text-[0.75rem] text-[#aecbff]'
export const diffStatAddClass = 'font-mono text-[0.75rem] tabular-nums text-[#3fb950]'
export const diffStatRemoveClass = 'font-mono text-[0.75rem] tabular-nums text-[#f85149]'
export const diffBodyClass =
  'm-0 overflow-x-auto py-0 font-mono text-[0.8125rem] leading-[1.5] text-[#d7e2f0]'
export const diffHunkClass =
  'block whitespace-pre bg-[#0b1626] px-3 py-0.5 text-[0.75rem] text-[#5f86c2]'
export const diffLineClass = 'flex min-h-[1.5em]'
export const diffGutterClass =
  'w-9 shrink-0 select-none px-1 text-right tabular-nums text-[#34507f]'
export const diffSignClass = 'w-4 shrink-0 select-none text-center'
export const diffCodeClass = 'min-w-0 flex-1 whitespace-pre pr-3'

export const DiffProps = Schema.Struct({
  patch: Schema.String,
  language: Schema.optional(Schema.String),
  filename: Schema.optional(Schema.String),
})
export type DiffProps = typeof DiffProps.Type

type DiffRowKind = 'hunk' | 'add' | 'remove' | 'context'

type DiffRow = {
  readonly kind: DiffRowKind
  readonly oldNo?: number
  readonly newNo?: number
  readonly text: string
}

type ParsedDiff = {
  readonly rows: ReadonlyArray<DiffRow>
  readonly added: number
  readonly removed: number
  readonly filename?: string
}

const stripPrefix = (path: string): string =>
  path.startsWith('a/') || path.startsWith('b/') ? path.slice(2) : path

// Parse a unified diff (git patch) into render rows. Header noise
// (`diff --git`, `index`, `---`, `+++`) is consumed for the filename but not
// rendered, leaving a clean hunk/line view.
export const parseUnifiedDiff = (
  patch: string,
  filenameOverride?: string,
): ParsedDiff => {
  const lines = patch.split('\n')
  const rows: DiffRow[] = []
  let oldNo = 0
  let newNo = 0
  let added = 0
  let removed = 0
  let filename = filenameOverride

  lines.forEach((line, index) => {
    // Drop a single trailing empty line produced by a trailing newline.
    if (line === '' && index === lines.length - 1) {
      return
    }

    if (line.startsWith('@@')) {
      const match = line.match(/@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@/)
      if (match !== null) {
        oldNo = Number(match[1])
        newNo = Number(match[2])
      }
      rows.push({ kind: 'hunk', text: line })
      return
    }

    if (
      line.startsWith('diff ') ||
      line.startsWith('index ') ||
      line.startsWith('new file') ||
      line.startsWith('deleted file') ||
      line.startsWith('similarity ') ||
      line.startsWith('rename ') ||
      line.startsWith('\\')
    ) {
      return
    }

    if (line.startsWith('+++ ')) {
      const path = line.slice(4).trim()
      if (filename === undefined && path !== '/dev/null' && path.length > 0) {
        filename = stripPrefix(path)
      }
      return
    }
    if (line.startsWith('--- ')) {
      const path = line.slice(4).trim()
      if (filename === undefined && path !== '/dev/null' && path.length > 0) {
        filename = stripPrefix(path)
      }
      return
    }

    const sign = line.charAt(0)
    if (sign === '+') {
      added += 1
      rows.push({ kind: 'add', newNo, text: line.slice(1) })
      newNo += 1
      return
    }
    if (sign === '-') {
      removed += 1
      rows.push({ kind: 'remove', oldNo, text: line.slice(1) })
      oldNo += 1
      return
    }

    const text = line.startsWith(' ') ? line.slice(1) : line
    rows.push({ kind: 'context', oldNo, newNo, text })
    oldNo += 1
    newNo += 1
  })

  return {
    rows,
    added,
    removed,
    ...(filename === undefined ? {} : { filename }),
  }
}

const lineBgClass = (kind: DiffRowKind): string => {
  if (kind === 'add') return 'bg-[#0f2a18]'
  if (kind === 'remove') return 'bg-[#2a1417]'
  return ''
}

const signFor = (kind: DiffRowKind): string => {
  if (kind === 'add') return '+'
  if (kind === 'remove') return '-'
  return ' '
}

const signClassFor = (kind: DiffRowKind): string => {
  if (kind === 'add') return clsx(diffSignClass, 'text-[#3fb950]')
  if (kind === 'remove') return clsx(diffSignClass, 'text-[#f85149]')
  return clsx(diffSignClass, 'text-transparent')
}

export const diffHeader = <Message>(input: {
  filename?: string
  added: number
  removed: number
  actions?: ReadonlyArray<Html>
}): Html => {
  const h = html<Message>()

  return h.div(
    [aiElementBase<Message>(MODULE_ID, 'DiffHeader'), h.Class(diffHeaderClass)],
    [
      h.div(
        [h.Class('flex min-w-0 items-center gap-3')],
        [
          input.filename === undefined
            ? null
            : h.span([h.Class(diffFilenameClass)], [input.filename]),
          h.div(
            [h.Class('flex shrink-0 items-center gap-2')],
            [
              h.span([h.Class(diffStatAddClass)], [`+${input.added}`]),
              h.span([h.Class(diffStatRemoveClass)], [`−${input.removed}`]),
            ],
          ),
        ],
      ),
      input.actions === undefined
        ? null
        : h.div([h.Class('flex items-center gap-1')], input.actions),
    ],
  )
}

const diffRowView = <Message>(
  row: DiffRow,
  language: string | undefined,
  showLineNumbers: boolean,
): Html => {
  const h = html<Message>()

  if (row.kind === 'hunk') {
    return h.span(
      [aiElementBase<Message>(MODULE_ID, 'DiffHunk'), h.Class(diffHunkClass)],
      [row.text],
    )
  }

  const tokens = tokenizeCode(row.text, language)
  const codeSpan = h.span(
    [h.Class(diffCodeClass)],
    tokens.map(token =>
      h.span([h.Class(codeTokenClass(token.kind))], [token.text]),
    ),
  )

  return h.span(
    [
      aiElementBase<Message>(MODULE_ID, 'DiffLine'),
      h.DataAttribute('diff-line', row.kind),
      h.Class(clsx(diffLineClass, lineBgClass(row.kind))),
    ],
    [
      ...(showLineNumbers
        ? [
            h.span(
              [h.AriaHidden(true), h.Class(diffGutterClass)],
              [row.oldNo === undefined ? '' : String(row.oldNo)],
            ),
            h.span(
              [h.AriaHidden(true), h.Class(diffGutterClass)],
              [row.newNo === undefined ? '' : String(row.newNo)],
            ),
          ]
        : []),
      h.span(
        [h.AriaHidden(true), h.Class(signClassFor(row.kind))],
        [signFor(row.kind)],
      ),
      codeSpan,
    ],
  )
}

// A framed unified-diff surface with a filename + add/remove stats header, a
// copy button (copies the original patch), an old|new line-number gutter, and
// per-line syntax highlighting. `copy` and line numbers are on by default.
export const diff = <Message>(input: {
  props: DiffProps
  showLineNumbers?: boolean
  copy?: boolean
  attrs?: ReadonlyArray<Attribute<Message>>
}): Html => {
  const h = html<Message>()
  const props = Schema.decodeUnknownSync(DiffProps)(input.props)
  const parsed = parseUnifiedDiff(props.patch, props.filename)
  const showLineNumbers = input.showLineNumbers ?? true
  const showCopy = input.copy ?? true

  return h.div(
    [
      ...(input.attrs ?? []),
      aiElementBase<Message>(MODULE_ID, 'Diff'),
      h.DataAttribute('oa-code-block', ''),
      h.Class(diffClass),
    ],
    [
      diffHeader<Message>({
        ...(parsed.filename === undefined ? {} : { filename: parsed.filename }),
        added: parsed.added,
        removed: parsed.removed,
        ...(showCopy ? { actions: [codeBlockCopyButton<Message>()] } : {}),
      }),
      h.pre(
        [h.Class(diffBodyClass)],
        [
          h.span(
            [h.Class('hidden'), h.DataAttribute('oa-code-source', '')],
            [props.patch],
          ),
          h.code(
            [h.Class('grid')],
            parsed.rows.map(row =>
              diffRowView<Message>(
                row,
                props.language,
                showLineNumbers,
              ),
            ),
          ),
        ],
      ),
    ],
  )
}
