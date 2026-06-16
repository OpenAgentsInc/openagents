import { clsx } from 'clsx'
import { Schema } from 'effect'
import type { Attribute, Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import { metaClass, statusDotClass, titleClass } from '../primitives'
import { aiElementBase } from './base'

const MODULE_ID = 'task'

// Ported from the autopilot3 task markup + Maud `AI_TASK_*` contracts, in the
// kit's dark-only palette. The trigger/content split mirrors the upstream
// collapsible contract (server emits both; JS only toggles trusted markup).
export const taskClass = 'grid gap-2 border border-[#222] bg-[#010102] p-3'
export const taskTriggerClass =
  'flex w-full items-center gap-2 text-[0.8125rem] text-white/60 transition-colors hover:text-[#f1efe8]'
export const taskContentClass = 'grid gap-2 border-l border-[#222] pl-4'
export const taskItemClass = 'flex items-start gap-2 text-[0.8125rem] text-white/60'
export const taskItemFileClass =
  'inline-flex items-center gap-1 border border-[#333] bg-[#141414] px-1.5 py-0.5 font-mono text-[0.75rem] text-[#f1efe8]'

export const TaskItemStatus = Schema.Literals([
  'done',
  'active',
  'queued',
  'failed',
])
export type TaskItemStatus = typeof TaskItemStatus.Type

export const TaskItemProps = Schema.Struct({
  label: Schema.String,
  status: Schema.optional(TaskItemStatus),
})
export type TaskItemProps = typeof TaskItemProps.Type

export const TaskProps = Schema.Struct({
  title: Schema.String,
  open: Schema.optional(Schema.Boolean),
  items: Schema.Array(TaskItemProps),
})
export type TaskProps = typeof TaskProps.Type

const itemTone = (status: TaskItemStatus | undefined) => {
  if (status === 'done') {
    return 'positive' as const
  }

  if (status === 'failed') {
    return 'negative' as const
  }

  if (status === 'active') {
    return 'info' as const
  }

  return 'neutral' as const
}

export const taskItemFile = <Message>(label: string): Html => {
  const h = html<Message>()

  return h.span(
    [
      aiElementBase<Message>(MODULE_ID, 'TaskItemFile'),
      h.Class(taskItemFileClass),
    ],
    [label],
  )
}

export const taskItem = <Message>(props: TaskItemProps): Html => {
  const h = html<Message>()
  const decoded = Schema.decodeUnknownSync(TaskItemProps)(props)

  return h.div(
    [aiElementBase<Message>(MODULE_ID, 'TaskItem'), h.Class(taskItemClass)],
    [
      h.span(
        [h.Class(clsx(statusDotClass(itemTone(decoded.status)), 'mt-1.5'))],
        [],
      ),
      h.span([], [decoded.label]),
    ],
  )
}

export const taskTrigger = <Message>(input: {
  title: string
  open?: boolean
  attrs?: ReadonlyArray<Attribute<Message>>
}): Html => {
  const h = html<Message>()

  return h.button(
    [
      ...(input.attrs ?? []),
      aiElementBase<Message>(MODULE_ID, 'TaskTrigger'),
      h.Type('button'),
      h.AriaExpanded(input.open ?? true),
      h.Class(taskTriggerClass),
    ],
    [h.span([h.Class(titleClass)], [input.title])],
  )
}

export const taskContent = <Message>(items: ReadonlyArray<Html>): Html => {
  const h = html<Message>()

  return h.div(
    [aiElementBase<Message>(MODULE_ID, 'TaskContent'), h.Class(taskContentClass)],
    items,
  )
}

// A collapsible task list: a trigger row with the title and an ordered list of
// status-tagged items.
export const task = <Message>(input: {
  props: TaskProps
  triggerAttrs?: ReadonlyArray<Attribute<Message>>
  attrs?: ReadonlyArray<Attribute<Message>>
}): Html => {
  const h = html<Message>()
  const props = Schema.decodeUnknownSync(TaskProps)(input.props)

  return h.div(
    [
      ...(input.attrs ?? []),
      aiElementBase<Message>(MODULE_ID, 'Task'),
      h.Class(taskClass),
    ],
    [
      taskTrigger<Message>({
        title: props.title,
        ...(props.open === undefined ? {} : { open: props.open }),
        ...(input.triggerAttrs === undefined
          ? {}
          : { attrs: input.triggerAttrs }),
      }),
      props.items.length === 0
        ? h.p([h.Class(metaClass)], ['No steps yet'])
        : taskContent<Message>(
            props.items.map(item => taskItem<Message>(item)),
          ),
    ],
  )
}
