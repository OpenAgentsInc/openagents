import { clsx } from 'clsx'
import type { Attribute, Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import {
  button,
  eyebrowClass,
  kitFamily,
  metaClass,
  statusDotClass,
  surfaceClass,
  titleClass,
  toneTextClass,
} from '@openagentsinc/ui'
import type { Tone } from '@openagentsinc/ui'

// EMAIL-SEQUENCE CUSTOMER UI (#4983/#4984; promise
// autopilot_sites.native_email_sequences.v1, yellow).
//
// Self-contained, presentational Foldkit/HTML builder for the customer-facing
// view of a native email sequence: the sequence header, its ordered steps, and
// (optionally) the viewer's enrollment status. Mirrors the credits-panel
// pattern exactly — pure builder functions over typed view inputs, no data
// fetching, no message wiring beyond optional caller-supplied attributes on the
// enroll/manage actions. An embedding page supplies the already-projected data
// from `projectEmailSequenceDefinition` (workers/api email-sequence-authoring)
// plus the viewer's enrollment state.
//
// This is the "customer UI" last-mile for the
// `email_sequence_customer_ui_missing` blocker. It is display-only and carries
// no send authority: sending stays behind the dispatcher and its own armed
// flag (see workers/api email-sequence-send-service.ts). Rendering a sequence
// here never enrolls anyone or sends anything.

// Lifecycle status of a sequence, mirroring the operator authoring literals
// (EmailSequenceStatusSchema in workers/api email-sequence-authoring.ts). Kept
// local so the web bundle does not depend on the worker package.
export type EmailSequenceStatus = 'draft' | 'active' | 'paused' | 'archived'

// Per-step status, same literal set (steps inherit the campaign status).
export type EmailSequenceStepStatus = EmailSequenceStatus

// Viewer enrollment state for this sequence. `none` = not enrolled; `enrolled`
// = active enrollment with scheduled sends; `skipped` = enrollment was declined
// because the viewer suppressed drip mail or disabled the drip preference.
export type EmailSequenceEnrollmentState = 'none' | 'enrolled' | 'skipped'

export type EmailSequenceStepModel = Readonly<{
  // Stable per-campaign step key.
  stepKey: string
  // Human label for the step, e.g. "Welcome".
  name: string
  // Pre-formatted send delay relative to enrollment, e.g. "Immediately" or
  // "After 2 days". Formatted by the caller; this component only arranges it.
  delayLabel: string
  status: EmailSequenceStepStatus
  // Optional lifecycle tag, e.g. "signup_day_0". Shown as meta when present.
  lifecycleKind?: string | null
}>

export type EmailSequenceModel = Readonly<{
  // Stable sequence slug (identity).
  slug: string
  // Human label, e.g. "New customer welcome".
  name: string
  // Audience label, e.g. "customer".
  audience: string
  status: EmailSequenceStatus
  steps: ReadonlyArray<EmailSequenceStepModel>
}>

export type EmailSequenceEnrollmentModel = Readonly<{
  state: EmailSequenceEnrollmentState
  // Number of scheduled per-step sends for an active enrollment. Ignored for
  // non-enrolled states. The displayed step count always comes from the
  // sequence's own steps.
  scheduledSendCount?: number
  // For state === 'skipped', the reason mirrors the worker
  // EnrollSubscriberResult skip reasons.
  skipReason?: 'drip_preference_disabled' | 'drip_suppressed'
}>

const statusTone = (status: EmailSequenceStatus): Tone => {
  switch (status) {
    case 'active':
      return 'positive'
    case 'draft':
      return 'accent'
    case 'paused':
      return 'warning'
    case 'archived':
      return 'neutral'
  }
}

const statusLabel = (status: EmailSequenceStatus): string => {
  switch (status) {
    case 'active':
      return 'Active'
    case 'draft':
      return 'Draft'
    case 'paused':
      return 'Paused'
    case 'archived':
      return 'Archived'
  }
}

const enrollmentTone = (state: EmailSequenceEnrollmentState): Tone => {
  switch (state) {
    case 'enrolled':
      return 'positive'
    case 'skipped':
      return 'warning'
    case 'none':
      return 'neutral'
  }
}

const skipReasonLabel = (
  reason: EmailSequenceEnrollmentModel['skipReason'],
): string => {
  switch (reason) {
    case 'drip_preference_disabled':
      return 'You have turned off these emails in your preferences.'
    case 'drip_suppressed':
      return 'These emails are suppressed for your address.'
    case undefined:
      return 'Enrollment was skipped.'
  }
}

// The sequence is enrollable from the customer's perspective only when it is
// active. Draft/paused/archived sequences render read-only.
export const isSequenceEnrollable = (model: EmailSequenceModel): boolean =>
  model.status === 'active'

// A single step row: order index, name, delay, lifecycle tag, and status dot.
const stepRow = <Message>(
  index: number,
  step: EmailSequenceStepModel,
): Html => {
  const h = html<Message>()
  const tone = statusTone(step.status)

  return h.li(
    [
      h.Class(
        'grid grid-cols-[auto_minmax(0,1fr)_auto] items-center gap-3 border-t border-[#222] py-2 first:border-t-0',
      ),
      h.DataAttribute('email-sequence-step', step.stepKey),
    ],
    [
      h.span(
        [
          h.Class(
            'flex h-6 w-6 flex-none items-center justify-center rounded-full border border-[#333] text-xs tabular-nums text-[#9a978d]',
          ),
        ],
        [String(index + 1)],
      ),
      h.div(
        [h.Class('min-w-0')],
        [
          h.p([h.Class(clsx(titleClass, 'truncate'))], [step.name]),
          h.p(
            [h.Class(metaClass)],
            [
              step.lifecycleKind === undefined || step.lifecycleKind === null
                ? step.delayLabel
                : `${step.delayLabel} · ${step.lifecycleKind}`,
            ],
          ),
        ],
      ),
      h.span(
        [h.Class(clsx('flex flex-none items-center gap-1.5', toneTextClass(tone)))],
        [
          h.span([h.Class(statusDotClass(tone))], []),
          h.span(
            [h.Class('text-xs font-medium uppercase tracking-[0.08em]')],
            [statusLabel(step.status)],
          ),
        ],
      ),
    ],
  )
}

// Optional enrollment-status row, shown when the caller supplies enrollment
// state. Read-only; the enroll action (if any) is rendered separately so a page
// can place it independently.
export const emailSequenceEnrollmentStatus = <Message>(
  enrollment: EmailSequenceEnrollmentModel,
): Html => {
  const h = html<Message>()
  const tone = enrollmentTone(enrollment.state)

  const message =
    enrollment.state === 'enrolled'
      ? enrollment.scheduledSendCount === undefined
        ? 'You are enrolled in this sequence.'
        : `You are enrolled. ${enrollment.scheduledSendCount} email${
            enrollment.scheduledSendCount === 1 ? '' : 's'
          } scheduled.`
      : enrollment.state === 'skipped'
        ? skipReasonLabel(enrollment.skipReason)
        : 'You are not enrolled in this sequence.'

  return h.div(
    [
      kitFamily<Message>('feedback/alerts'),
      h.Class(
        clsx(
          'grid grid-cols-[auto_minmax(0,1fr)] items-center gap-2 border border-[#222] p-3 text-sm leading-5',
          toneTextClass(tone),
        ),
      ),
      h.DataAttribute('email-sequence-enrollment', enrollment.state),
    ],
    [
      h.span([h.Class(statusDotClass(tone))], []),
      h.span([h.Class('min-w-0')], [message]),
    ],
  )
}

// Full customer-facing sequence panel: header (name/audience/status), the
// ordered steps, an optional enrollment-status row, and an optional enroll
// action. Supplying `enrollAttrs` wires a message on the enroll button; omit it
// (and `enrollment`) for a purely read-only display.
export const emailSequencePanel = <Message>(
  model: EmailSequenceModel,
  options: {
    enrollment?: EmailSequenceEnrollmentModel
    enrollAttrs?: ReadonlyArray<Attribute<Message>>
  } = {},
): Html => {
  const h = html<Message>()
  const tone = statusTone(model.status)
  const enrollable = isSequenceEnrollable(model)
  const alreadyEnrolled = options.enrollment?.state === 'enrolled'

  return h.section(
    [
      kitFamily<Message>('data-display/cards'),
      h.Class(clsx(surfaceClass, 'grid gap-4 p-4')),
      h.DataAttribute('email-sequence-panel', model.slug),
      h.DataAttribute('email-sequence-status', model.status),
    ],
    [
      h.div(
        [h.Class('flex min-w-0 items-start justify-between gap-3')],
        [
          h.div(
            [h.Class('min-w-0')],
            [
              h.p([h.Class(eyebrowClass)], ['Email sequence']),
              h.p(
                [
                  h.Class(clsx('m-0 text-xl font-semibold text-[#f1efe8]')),
                  h.DataAttribute('email-sequence-name', ''),
                ],
                [model.name],
              ),
              h.p([h.Class(metaClass)], [`Audience: ${model.audience}`]),
            ],
          ),
          h.div(
            [
              h.Class(
                clsx('flex flex-none items-center gap-2', toneTextClass(tone)),
              ),
            ],
            [
              h.span([h.Class(statusDotClass(tone))], []),
              h.span(
                [h.Class('text-xs font-medium uppercase tracking-[0.08em]')],
                [statusLabel(model.status)],
              ),
            ],
          ),
        ],
      ),
      model.steps.length === 0
        ? h.p(
            [h.Class(metaClass), h.DataAttribute('email-sequence-no-steps', '')],
            ['This sequence has no steps yet.'],
          )
        : h.ul(
            [h.Class('m-0 grid list-none gap-0 p-0')],
            model.steps.map((step, index) => stepRow<Message>(index, step)),
          ),
      options.enrollment === undefined
        ? null
        : emailSequenceEnrollmentStatus<Message>(options.enrollment),
      options.enrollAttrs === undefined || alreadyEnrolled
        ? null
        : button<Message>({
            label: enrollable ? 'Join this sequence' : 'Not accepting signups',
            size: 'sm',
            variant: 'primary',
            block: true,
            attrs: enrollable
              ? options.enrollAttrs
              : [h.Disabled(true), ...options.enrollAttrs],
          }),
      h.p(
        [h.Class(metaClass)],
        [
          `${model.steps.length} step${
            model.steps.length === 1 ? '' : 's'
          } in this sequence.`,
        ],
      ),
    ],
  )
}
