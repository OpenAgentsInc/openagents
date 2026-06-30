import {
  emptyComposerState,
  serializeComposerMarkdown,
  type ComposerState,
} from '@openagentsinc/composer-state'
import { Schema } from 'effect'
import type { Attribute, Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import {
  classAttrs,
  componentClass,
} from '../class-foldkit'
import { aiElementBase } from './base'
import { response } from './response'

const MODULE_ID = 'command-composer'

export const commandComposerClass = componentClass('oa-ai-command-composer')
export const commandComposerFrameClass = componentClass(
  'oa-ai-command-composer-frame',
)
export const commandComposerTextareaClass = componentClass(
  'oa-ai-command-composer-textarea',
)
export const commandComposerRailClass = componentClass(
  'oa-ai-command-composer-rail',
)
export const commandComposerAttachmentClass = componentClass(
  'oa-ai-command-composer-attachment',
)
export const commandComposerStatusClass = componentClass(
  'oa-ai-command-composer-status',
)
export const commandComposerControlsClass = componentClass(
  'oa-ai-command-composer-controls',
)
export const commandComposerButtonClass = componentClass(
  'oa-ai-command-composer-button',
)
export const commandComposerSubmitClass = componentClass(
  'oa-ai-command-composer-submit',
)
export const commandComposerResizeHandleClass = componentClass(
  'oa-ai-command-composer-resize-handle',
)
export const commandComposerA11yClass = componentClass(
  'oa-ai-command-composer-a11y',
)
export const commandComposerMarkdownPreviewClass = componentClass(
  'oa-ai-command-composer-markdown-preview',
)
export const commandComposerAttachmentActionClass = componentClass(
  'oa-ai-command-composer-attachment-action',
)
export const commandComposerDropcursorClass = componentClass(
  'oa-ai-command-composer-dropcursor',
)

const commandComposerStyles = {
  root: commandComposerClass,
  frame: commandComposerFrameClass,
  textarea: commandComposerTextareaClass,
  rail: commandComposerRailClass,
  attachment: commandComposerAttachmentClass,
  status: commandComposerStatusClass,
  controls: commandComposerControlsClass,
  button: commandComposerButtonClass,
  submit: commandComposerSubmitClass,
  resizeHandle: commandComposerResizeHandleClass,
  a11y: commandComposerA11yClass,
  markdownPreview: commandComposerMarkdownPreviewClass,
  attachmentAction: commandComposerAttachmentActionClass,
  dropcursor: commandComposerDropcursorClass,
}

export const CommandComposerStatus = Schema.Literals([
  'ready',
  'submitted',
  'streaming',
  'error',
])
export type CommandComposerStatus = typeof CommandComposerStatus.Type

export const CommandComposerAttachmentKind = Schema.Literals([
  'image',
  'file',
  'text',
  'snippet',
])
export type CommandComposerAttachmentKind =
  typeof CommandComposerAttachmentKind.Type

export const CommandComposerAttachmentStatus = Schema.Literals([
  'staged',
  'uploading',
  'ready',
  'error',
])
export type CommandComposerAttachmentStatus =
  typeof CommandComposerAttachmentStatus.Type

export const CommandComposerAttachmentDimensions = Schema.Struct({
  width: Schema.Number,
  height: Schema.Number,
})
export type CommandComposerAttachmentDimensions =
  typeof CommandComposerAttachmentDimensions.Type

export const CommandComposerAttachmentProps = Schema.Struct({
  id: Schema.String,
  kind: CommandComposerAttachmentKind,
  name: Schema.String,
  mime: Schema.String,
  sizeBytes: Schema.Number,
  sizeLabel: Schema.optional(Schema.String),
  dimensions: Schema.optional(CommandComposerAttachmentDimensions),
  status: CommandComposerAttachmentStatus,
  previewUrl: Schema.optional(Schema.String),
  contentRef: Schema.optional(Schema.String),
  source: Schema.optional(Schema.Literals(['paste', 'drop', 'manual'])),
  errorText: Schema.optional(Schema.String),
})
export type CommandComposerAttachmentProps =
  typeof CommandComposerAttachmentProps.Type

export const CommandComposerProps = Schema.Struct({
  name: Schema.String,
  label: Schema.optional(Schema.String),
  placeholder: Schema.optional(Schema.String),
  value: Schema.optional(Schema.String),
  status: Schema.optional(CommandComposerStatus),
  submitLabel: Schema.optional(Schema.String),
  stopLabel: Schema.optional(Schema.String),
  rows: Schema.optional(Schema.Number),
  autofocus: Schema.optional(Schema.Boolean),
  preview: Schema.optional(Schema.Boolean),
  expanded: Schema.optional(Schema.Boolean),
  heightPx: Schema.optional(Schema.Number),
  tokenEstimate: Schema.optional(Schema.Number),
  sizeLabel: Schema.optional(Schema.String),
  keymapLabel: Schema.optional(Schema.String),
  selectedAttachmentId: Schema.optional(Schema.String),
  dragActive: Schema.optional(Schema.Boolean),
})
export type CommandComposerProps = typeof CommandComposerProps.Type

export type CommandComposerButtonVariant = 'ghost' | 'primary' | 'danger'
export type CommandComposerAttachmentAction = 'preview' | 'retry' | 'remove'
export type CommandComposerAttachmentActionAttrs<Message> = (
  attachment: CommandComposerAttachmentProps,
  action: CommandComposerAttachmentAction,
) => ReadonlyArray<Attribute<Message>>

export type CommandComposerIconName =
  | 'attach'
  | 'code'
  | 'compact'
  | 'expand'
  | 'file'
  | 'image'
  | 'preview'
  | 'preview-off'
  | 'resize'
  | 'send'
  | 'stop'
  | 'text'

const decodeProps = Schema.decodeUnknownSync(CommandComposerProps)
const decodeAttachment = Schema.decodeUnknownSync(CommandComposerAttachmentProps)

const idSafe = (name: string): string =>
  name.replace(/[^a-zA-Z0-9_-]/g, '-').replace(/^-+|-+$/g, '') || 'prompt'

const formatBytes = (bytes: number): string => {
  if (!Number.isFinite(bytes) || bytes <= 0) {
    return '0 B'
  }

  const units = ['B', 'KB', 'MB', 'GB'] as const
  let value = bytes
  let unit = 0
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024
    unit += 1
  }

  const digits = value >= 10 || unit === 0 ? 0 : 1
  return `${value.toFixed(digits)} ${units[unit]}`
}

const statusLabelFor = (status: CommandComposerStatus): string => {
  switch (status) {
    case 'submitted':
      return 'Submitted'
    case 'streaming':
      return 'Streaming'
    case 'error':
      return 'Needs attention'
    case 'ready':
      return 'Ready'
  }
}

const submitLabelFor = (
  status: CommandComposerStatus,
  props: CommandComposerProps,
): string => {
  if (status === 'submitted' || status === 'streaming') {
    return props.stopLabel ?? 'Stop'
  }

  if (status === 'error') {
    return props.submitLabel ?? 'Retry'
  }

  return props.submitLabel ?? 'Send'
}

const submitIconFor = (status: CommandComposerStatus): CommandComposerIconName =>
  status === 'submitted' || status === 'streaming' ? 'stop' : 'send'

const glyphForIcon = (icon: CommandComposerIconName): string => {
  switch (icon) {
    case 'attach':
      return '+'
    case 'code':
      return '{}'
    case 'compact':
      return '-'
    case 'expand':
      return '[]'
    case 'file':
      return '#'
    case 'image':
      return '[]'
    case 'preview':
      return 'o'
    case 'preview-off':
      return '/'
    case 'resize':
      return '/'
    case 'send':
      return '^'
    case 'stop':
      return 'x'
    case 'text':
      return 'T'
  }
}

const commandComposerIcon = <Message>(icon: CommandComposerIconName): Html => {
  const h = html<Message>()

  return h.span(
    [
      h.AriaHidden(true),
      h.DataAttribute('oa-command-composer-icon', icon),
      h.Class('oa-ai-command-composer-icon'),
    ],
    [glyphForIcon(icon)],
  )
}

const textareaValueFor = (
  props: CommandComposerProps,
  state: ComposerState,
): string => props.value ?? serializeComposerMarkdown(state.doc)

const normalizeAttachment = (
  attachment: CommandComposerAttachmentProps,
): CommandComposerAttachmentProps => {
  const decoded = decodeAttachment(attachment)
  return {
    ...decoded,
    sizeLabel: decoded.sizeLabel ?? formatBytes(decoded.sizeBytes),
  }
}

const statusTextForAttachment = (
  status: CommandComposerAttachmentStatus,
): string => {
  switch (status) {
    case 'staged':
      return 'Staged'
    case 'uploading':
      return 'Uploading'
    case 'ready':
      return 'Ready'
    case 'error':
      return 'Error'
  }
}

export const commandComposerFrame = <Message>(input: {
  children: ReadonlyArray<Html | string>
  attrs?: ReadonlyArray<Attribute<Message>>
}): Html => {
  const h = html<Message>()

  return h.div(
    [
      ...(input.attrs ?? []),
      aiElementBase<Message>(MODULE_ID, 'CommandComposerFrame'),
      ...classAttrs<Message>(commandComposerStyles.frame),
    ],
    input.children,
  )
}

export const commandComposerButton = <Message>(input: {
  label: string
  icon?: CommandComposerIconName
  variant?: CommandComposerButtonVariant
  pressed?: boolean
  disabled?: boolean
  attrs?: ReadonlyArray<Attribute<Message>>
}): Html => {
  const h = html<Message>()
  const variant = input.variant ?? 'ghost'

  return h.button(
    [
      ...(input.attrs ?? []),
      aiElementBase<Message>(MODULE_ID, 'CommandComposerButton'),
      h.Type('button'),
      h.AriaLabel(input.label),
      h.Title(input.label),
      h.DataAttribute('oa-command-composer-control', input.label),
      h.DataAttribute('variant', variant),
      ...(input.pressed === undefined
        ? []
        : [h.AriaPressed(input.pressed ? 'true' : 'false')]),
      ...(input.disabled === true ? [h.Disabled(true)] : []),
      ...classAttrs<Message>(commandComposerStyles.button),
    ],
    [
      ...(input.icon === undefined
        ? []
        : [commandComposerIcon<Message>(input.icon)]),
      h.span(
        [h.Class('oa-ai-command-composer-button-label')],
        [input.label],
      ),
    ],
  )
}

export const commandComposerAttachmentAction = <Message>(input: {
  attachment: CommandComposerAttachmentProps
  action: CommandComposerAttachmentAction
  attrs?: ReadonlyArray<Attribute<Message>>
}): Html => {
  const h = html<Message>()
  const label =
    input.action === 'preview'
      ? input.attachment.previewUrl === undefined
        ? 'Open attachment'
        : 'Preview attachment'
      : input.action === 'retry'
        ? 'Retry attachment'
        : 'Remove attachment'

  return h.button(
    [
      ...(input.attrs ?? []),
      aiElementBase<Message>(MODULE_ID, 'CommandComposerAttachmentAction'),
      h.Type('button'),
      h.AriaLabel(`${label}: ${input.attachment.name}`),
      h.Title(label),
      h.DataAttribute('oa-command-composer-attachment-action', input.action),
      h.DataAttribute('attachment-id', input.attachment.id),
      ...classAttrs<Message>(commandComposerStyles.attachmentAction),
    ],
    [input.action === 'preview' ? 'Open' : input.action === 'retry' ? 'Retry' : 'Remove'],
  )
}

export const commandComposerSubmit = <Message>(input: {
  status?: CommandComposerStatus
  label?: string
  disabled?: boolean
  attrs?: ReadonlyArray<Attribute<Message>>
}): Html => {
  const h = html<Message>()
  const status = input.status ?? 'ready'
  const active = status === 'submitted' || status === 'streaming'
  const label =
    input.label ??
    submitLabelFor(status, {
      name: 'prompt',
      status,
    })

  return h.button(
    [
      ...(input.attrs ?? []),
      aiElementBase<Message>(MODULE_ID, 'CommandComposerSubmit'),
      h.Type(active ? 'button' : 'submit'),
      h.AriaLabel(label),
      h.Title(label),
      h.DataAttribute('oa-command-composer-submit', active ? 'stop' : 'send'),
      h.DataAttribute('status', status),
      ...(input.disabled === true ? [h.Disabled(true)] : []),
      ...classAttrs<Message>(commandComposerStyles.submit),
    ],
    [
      commandComposerIcon<Message>(submitIconFor(status)),
      h.span([h.Class('oa-ai-command-composer-submit-label')], [label]),
    ],
  )
}

export const commandComposerAttachment = <Message>(input:
  | CommandComposerAttachmentProps
  | {
      props: CommandComposerAttachmentProps
      selected?: boolean
      actionAttrs?: CommandComposerAttachmentActionAttrs<Message>
    },
): Html => {
  const h = html<Message>()
  const props = normalizeAttachment('props' in input ? input.props : input)
  const selected = 'props' in input && input.selected === true
  const actionAttrs =
    'props' in input && input.actionAttrs !== undefined
      ? input.actionAttrs
      : () => []
  const iconName: CommandComposerIconName =
    props.kind === 'image'
      ? 'image'
      : props.kind === 'text'
        ? 'text'
        : props.kind === 'snippet'
          ? 'code'
          : 'file'

  const actionNodes: Html[] = [
    ...(props.previewUrl !== undefined || props.status === 'ready'
      ? [
          commandComposerAttachmentAction<Message>({
            attachment: props,
            action: 'preview',
            attrs: actionAttrs(props, 'preview'),
          }),
        ]
      : []),
    ...(props.status === 'error'
      ? [
          commandComposerAttachmentAction<Message>({
            attachment: props,
            action: 'retry',
            attrs: actionAttrs(props, 'retry'),
          }),
        ]
      : []),
    commandComposerAttachmentAction<Message>({
      attachment: props,
      action: 'remove',
      attrs: actionAttrs(props, 'remove'),
    }),
  ]

  return h.div(
    [
      aiElementBase<Message>(MODULE_ID, 'CommandComposerAttachment'),
      h.DataAttribute('oa-command-composer-attachment', props.id),
      h.DataAttribute('kind', props.kind),
      h.DataAttribute('status', props.status),
      h.DataAttribute('selected', selected ? 'true' : 'false'),
      h.Role('listitem'),
      h.Tabindex(0),
      h.AriaSelected(selected),
      ...classAttrs<Message>(commandComposerStyles.attachment),
    ],
    [
      h.span(
        [
          h.Tabindex(0),
          h.AriaLabel(`Before ${props.name}`),
          h.DataAttribute('oa-command-composer-gapcursor', 'before'),
          h.DataAttribute('attachment-id', props.id),
          h.Class('oa-ai-command-composer-gapcursor'),
        ],
        [],
      ),
      h.span(
        [h.AriaHidden(true), h.Class('oa-ai-command-composer-attachment-icon')],
        [commandComposerIcon<Message>(iconName)],
      ),
      ...(props.kind === 'image' && props.previewUrl !== undefined
        ? [
            h.img([
              h.Src(props.previewUrl),
              h.Alt(`${props.name} preview`),
              h.Loading('lazy'),
              h.Decoding('async'),
              ...(props.dimensions === undefined
                ? []
                : [
                    h.Width(String(props.dimensions.width)),
                    h.Height(String(props.dimensions.height)),
                  ]),
              h.Class('oa-ai-command-composer-attachment-thumb'),
            ]),
          ]
        : []),
      h.span([h.Class('oa-ai-command-composer-attachment-main')], [
        h.span([h.Class('oa-ai-command-composer-attachment-name')], [
          props.name,
        ]),
        h.span([h.Class('oa-ai-command-composer-attachment-meta')], [
          `${props.mime} - ${props.sizeLabel ?? formatBytes(props.sizeBytes)}`,
        ]),
        h.span([h.Class('oa-ai-command-composer-attachment-status')], [
          statusTextForAttachment(props.status),
        ]),
        ...(props.errorText === undefined
          ? []
          : [
              h.span(
                [h.Class('oa-ai-command-composer-attachment-error')],
                [props.errorText],
              ),
            ]),
      ]),
      h.span([h.Class('oa-ai-command-composer-attachment-actions')], actionNodes),
      h.span(
        [
          h.Tabindex(0),
          h.AriaLabel(`After ${props.name}`),
          h.DataAttribute('oa-command-composer-gapcursor', 'after'),
          h.DataAttribute('attachment-id', props.id),
          h.Class('oa-ai-command-composer-gapcursor'),
        ],
        [],
      ),
    ],
  )
}

export const commandComposerDropcursor = <Message>(): Html => {
  const h = html<Message>()

  return h.div(
    [
      aiElementBase<Message>(MODULE_ID, 'CommandComposerDropcursor'),
      h.DataAttribute('oa-command-composer-dropcursor', ''),
      h.AriaHidden(true),
      ...classAttrs<Message>(commandComposerStyles.dropcursor),
    ],
    [],
  )
}

export const commandComposerRail = <Message>(input: {
  attachments: ReadonlyArray<CommandComposerAttachmentProps>
  selectedAttachmentId?: string
  dragActive?: boolean
  actionAttrs?: CommandComposerAttachmentActionAttrs<Message>
}): Html => {
  const h = html<Message>()
  const attachments = input.attachments.map(normalizeAttachment)

  return h.div(
    [
      aiElementBase<Message>(MODULE_ID, 'CommandComposerRail'),
      h.DataAttribute('oa-command-composer-rail', ''),
      h.DataAttribute('oa-command-composer-drop-target', ''),
      h.DataAttribute(
        'oa-command-composer-drag-active',
        input.dragActive === true ? 'true' : 'false',
      ),
      h.Role('list'),
      h.AriaLabel('Composer attachments'),
      ...classAttrs<Message>(commandComposerStyles.rail),
    ],
    [
      ...attachments.map(attachment =>
        commandComposerAttachment<Message>({
          props: attachment,
          selected: input.selectedAttachmentId === attachment.id,
          ...(input.actionAttrs === undefined
            ? {}
            : { actionAttrs: input.actionAttrs }),
        }),
      ),
      ...(input.dragActive === true ? [commandComposerDropcursor<Message>()] : []),
    ],
  )
}

export const commandComposerStatusStrip = <Message>(input: {
  status: CommandComposerStatus
  tokenEstimate?: number
  sizeLabel?: string
  keymapLabel?: string
  attachmentCount?: number
}): Html => {
  const h = html<Message>()
  const details = [
    statusLabelFor(input.status),
    ...(input.attachmentCount === undefined || input.attachmentCount === 0
      ? []
      : [`${input.attachmentCount} attached`]),
    ...(input.tokenEstimate === undefined
      ? []
      : [`${input.tokenEstimate.toLocaleString()} tok`]),
    ...(input.sizeLabel === undefined ? [] : [input.sizeLabel]),
    ...(input.keymapLabel === undefined ? [] : [input.keymapLabel]),
  ]

  return h.div(
    [
      aiElementBase<Message>(MODULE_ID, 'CommandComposerStatus'),
      h.DataAttribute('oa-command-composer-status-label', input.status),
      h.AriaLive('polite'),
      ...classAttrs<Message>(commandComposerStyles.status),
    ],
    details.map((detail, index) =>
      h.span(
        [
          h.DataAttribute('slot', index === 0 ? 'status' : 'detail'),
          ...(index === 0 ? [h.DataAttribute('status', input.status)] : []),
        ],
        [detail],
      ),
    ),
  )
}

export const commandComposerControls = <Message>(input: {
  controls: ReadonlyArray<Html>
}): Html => {
  const h = html<Message>()

  return h.div(
    [
      aiElementBase<Message>(MODULE_ID, 'CommandComposerControls'),
      h.DataAttribute('oa-command-composer-controls', ''),
      ...classAttrs<Message>(commandComposerStyles.controls),
    ],
    input.controls,
  )
}

export const commandComposerResizeHandle = <Message>(input?: {
  attrs?: ReadonlyArray<Attribute<Message>>
}): Html => {
  const h = html<Message>()

  return h.button(
    [
      ...(input?.attrs ?? []),
      aiElementBase<Message>(MODULE_ID, 'CommandComposerResizeHandle'),
      h.Type('button'),
      h.AriaLabel('Resize composer'),
      h.Title('Resize composer'),
      h.DataAttribute('oa-command-composer-resize', ''),
      ...classAttrs<Message>(commandComposerStyles.resizeHandle),
    ],
    [commandComposerIcon<Message>('resize')],
  )
}

export const commandComposerA11y = <Message>(input: {
  status: CommandComposerStatus
  attachmentCount: number
  characterCount: number
}): Html => {
  const h = html<Message>()
  const attachmentText =
    input.attachmentCount === 1
      ? '1 attachment'
      : `${input.attachmentCount} attachments`

  return h.div(
    [
      aiElementBase<Message>(MODULE_ID, 'CommandComposerA11y'),
      h.AriaLive('polite'),
      ...classAttrs<Message>(commandComposerStyles.a11y),
    ],
    [
      `${statusLabelFor(input.status)}. ${attachmentText}. ${input.characterCount} characters.`,
    ],
  )
}

export const commandComposerMarkdownPreview = <Message>(input: {
  markdown: string
  streaming?: boolean
}): Html => {
  const h = html<Message>()

  return h.div(
    [
      aiElementBase<Message>(MODULE_ID, 'CommandComposerMarkdownPreview'),
      h.DataAttribute('oa-command-composer-preview', ''),
      ...classAttrs<Message>(commandComposerStyles.markdownPreview),
    ],
    [
      response<Message>({
        markdown: input.markdown,
        ...(input.streaming === undefined
          ? {}
          : { streaming: input.streaming }),
      }),
    ],
  )
}

const defaultControls = <Message>(
  props: CommandComposerProps,
): ReadonlyArray<Html> => [
  commandComposerButton<Message>({ label: 'Attach', icon: 'attach' }),
  commandComposerButton<Message>({ label: 'Text', icon: 'text' }),
  commandComposerButton<Message>({
    label: 'Preview',
    icon: props.preview === true ? 'preview-off' : 'preview',
    pressed: props.preview === true,
  }),
  commandComposerButton<Message>({
    label: props.expanded === true ? 'Compact' : 'Expand',
    icon: props.expanded === true ? 'compact' : 'expand',
    pressed: props.expanded === true,
  }),
]

export const commandComposer = <Message>(input: {
  props: CommandComposerProps
  state?: ComposerState
  attachments?: ReadonlyArray<CommandComposerAttachmentProps>
  controls?: ReadonlyArray<Html>
  attachmentActionAttrs?: CommandComposerAttachmentActionAttrs<Message>
  formAttrs?: ReadonlyArray<Attribute<Message>>
  textareaAttrs?: ReadonlyArray<Attribute<Message>>
  submitAttrs?: ReadonlyArray<Attribute<Message>>
  resizeAttrs?: ReadonlyArray<Attribute<Message>>
}): Html => {
  const h = html<Message>()
  const props = decodeProps(input.props)
  const state = input.state ?? emptyComposerState()
  const status = props.status ?? 'ready'
  const value = textareaValueFor(props, state)
  const attachments = (
    input.attachments ??
    state.doc.attachments
  ).map(attachment => normalizeAttachment(attachment))
  const selectedAttachmentId =
    props.selectedAttachmentId ?? state.selection.selectedAttachmentId
  const textareaId = `oa-command-composer-${idSafe(props.name)}`
  const label = props.label ?? 'Message Khala'

  return h.form(
    [
      ...(input.formAttrs ?? []),
      aiElementBase<Message>(MODULE_ID, 'CommandComposer'),
      h.DataAttribute('oa-command-composer', ''),
      h.DataAttribute('oa-command-composer-status', status),
      h.DataAttribute(
        'oa-command-composer-expanded',
        props.expanded === true ? 'true' : 'false',
      ),
      h.DataAttribute('oa-composer-schema-version', String(state.doc.schemaVersion)),
      ...(props.heightPx === undefined
        ? []
        : [
            h.Style({
              '--oa-command-composer-height': `${props.heightPx}px`,
            }),
          ]),
      ...classAttrs<Message>(commandComposerStyles.root),
    ],
    [
      commandComposerFrame<Message>({
        children: [
          h.label(
            [h.For(textareaId), h.Class('oa-ai-command-composer-label')],
            [label],
          ),
          ...(attachments.length === 0
            ? []
            : [
                commandComposerRail<Message>({
                  attachments,
                  ...(selectedAttachmentId === undefined
                    ? {}
                    : { selectedAttachmentId }),
                  dragActive: props.dragActive === true,
                  ...(input.attachmentActionAttrs === undefined
                    ? {}
                    : { actionAttrs: input.attachmentActionAttrs }),
                }),
              ]),
          h.textarea(
            [
              ...(input.textareaAttrs ?? []),
              aiElementBase<Message>(MODULE_ID, 'CommandComposerTextarea'),
              h.Id(textareaId),
              h.Name(props.name),
              h.AriaLabel(label),
              h.AriaDescribedBy(`${textareaId}-a11y`),
              h.Placeholder(props.placeholder ?? 'Send a message'),
              h.Rows(props.rows ?? 3),
              h.Spellcheck(true),
              h.Autocapitalize('sentences'),
              h.InputMode('text'),
              h.EnterKeyHint('send'),
              h.DataAttribute('oa-command-composer-textarea', ''),
              h.DataAttribute('oa-command-composer-native-editing', 'true'),
              h.DataAttribute('oa-command-composer-focus-after-submit', ''),
              ...(props.autofocus === true
                ? [
                    h.Autofocus(true),
                    h.DataAttribute('oa-command-composer-autofocus', ''),
                  ]
                : []),
              ...classAttrs<Message>(commandComposerStyles.textarea),
            ],
            [value],
          ),
          ...(props.preview === true
            ? [
                commandComposerMarkdownPreview<Message>({
                  markdown: value,
                  streaming: status === 'streaming',
                }),
              ]
            : []),
          h.div([h.Class('oa-ai-command-composer-footer')], [
            commandComposerControls<Message>({
              controls: input.controls ?? defaultControls<Message>(props),
            }),
            commandComposerStatusStrip<Message>({
              status,
              attachmentCount: attachments.length,
              ...(props.tokenEstimate === undefined
                ? {}
                : { tokenEstimate: props.tokenEstimate }),
              ...(props.sizeLabel === undefined ? {} : { sizeLabel: props.sizeLabel }),
              ...(props.keymapLabel === undefined
                ? {}
                : { keymapLabel: props.keymapLabel }),
            }),
            commandComposerResizeHandle<Message>({
              ...(input.resizeAttrs === undefined
                ? {}
                : { attrs: input.resizeAttrs }),
            }),
            commandComposerSubmit<Message>({
              status,
              label: submitLabelFor(status, props),
              ...(input.submitAttrs === undefined
                ? {}
                : { attrs: input.submitAttrs }),
            }),
          ]),
          h.div([h.Id(`${textareaId}-a11y`)], [
            commandComposerA11y<Message>({
              status,
              attachmentCount: attachments.length,
              characterCount: value.length,
            }),
          ]),
        ],
      }),
    ],
  )
}

export const commandComposerClassNames = {
  root: commandComposerClass,
  frame: commandComposerFrameClass,
  textarea: commandComposerTextareaClass,
  rail: commandComposerRailClass,
  attachment: commandComposerAttachmentClass,
  status: commandComposerStatusClass,
  controls: commandComposerControlsClass,
  button: commandComposerButtonClass,
  submit: commandComposerSubmitClass,
  resizeHandle: commandComposerResizeHandleClass,
  a11y: commandComposerA11yClass,
  markdownPreview: commandComposerMarkdownPreviewClass,
  attachmentAction: commandComposerAttachmentActionClass,
  dropcursor: commandComposerDropcursorClass,
} as const

export const commandComposerClassName = (
  part: keyof typeof commandComposerClassNames,
): string => commandComposerClassNames[part]
