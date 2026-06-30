import { Option } from 'effect'
import type { Attribute, Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import {
  basecoatAttrs,
  basecoatClass,
  dataAttr,
  type BasecoatAttrs,
} from './shared'

export type RangeStep = number | 'any'

export type RangeKey =
  | 'ArrowDown'
  | 'ArrowLeft'
  | 'ArrowRight'
  | 'ArrowUp'
  | 'End'
  | 'Home'
  | 'PageDown'
  | 'PageUp'

export type RangeModel = Readonly<{
  min: number
  max: number
  step: RangeStep
  value: number
  focused: boolean
  dragging: boolean
}>

export type RangeInit = Readonly<{
  min?: number
  max?: number
  step?: RangeStep
  value?: number | string | null
  defaultValue?: number | string | null
  focused?: boolean
  dragging?: boolean
}>

export type RangeMessage =
  | Readonly<{ _tag: 'RangeValueChanged'; value: number | string }>
  | Readonly<{ _tag: 'RangeFocused' }>
  | Readonly<{ _tag: 'RangeBlurred' }>
  | Readonly<{ _tag: 'RangeDragStarted' }>
  | Readonly<{ _tag: 'RangeDragEnded' }>
  | Readonly<{ _tag: 'RangeKeyDown'; key: RangeKey }>

export type RangeProps<Message> = BasecoatAttrs<Message> & Readonly<{
  model: RangeModel
  id?: string
  name?: string
  disabled?: boolean
  required?: boolean
  ariaLabel?: string
  labelledBy?: string
  describedBy?: string
  valueText?: string
  toMessage?: (message: RangeMessage) => Message
}>

export type RangeViewProps<Message> = RangeProps<Message> & Readonly<{
  toMessage: (message: RangeMessage) => Message
}>

const inputRoot = basecoatClass('input')

const optionalStringAttr = <Message>(
  value: string | undefined,
  attr: (value: string) => Attribute<Message>,
): ReadonlyArray<Attribute<Message>> => value === undefined ? [] : [attr(value)]

const optionalBooleanAttr = <Message>(
  enabled: boolean | undefined,
  attr: (value: true) => Attribute<Message>,
): ReadonlyArray<Attribute<Message>> => enabled === true ? [attr(true)] : []

const mappedAttr = <Message>(
  toMessage: ((message: RangeMessage) => Message) | undefined,
  message: RangeMessage,
  attr: (message: Message) => Attribute<Message>,
): ReadonlyArray<Attribute<Message>> =>
  toMessage === undefined ? [] : [attr(toMessage(message))]

const numberFrom = (
  value: number | string | null | undefined,
): number | null => {
  if (value === null || value === undefined || value === '') {
    return null
  }

  const parsed = typeof value === 'number' ? value : Number.parseFloat(value)
  return Number.isFinite(parsed) ? parsed : null
}

const orderedBounds = (
  min: number | undefined,
  max: number | undefined,
): Readonly<{ min: number; max: number }> => {
  const lower = Number.isFinite(min) ? min as number : 0
  const upper = Number.isFinite(max) ? max as number : 100

  return lower <= upper
    ? { min: lower, max: upper }
    : { min: upper, max: lower }
}

const normalizedStep = (step: RangeStep | undefined): RangeStep =>
  step === 'any'
    ? 'any'
    : typeof step === 'number' && Number.isFinite(step) && step > 0
      ? step
      : 1

const clamp = (value: number, min: number, max: number): number =>
  Math.min(max, Math.max(min, value))

const decimalPlaces = (value: number): number => {
  const text = String(value)
  const exponent = text.toLowerCase().split('e-')[1]
  if (exponent !== undefined) {
    return Number.parseInt(exponent, 10)
  }

  return text.includes('.') ? text.split('.')[1]?.length ?? 0 : 0
}

const normalizeValue = (
  value: number,
  min: number,
  max: number,
  step: RangeStep,
): number => {
  const bounded = clamp(value, min, max)
  if (step === 'any') {
    return bounded
  }

  const stepped = min + Math.round((bounded - min) / step) * step
  const precision = Math.max(decimalPlaces(min), decimalPlaces(step))

  return clamp(Number(stepped.toFixed(precision)), min, max)
}

const keyboardStep = (model: RangeModel): number =>
  model.step === 'any' ? 1 : model.step

const applyDelta = (
  model: RangeModel,
  delta: number,
): RangeModel => ({
  ...model,
  value: normalizeValue(model.value + delta, model.min, model.max, model.step),
})

export const rangeInit = (input: RangeInit = {}): RangeModel => {
  const bounds = orderedBounds(input.min, input.max)
  const step = normalizedStep(input.step)
  const value = numberFrom(input.value ?? input.defaultValue) ?? bounds.min

  return {
    ...bounds,
    step,
    value: normalizeValue(value, bounds.min, bounds.max, step),
    focused: input.focused === true,
    dragging: input.dragging === true,
  }
}

export const rangeValueChanged = (
  value: number | string,
): RangeMessage => ({ _tag: 'RangeValueChanged', value })

export const rangeFocused = (): RangeMessage => ({ _tag: 'RangeFocused' })
export const rangeBlurred = (): RangeMessage => ({ _tag: 'RangeBlurred' })
export const rangeDragStarted = (): RangeMessage => ({ _tag: 'RangeDragStarted' })
export const rangeDragEnded = (): RangeMessage => ({ _tag: 'RangeDragEnded' })
export const rangeKeyDown = (key: RangeKey): RangeMessage => ({
  _tag: 'RangeKeyDown',
  key,
})

export const rangeUpdate = (
  model: RangeModel,
  message: RangeMessage,
): RangeModel => {
  switch (message._tag) {
    case 'RangeValueChanged': {
      const value = numberFrom(message.value)
      return value === null
        ? model
        : {
            ...model,
            value: normalizeValue(value, model.min, model.max, model.step),
          }
    }
    case 'RangeFocused':
      return { ...model, focused: true }
    case 'RangeBlurred':
      return { ...model, focused: false, dragging: false }
    case 'RangeDragStarted':
      return { ...model, dragging: true, focused: true }
    case 'RangeDragEnded':
      return { ...model, dragging: false }
    case 'RangeKeyDown': {
      const step = keyboardStep(model)
      const pageStep = step * 10

      switch (message.key) {
        case 'ArrowDown':
        case 'ArrowLeft':
          return applyDelta(model, -step)
        case 'ArrowRight':
        case 'ArrowUp':
          return applyDelta(model, step)
        case 'PageDown':
          return applyDelta(model, -pageStep)
        case 'PageUp':
          return applyDelta(model, pageStep)
        case 'Home':
          return { ...model, value: model.min }
        case 'End':
          return { ...model, value: model.max }
      }
    }
  }
}

export const rangePercent = (model: RangeModel): number =>
  model.max === model.min
    ? 0
    : clamp(((model.value - model.min) / (model.max - model.min)) * 100, 0, 100)

export const rangeValue = (model: RangeModel): number => model.value

const handledRangeKeys = new Set<RangeKey>([
  'ArrowDown',
  'ArrowLeft',
  'ArrowRight',
  'ArrowUp',
  'End',
  'Home',
  'PageDown',
  'PageUp',
])

const mappedKeydownAttr = <Message>(
  toMessage: ((message: RangeMessage) => Message) | undefined,
): ReadonlyArray<Attribute<Message>> => {
  if (toMessage === undefined) {
    return []
  }

  const h = html<Message>()

  return [
    h.OnKeyDownPreventDefault(key =>
      handledRangeKeys.has(key as RangeKey)
        ? Option.some(toMessage(rangeKeyDown(key as RangeKey)))
        : Option.none(),
    ),
  ]
}

const mappedPointerDownAttr = <Message>(
  toMessage: ((message: RangeMessage) => Message) | undefined,
): ReadonlyArray<Attribute<Message>> => {
  if (toMessage === undefined) {
    return []
  }

  const h = html<Message>()

  return [
    h.OnPointerDown(() => Option.some(toMessage(rangeDragStarted()))),
    h.OnPointerUp(() => Option.some(toMessage(rangeDragEnded()))),
  ]
}

export const range = <Message>(input: RangeProps<Message>): Html => {
  const h = html<Message>()
  const percent = rangePercent(input.model)

  return h.input([
    ...basecoatAttrs<Message>(input, inputRoot),
    h.Type('range'),
    h.Value(String(input.model.value)),
    h.Min(String(input.model.min)),
    h.Max(String(input.model.max)),
    h.Step(input.model.step === 'any' ? 'any' : String(input.model.step)),
    h.Style({ '--slider-value': `${percent}%` }),
    h.DataAttribute('range-initialized', 'true'),
    h.AriaValuemin(input.model.min),
    h.AriaValuemax(input.model.max),
    h.AriaValuenow(input.model.value),
    ...dataAttr<Message>('focused', input.model.focused ? 'true' : undefined),
    ...dataAttr<Message>('dragging', input.model.dragging ? 'true' : undefined),
    ...optionalStringAttr<Message>(input.id, h.Id),
    ...optionalStringAttr<Message>(input.name, h.Name),
    ...optionalStringAttr<Message>(input.ariaLabel, h.AriaLabel),
    ...optionalStringAttr<Message>(input.labelledBy, h.AriaLabelledBy),
    ...optionalStringAttr<Message>(input.describedBy, h.AriaDescribedBy),
    ...optionalStringAttr<Message>(input.valueText, h.AriaValuetext),
    ...optionalBooleanAttr<Message>(input.disabled, h.Disabled),
    ...optionalBooleanAttr<Message>(input.required, h.Required),
    ...(input.disabled === true
      ? []
      : [
          ...(input.toMessage === undefined
            ? []
            : [h.OnInput(value => input.toMessage?.(rangeValueChanged(value)) as Message)]),
          ...mappedAttr<Message>(input.toMessage, rangeFocused(), h.OnFocus),
          ...mappedAttr<Message>(input.toMessage, rangeBlurred(), h.OnBlur),
          ...mappedKeydownAttr<Message>(input.toMessage),
          ...mappedPointerDownAttr<Message>(input.toMessage),
        ]),
  ])
}

export const rangeView = range
