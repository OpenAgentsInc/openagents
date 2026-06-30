import { Effect, Schema as S } from 'effect'
import type { Command } from 'foldkit/command'
import { define as defineCommand } from 'foldkit/command'
import type { Attribute, Html } from 'foldkit/html'
import { html } from 'foldkit/html'
import { m } from 'foldkit/message'
import type { CallableTaggedStruct } from 'foldkit/schema'

import {
  basecoatAttrs,
  basecoatClass,
  dataAttr,
  type BasecoatAttrs,
  type BasecoatChildren,
} from './shared'

export type Model = Readonly<{
  id: string
  isOpen: boolean
  focusSelector?: string
}>

export const RequestedOpen: CallableTaggedStruct<
  'BasecoatDialogRequestedOpen',
  {}
> = m('BasecoatDialogRequestedOpen')
export const RequestedClose: CallableTaggedStruct<
  'BasecoatDialogRequestedClose',
  {}
> = m('BasecoatDialogRequestedClose')
export const CompletedShowDialog: CallableTaggedStruct<
  'BasecoatDialogCompletedShowDialog',
  {}
> = m('BasecoatDialogCompletedShowDialog')
export const CompletedCloseDialog: CallableTaggedStruct<
  'BasecoatDialogCompletedCloseDialog',
  {}
> = m('BasecoatDialogCompletedCloseDialog')

export const Message: S.Union<[
  typeof RequestedOpen,
  typeof RequestedClose,
  typeof CompletedShowDialog,
  typeof CompletedCloseDialog,
]> = S.Union([
  RequestedOpen,
  RequestedClose,
  CompletedShowDialog,
  CompletedCloseDialog,
])

export type Message = typeof Message.Type

export const Opened: CallableTaggedStruct<'BasecoatDialogOpened', {}> =
  m('BasecoatDialogOpened')
export const Closed: CallableTaggedStruct<'BasecoatDialogClosed', {}> =
  m('BasecoatDialogClosed')

export const OutMessage: S.Union<[typeof Opened, typeof Closed]> = S.Union([
  Opened,
  Closed,
])

export type OutMessage = typeof OutMessage.Type

export type InitConfig = Readonly<{
  id: string
  isOpen?: boolean
  focusSelector?: string
}>

export type UpdateResult = readonly [
  Model,
  ReadonlyArray<Command<Message>>,
  OutMessage | null,
]

export type DialogTriggerProps<ParentMessage> =
  BasecoatAttrs<ParentMessage> & Readonly<{
    children: BasecoatChildren
    toMessage?: (message: Message) => ParentMessage
    disabled?: boolean
    type?: 'button' | 'submit' | 'reset'
  }>

export type DialogCloseProps<ParentMessage> =
  BasecoatAttrs<ParentMessage> & Readonly<{
    toMessage?: (message: Message) => ParentMessage
    ariaLabel?: string
    children?: BasecoatChildren
    disabled?: boolean
  }>

export type DialogHeaderProps<ParentMessage> =
  BasecoatAttrs<ParentMessage> & Readonly<{
    children: BasecoatChildren
  }>

export type DialogTitleProps<ParentMessage> =
  BasecoatAttrs<ParentMessage> & Readonly<{
    model: Model
    children: BasecoatChildren
  }>

export type DialogDescriptionProps<ParentMessage> =
  BasecoatAttrs<ParentMessage> & Readonly<{
    model: Model
    children: BasecoatChildren
  }>

export type DialogBodyProps<ParentMessage> =
  BasecoatAttrs<ParentMessage> & Readonly<{
    children: BasecoatChildren
  }>

export type DialogFooterProps<ParentMessage> =
  BasecoatAttrs<ParentMessage> & Readonly<{
    children: BasecoatChildren
  }>

export type DialogSurfaceProps<ParentMessage> =
  BasecoatAttrs<ParentMessage> & Readonly<{
    children: BasecoatChildren
  }>

export type DialogProps<ParentMessage> =
  BasecoatAttrs<ParentMessage> & Readonly<{
    model: Model
    children: BasecoatChildren
    toMessage?: (message: Message) => ParentMessage
    labelledBy?: string
    describedBy?: string
    modal?: boolean
    closeOnCancel?: boolean
    closeOnOverlayClick?: boolean
  }>

export type DialogViewProps<ParentMessage> =
  Omit<DialogProps<ParentMessage>, 'children'> & Readonly<{
    title?: BasecoatChildren
    description?: BasecoatChildren
    body?: BasecoatChildren
    footer?: BasecoatChildren
    closeButton?: boolean
    surfaceAttrs?: ReadonlyArray<Attribute<ParentMessage>>
    surfaceClassName?: string
    headerAttrs?: ReadonlyArray<Attribute<ParentMessage>>
    headerClassName?: string
    bodyAttrs?: ReadonlyArray<Attribute<ParentMessage>>
    bodyClassName?: string
    footerAttrs?: ReadonlyArray<Attribute<ParentMessage>>
    footerClassName?: string
  }>

const dialogRoot = basecoatClass('dialog')
const alertDialogRoot = basecoatClass('alert-dialog')
const buttonRoot = basecoatClass('btn')

const optionalStringAttr = <Message>(
  value: string | undefined,
  attr: (value: string) => Attribute<Message>,
): ReadonlyArray<Attribute<Message>> => value === undefined ? [] : [attr(value)]

const optionalBooleanAttr = <Message>(
  enabled: boolean | undefined,
  attr: (value: true) => Attribute<Message>,
): ReadonlyArray<Attribute<Message>> => enabled === true ? [attr(true)] : []

const optionalBasecoatProps = <Message>(
  attrs: ReadonlyArray<Attribute<Message>> | undefined,
  className: string | undefined,
): BasecoatAttrs<Message> => ({
  ...(attrs === undefined ? {} : { attrs }),
  ...(className === undefined ? {} : { className }),
})

const dialogLabelAttrs = (
  labelledBy: string | undefined,
  describedBy: string | undefined,
): Readonly<{
  labelledBy?: string
  describedBy?: string
}> => ({
  ...(labelledBy === undefined ? {} : { labelledBy }),
  ...(describedBy === undefined ? {} : { describedBy }),
})

const mapMessage = <ParentMessage>(
  input: Readonly<{ toMessage?: (message: Message) => ParentMessage }>,
  message: Message,
): ParentMessage =>
  input.toMessage === undefined
    ? message as unknown as ParentMessage
    : input.toMessage(message)

const focusDialogElement = (id: string, focusSelector: string): void => {
  if (focusSelector.length === 0) {
    return
  }

  const dialogElement = document.getElementById(id)
  const focusTarget = dialogElement?.querySelector(focusSelector)

  if (focusTarget instanceof HTMLElement) {
    focusTarget.focus()
  }
}

export const ShowDialog = defineCommand(
  'BasecoatShowDialog',
  { id: S.String, focusSelector: S.String },
  CompletedShowDialog,
)(({ id, focusSelector }) =>
  Effect.sync(() => {
    if (typeof document === 'undefined') {
      return CompletedShowDialog()
    }

    const element = document.getElementById(id)

    if (element instanceof HTMLDialogElement && !element.open) {
      element.showModal()
    }

    focusDialogElement(id, focusSelector)

    return CompletedShowDialog()
  }),
)

export const CloseDialog = defineCommand(
  'BasecoatCloseDialog',
  { id: S.String },
  CompletedCloseDialog,
)(({ id }) =>
  Effect.sync(() => {
    if (typeof document === 'undefined') {
      return CompletedCloseDialog()
    }

    const element = document.getElementById(id)

    if (element instanceof HTMLDialogElement && element.open) {
      element.close()
    }

    return CompletedCloseDialog()
  }),
)

export const init = (config: InitConfig): Model => ({
  id: config.id,
  isOpen: config.isOpen ?? false,
  ...(config.focusSelector === undefined
    ? {}
    : { focusSelector: config.focusSelector }),
})

export const titleId = (model: Model): string => `${model.id}-title`

export const descriptionId = (model: Model): string =>
  `${model.id}-description`

export const update = (model: Model, message: Message): UpdateResult => {
  switch (message._tag) {
    case 'BasecoatDialogRequestedOpen':
      return model.isOpen
        ? [model, [], null]
        : [
            { ...model, isOpen: true },
            [
              ShowDialog({
                id: model.id,
                focusSelector: model.focusSelector ?? '',
              }),
            ],
            Opened(),
          ]
    case 'BasecoatDialogRequestedClose':
      return model.isOpen
        ? [
            { ...model, isOpen: false },
            [CloseDialog({ id: model.id })],
            Closed(),
          ]
        : [model, [], null]
    case 'BasecoatDialogCompletedShowDialog':
    case 'BasecoatDialogCompletedCloseDialog':
      return [model, [], null]
  }
}

export const open = (model: Model): UpdateResult =>
  update(model, RequestedOpen())

export const close = (model: Model): UpdateResult =>
  update(model, RequestedClose())

export const dialogTrigger = <ParentMessage>(
  input: DialogTriggerProps<ParentMessage>,
): Html => {
  const h = html<ParentMessage>()

  return h.button(
    [
      ...basecoatAttrs<ParentMessage>(input, buttonRoot),
      h.Type(input.type ?? 'button'),
      ...optionalBooleanAttr<ParentMessage>(input.disabled, h.Disabled),
      ...(input.disabled === true
        ? []
        : [h.OnClick(mapMessage<ParentMessage>(input, RequestedOpen()))]),
    ],
    input.children,
  )
}

const closeIcon = <ParentMessage>(): Html => {
  const h = html<ParentMessage>()

  return h.svg(
    [
      h.Xmlns('http://www.w3.org/2000/svg'),
      h.Width('24'),
      h.Height('24'),
      h.ViewBox('0 0 24 24'),
      h.Fill('none'),
      h.Stroke('currentColor'),
      h.StrokeWidth('2'),
      h.StrokeLinecap('round'),
      h.StrokeLinejoin('round'),
      h.Class('lucide lucide-x-icon lucide-x'),
      h.AriaHidden(true),
    ],
    [
      h.path([h.D('M18 6 6 18')], []),
      h.path([h.D('m6 6 12 12')], []),
    ],
  )
}

export const dialogClose = <ParentMessage>(
  input: DialogCloseProps<ParentMessage> = {},
): Html => {
  const h = html<ParentMessage>()

  return h.button(
    [
      ...basecoatAttrs<ParentMessage>(input, buttonRoot),
      h.Type('button'),
      h.DataAttribute('variant', 'ghost'),
      h.DataAttribute('size', 'icon-sm'),
      h.AriaLabel(input.ariaLabel ?? 'Close dialog'),
      ...optionalBooleanAttr<ParentMessage>(input.disabled, h.Disabled),
      ...(input.disabled === true
        ? []
        : [h.OnClick(mapMessage<ParentMessage>(input, RequestedClose()))]),
    ],
    input.children ?? [closeIcon<ParentMessage>()],
  )
}

export const dialogHeader = <ParentMessage>(
  input: DialogHeaderProps<ParentMessage>,
): Html => {
  const h = html<ParentMessage>()

  return h.header(basecoatAttrs<ParentMessage>(input), input.children)
}

export const dialogTitle = <ParentMessage>(
  input: DialogTitleProps<ParentMessage>,
): Html => {
  const h = html<ParentMessage>()

  return h.h2(
    [
      ...basecoatAttrs<ParentMessage>(input),
      h.Id(titleId(input.model)),
    ],
    input.children,
  )
}

export const dialogDescription = <ParentMessage>(
  input: DialogDescriptionProps<ParentMessage>,
): Html => {
  const h = html<ParentMessage>()

  return h.p(
    [
      ...basecoatAttrs<ParentMessage>(input),
      h.Id(descriptionId(input.model)),
    ],
    input.children,
  )
}

export const dialogBody = <ParentMessage>(
  input: DialogBodyProps<ParentMessage>,
): Html => {
  const h = html<ParentMessage>()

  return h.section(basecoatAttrs<ParentMessage>(input), input.children)
}

export const dialogFooter = <ParentMessage>(
  input: DialogFooterProps<ParentMessage>,
): Html => {
  const h = html<ParentMessage>()

  return h.footer(basecoatAttrs<ParentMessage>(input), input.children)
}

export const dialogSurface = <ParentMessage>(
  input: DialogSurfaceProps<ParentMessage>,
): Html => {
  const h = html<ParentMessage>()

  return h.div(basecoatAttrs<ParentMessage>(input), input.children)
}

const dialogElement = <ParentMessage>(
  input: DialogProps<ParentMessage>,
  className: ReturnType<typeof basecoatClass>,
): Html => {
  const h = html<ParentMessage>()

  return h.dialog(
    [
      ...basecoatAttrs<ParentMessage>(input, className),
      h.Id(input.model.id),
      h.Open(input.model.isOpen),
      ...(input.modal === false ? [] : [h.AriaModal(true)]),
      ...optionalStringAttr<ParentMessage>(
        input.labelledBy,
        h.AriaLabelledBy,
      ),
      ...optionalStringAttr<ParentMessage>(
        input.describedBy,
        h.AriaDescribedBy,
      ),
      ...(input.closeOnCancel === false
        ? []
        : [h.OnCancel(mapMessage<ParentMessage>(input, RequestedClose()))]),
      ...dataAttr<ParentMessage>(
        'close-on-overlay-click',
        input.closeOnOverlayClick === false ? undefined : 'true',
      ),
    ],
    input.children,
  )
}

export const dialog = <ParentMessage>(
  input: DialogProps<ParentMessage>,
): Html =>
  dialogElement<ParentMessage>(input, dialogRoot)

export const alertDialog = <ParentMessage>(
  input: DialogProps<ParentMessage>,
): Html =>
  dialogElement<ParentMessage>(
    {
      closeOnCancel: false,
      closeOnOverlayClick: false,
      modal: true,
      ...input,
    },
    alertDialogRoot,
  )

const builtChildren = <ParentMessage>(
  input: DialogViewProps<ParentMessage>,
): BasecoatChildren => {
  const headerChildren = [
    ...(input.title === undefined
      ? []
      : [
          dialogTitle<ParentMessage>({
            model: input.model,
            children: input.title,
          }),
        ]),
    ...(input.description === undefined
      ? []
      : [
          dialogDescription<ParentMessage>({
            model: input.model,
            children: input.description,
          }),
        ]),
  ]

  return [
    dialogSurface<ParentMessage>({
      ...optionalBasecoatProps<ParentMessage>(
        input.surfaceAttrs,
        input.surfaceClassName,
      ),
      children: [
        ...(headerChildren.length === 0
          ? []
          : [
              dialogHeader<ParentMessage>({
                ...optionalBasecoatProps<ParentMessage>(
                  input.headerAttrs,
                  input.headerClassName,
                ),
                children: headerChildren,
              }),
            ]),
        ...(input.body === undefined
          ? []
          : [
              dialogBody<ParentMessage>({
                ...optionalBasecoatProps<ParentMessage>(
                  input.bodyAttrs,
                  input.bodyClassName,
                ),
                children: input.body,
              }),
            ]),
        ...(input.footer === undefined
          ? []
          : [
              dialogFooter<ParentMessage>({
                ...optionalBasecoatProps<ParentMessage>(
                  input.footerAttrs,
                  input.footerClassName,
                ),
                children: input.footer,
              }),
            ]),
        ...(input.closeButton === false
          ? []
          : [
              dialogClose<ParentMessage>({
                ...(input.toMessage === undefined
                  ? {}
                  : { toMessage: input.toMessage }),
              }),
            ]),
      ],
    }),
  ]
}

export const view = <ParentMessage>(
  input: DialogViewProps<ParentMessage>,
): Html =>
  dialog<ParentMessage>({
    ...input,
    ...dialogLabelAttrs(
      input.labelledBy ?? (
        input.title === undefined ? undefined : titleId(input.model)
      ),
      input.describedBy ?? (
        input.description === undefined ? undefined : descriptionId(input.model)
      ),
    ),
    children: builtChildren<ParentMessage>(input),
  })

export const alertDialogView = <ParentMessage>(
  input: DialogViewProps<ParentMessage>,
): Html =>
  alertDialog<ParentMessage>({
    ...input,
    ...dialogLabelAttrs(
      input.labelledBy ?? (
        input.title === undefined ? undefined : titleId(input.model)
      ),
      input.describedBy ?? (
        input.description === undefined ? undefined : descriptionId(input.model)
      ),
    ),
    children: builtChildren<ParentMessage>({ ...input, closeButton: false }),
  })
