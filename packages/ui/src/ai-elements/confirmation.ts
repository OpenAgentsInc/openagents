import { clsx } from 'clsx'
import { Schema } from 'effect'
import type { Attribute, Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import { buttonClass, metaClass, statusDotClass, titleClass } from '../primitives'
import { aiElementBase } from './base'

const MODULE_ID = 'confirmation'

// Ported from the autopilot3 confirmation markup + Maud `AI_CONFIRMATION_*`
// contracts, in the kit's dark-only palette. Human-in-the-loop approval gate:
// renders accept/reject controls when requested, and a resolved state line
// once responded.
export const confirmationClass =
  'grid gap-2 border border-[#ffb400] bg-[#010102] p-3'
export const confirmationResolvedClass = 'border-[#222]'
export const confirmationTitleClass = clsx(titleClass, 'inline')
export const confirmationActionsClass =
  'flex items-center justify-end gap-2 self-end'
export const confirmationStateClass = 'flex items-center gap-2'

export const ConfirmationState = Schema.Literals([
  'requested',
  'approved',
  'rejected',
])
export type ConfirmationState = typeof ConfirmationState.Type

export const ConfirmationProps = Schema.Struct({
  title: Schema.String,
  state: ConfirmationState,
  detail: Schema.optional(Schema.String),
  approveLabel: Schema.optional(Schema.String),
  rejectLabel: Schema.optional(Schema.String),
})
export type ConfirmationProps = typeof ConfirmationProps.Type

export const confirmationAction = <Message>(input: {
  label: string
  variant?: 'primary' | 'secondary' | 'danger'
  attrs?: ReadonlyArray<Attribute<Message>>
}): Html => {
  const h = html<Message>()

  return h.button(
    [
      ...(input.attrs ?? []),
      aiElementBase<Message>(MODULE_ID, 'ConfirmationAction'),
      h.Type('button'),
      h.Class(buttonClass(input.variant ?? 'secondary', 'sm')),
    ],
    [input.label],
  )
}

export const confirmationTitle = <Message>(text: string): Html => {
  const h = html<Message>()

  return h.span(
    [
      aiElementBase<Message>(MODULE_ID, 'ConfirmationTitle'),
      h.Class(confirmationTitleClass),
    ],
    [text],
  )
}

export const confirmationActions = <Message>(
  actions: ReadonlyArray<Html>,
): Html => {
  const h = html<Message>()

  return h.div(
    [
      aiElementBase<Message>(MODULE_ID, 'ConfirmationActions'),
      h.Class(confirmationActionsClass),
    ],
    actions,
  )
}

// An approval gate. When `state` is `requested`, the supplied `actions`
// (accept/reject `confirmationAction`s) render; once resolved, a status line
// reflects the decision instead.
export const confirmation = <Message>(input: {
  props: ConfirmationProps
  actions?: ReadonlyArray<Html>
  attrs?: ReadonlyArray<Attribute<Message>>
}): Html => {
  const h = html<Message>()
  const props = Schema.decodeUnknownSync(ConfirmationProps)(input.props)
  const resolved = props.state !== 'requested'

  const resolvedLine =
    props.state === 'approved'
      ? h.div(
          [h.Class(confirmationStateClass)],
          [
            h.span([h.Class(statusDotClass('positive'))], []),
            h.span([h.Class(metaClass)], ['Approved']),
          ],
        )
      : props.state === 'rejected'
        ? h.div(
            [h.Class(confirmationStateClass)],
            [
              h.span([h.Class(statusDotClass('negative'))], []),
              h.span([h.Class(metaClass)], ['Rejected']),
            ],
          )
        : null

  return h.div(
    [
      ...(input.attrs ?? []),
      aiElementBase<Message>(MODULE_ID, 'Confirmation'),
      h.Role('alert'),
      h.Class(
        clsx(confirmationClass, { [confirmationResolvedClass]: resolved }),
      ),
    ],
    [
      confirmationTitle<Message>(props.title),
      props.detail === undefined
        ? null
        : h.p([h.Class(metaClass)], [props.detail]),
      resolved
        ? resolvedLine
        : confirmationActions<Message>(input.actions ?? []),
    ],
  )
}
