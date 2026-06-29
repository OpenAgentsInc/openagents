import type { Attribute } from 'foldkit/html'
import { html } from 'foldkit/html'

// AI Elements base-contract registry.
//
// Mirrors the Maud curation map in
// `autopilot4-deprecated/src/ui_components/{base.rs,ai_elements.rs}`: every
// AI-element primitive records the design-system primitive it derives from via
// an `ai-elements:<module>/<Primitive>` tag, emitted as a `data-ui-base`
// attribute so trusted, typed components stay auditable in the DOM.
//
// The catalog below is the *curation spec* for the subset of modules this
// package implements (issue #5083 priority order). The coverage test asserts
// the implemented module list and per-module primitive counts against it,
// mirroring the v4 family-coverage test style.

export type AiElementCategory =
  | 'agent-runtime'
  | 'chat'
  | 'code'
  | 'data'
  | 'workflow'

export type AiElementPort = Readonly<{
  moduleId: string
  label: string
  category: AiElementCategory
  purpose: string
  primitives: ReadonlyArray<string>
}>

export const aiElementBaseTag = (moduleId: string, primitive: string): string =>
  `ai-elements:${moduleId}/${primitive}`

// A `data-ui-base` attribute that records which AI Elements primitive a piece
// of trusted markup derives from. Use the named module barrel constants (e.g.
// `PROMPT_INPUT_PORT.moduleId`) so the tag stays in sync with the catalog.
export const aiElementBase = <Message>(
  moduleId: string,
  primitive: string,
): Attribute<Message> =>
  html<Message>().DataAttribute('ui-base', aiElementBaseTag(moduleId, primitive))

// Curation spec — the module + primitive catalog this package ships. Keep this
// list in lockstep with the per-component primitive constants; the coverage
// test fails closed if they drift.
export const aiElementPorts: ReadonlyArray<AiElementPort> = [
  {
    moduleId: 'prompt-input',
    label: 'Prompt Input',
    category: 'chat',
    purpose:
      'Compose prompts with a body textarea, tools row, attachments affordance, and submit state.',
    primitives: [
      'PromptInput',
      'PromptInputBody',
      'PromptInputTextarea',
      'PromptInputFooter',
      'PromptInputTools',
      'PromptInputButton',
      'PromptInputSubmit',
    ],
  },
  {
    moduleId: 'message',
    label: 'Message',
    category: 'chat',
    purpose:
      'Render user/assistant turns with roles, content, timestamps, and an actions row.',
    primitives: ['Message', 'MessageContent', 'MessageActions', 'MessageMeta'],
  },
  {
    moduleId: 'response',
    label: 'Response',
    category: 'chat',
    purpose:
      'Render assistant Markdown prose (bold, italics, headings, lists, inline/fenced code, links, blockquotes, rules) as typed Foldkit nodes, tolerating incomplete/streaming markdown, with a streaming cursor affordance.',
    primitives: ['Response', 'ResponseCode', 'ResponseCursor'],
  },
  {
    moduleId: 'code-block',
    label: 'Code Block',
    category: 'code',
    purpose:
      'Frame a syntax-highlighted code surface with a filename/language header, a copy button, optional line numbers, and an optional run/test result panel.',
    primitives: [
      'CodeBlock',
      'CodeBlockHeader',
      'CodeBlockBody',
      'CodeBlockRunResult',
      'CodeBlockCopyButton',
    ],
  },
  {
    moduleId: 'diff',
    label: 'Diff',
    category: 'code',
    purpose:
      'Render a unified diff as a framed surface with a filename + add/remove stats header, an old|new line-number gutter, +/- signs, subtle green/red line tints, and per-line syntax highlighting.',
    primitives: ['Diff', 'DiffHeader', 'DiffHunk', 'DiffLine'],
  },
  {
    moduleId: 'task',
    label: 'Task',
    category: 'workflow',
    purpose:
      'Show a task list with a trigger, content, ordered items, and file references.',
    primitives: ['Task', 'TaskTrigger', 'TaskContent', 'TaskItem', 'TaskItemFile'],
  },
  {
    moduleId: 'sources',
    label: 'Sources',
    category: 'data',
    purpose:
      'Collapse and list the source links / provenance used by an answer (ties to receipts).',
    primitives: ['Sources', 'SourcesTrigger', 'SourcesContent', 'Source'],
  },
  {
    moduleId: 'tool',
    label: 'Tool',
    category: 'agent-runtime',
    purpose:
      'Display tool/agent state, input parameters, output, and error text with a status badge.',
    primitives: ['Tool', 'ToolHeader', 'ToolContent', 'ToolInput', 'ToolOutput'],
  },
  {
    moduleId: 'confirmation',
    label: 'Confirmation',
    category: 'agent-runtime',
    purpose:
      'Human-in-the-loop approval gate with a request title, accept/reject actions, and resolved state.',
    primitives: [
      'Confirmation',
      'ConfirmationTitle',
      'ConfirmationActions',
      'ConfirmationAction',
    ],
  },
  {
    moduleId: 'reasoning',
    label: 'Reasoning',
    category: 'agent-runtime',
    purpose:
      'Expose a bounded, collapsible reasoning summary with streaming duration and content.',
    primitives: ['Reasoning', 'ReasoningTrigger', 'ReasoningContent'],
  },
  {
    moduleId: 'web-preview',
    label: 'Web Preview',
    category: 'code',
    purpose:
      'Frame a preview URL, navigation controls, an iframe body, and a console log panel.',
    primitives: [
      'WebPreview',
      'WebPreviewNavigation',
      'WebPreviewUrl',
      'WebPreviewBody',
      'WebPreviewConsole',
    ],
  },
] as const

export const aiElementModuleCount = (): number => aiElementPorts.length

export const aiElementPrimitiveCount = (): number =>
  aiElementPorts.reduce((total, port) => total + port.primitives.length, 0)
