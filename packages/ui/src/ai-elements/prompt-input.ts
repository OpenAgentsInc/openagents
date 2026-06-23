import { Schema } from 'effect'
import type { Attribute, Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import {
  stylexAttrs,
  stylexFallback,
} from '../stylex-foldkit'
import { aiElementBase } from './base'

const MODULE_ID = 'prompt-input'

// Named class constants (no ad-hoc/model-authored class strings). Ported from
// the autopilot3 prompt-input markup + the Maud `AI_PROMPT_INPUT_*` contracts,
// re-expressed in the kit's dark-only / pure-black / compact-mono palette.
export const promptInputClass =
  'grid w-full gap-2 border border-[#222] bg-[#010102] p-2'
export const promptInputBodyClass = 'grid gap-2'
export const promptInputTextareaClass =
  'field-sizing-content max-h-48 min-h-16 w-full resize-none border-0 bg-transparent px-2 py-1.5 font-mono text-[0.8125rem] leading-[1.35] text-[#f1efe8] outline-none placeholder:text-white/30'
export const promptInputFooterClass =
  'flex min-w-0 items-center justify-between gap-2'
export const promptInputToolsClass = 'flex min-w-0 items-center gap-1'
export const promptInputButtonClass =
  'inline-flex min-h-8 cursor-pointer items-center gap-1.5 border border-[#333] bg-transparent px-2.5 text-[0.75rem] text-white/60 hover:border-[#ffb400] hover:text-[#f1efe8] disabled:cursor-not-allowed disabled:opacity-45'
export const promptInputSubmitClass =
  'inline-flex min-h-8 cursor-pointer items-center gap-2 border border-[#f1efe8] bg-[#f1efe8] px-3 text-[0.8125rem] font-medium text-[#000] hover:border-[#ffb400] disabled:cursor-not-allowed disabled:opacity-45'

const promptInputStyles = {
  root: stylexFallback('oa-ai-prompt-input'),
  body: stylexFallback('oa-ai-prompt-input-body'),
  textarea: stylexFallback('oa-ai-prompt-input-textarea'),
  footer: stylexFallback('oa-ai-prompt-input-footer'),
  tools: stylexFallback('oa-ai-prompt-input-tools'),
  button: stylexFallback('oa-ai-prompt-input-button'),
  submit: stylexFallback('oa-ai-prompt-input-submit'),
}

export const PromptInputStatus = Schema.Literals([
  'ready',
  'submitted',
  'streaming',
  'error',
])
export type PromptInputStatus = typeof PromptInputStatus.Type

export const PromptInputProps = Schema.Struct({
  name: Schema.String,
  placeholder: Schema.optional(Schema.String),
  value: Schema.optional(Schema.String),
  status: Schema.optional(PromptInputStatus),
  submitLabel: Schema.optional(Schema.String),
  rows: Schema.optional(Schema.Number),
})
export type PromptInputProps = typeof PromptInputProps.Type

const submitLabelFor = (status: PromptInputStatus | undefined): string => {
  if (status === 'submitted' || status === 'streaming') {
    return 'Stop'
  }

  if (status === 'error') {
    return 'Retry'
  }

  return 'Send'
}

export const promptInputButton = <Message>(input: {
  label: string
  disabled?: boolean
  attrs?: ReadonlyArray<Attribute<Message>>
}): Html => {
  const h = html<Message>()

  return h.button(
    [
      ...(input.attrs ?? []),
      aiElementBase<Message>(MODULE_ID, 'PromptInputButton'),
      h.Type('button'),
      ...(input.disabled === true ? [h.Disabled(true)] : []),
      ...stylexAttrs<Message>(promptInputStyles.button),
    ],
    [input.label],
  )
}

export const promptInputSubmit = <Message>(input: {
  status?: PromptInputStatus
  label?: string
  disabled?: boolean
  attrs?: ReadonlyArray<Attribute<Message>>
}): Html => {
  const h = html<Message>()
  const label = input.label ?? submitLabelFor(input.status)

  return h.button(
    [
      ...(input.attrs ?? []),
      aiElementBase<Message>(MODULE_ID, 'PromptInputSubmit'),
      h.Type('submit'),
      ...(input.disabled === true ? [h.Disabled(true)] : []),
      ...stylexAttrs<Message>(promptInputStyles.submit),
    ],
    [label],
  )
}

// Composer surface: a labelled textarea body plus a footer with a tools row
// (attachments / commands live here as `promptInputButton`s) and a submit
// control whose label tracks the request status.
export const promptInput = <Message>(input: {
  props: PromptInputProps
  tools?: ReadonlyArray<Html>
  formAttrs?: ReadonlyArray<Attribute<Message>>
  textareaAttrs?: ReadonlyArray<Attribute<Message>>
  submitAttrs?: ReadonlyArray<Attribute<Message>>
}): Html => {
  const h = html<Message>()
  const props = Schema.decodeUnknownSync(PromptInputProps)(input.props)

  return h.form(
    [
      ...(input.formAttrs ?? []),
      aiElementBase<Message>(MODULE_ID, 'PromptInput'),
      ...stylexAttrs<Message>(promptInputStyles.root),
    ],
    [
      h.div(
        [
          aiElementBase<Message>(MODULE_ID, 'PromptInputBody'),
          ...stylexAttrs<Message>(promptInputStyles.body),
        ],
        [
          h.textarea(
            [
              ...(input.textareaAttrs ?? []),
              aiElementBase<Message>(MODULE_ID, 'PromptInputTextarea'),
              h.Name(props.name),
              ...(props.placeholder === undefined
                ? []
                : [h.Placeholder(props.placeholder)]),
              ...(props.rows === undefined ? [] : [h.Rows(props.rows)]),
              ...stylexAttrs<Message>(promptInputStyles.textarea),
            ],
            [props.value ?? ''],
          ),
        ],
      ),
      h.div(
        [
          aiElementBase<Message>(MODULE_ID, 'PromptInputFooter'),
          ...stylexAttrs<Message>(promptInputStyles.footer),
        ],
        [
          h.div(
            [
              aiElementBase<Message>(MODULE_ID, 'PromptInputTools'),
              ...stylexAttrs<Message>(promptInputStyles.tools),
            ],
            input.tools ?? [],
          ),
          promptInputSubmit<Message>({
            ...(props.status === undefined ? {} : { status: props.status }),
            ...(props.submitLabel === undefined
              ? {}
              : { label: props.submitLabel }),
            disabled: props.status === 'submitted',
            ...(input.submitAttrs === undefined
              ? {}
              : { attrs: input.submitAttrs }),
          }),
        ],
      ),
    ],
  )
}
