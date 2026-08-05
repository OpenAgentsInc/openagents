import { khalaTheme } from '@openagentsinc/design-tokens'
import {
  type SwapSessionViewModel,
  swapSessionViewModel,
} from '@openagentsinc/mkt-swp'
import {
  type PrimaryActionModel,
  type PrimaryActionTone,
  type SwapWidgetState,
  derivePrimaryAction,
} from '@openagentsinc/mkt-swp/view'
import type { Catalog } from '@openagentsinc/swap-i18n'
import { desktopThemeCssVariables } from '@openagentsinc/ui/desktop-workbench'
import { Data } from 'effect'
import type { ReactNode } from 'react'

import type { SwapSettings } from './settings'

/**
 * The seams the sibling SWAP packages fill. A slot is ordinary React content
 * rendered inside the widget's composition order; the widget owns the order
 * and the primary action, never the slot's internals.
 */
export type SwapWidgetSlots = Readonly<{
  assetSelection?: ReactNode
  amountEntry?: ReactNode
  feePanel?: ReactNode
  destinationEntry?: ReactNode
  quoteCompare?: ReactNode
  rescueCeremony?: ReactNode
  sessionStatus?: ReactNode
}>

const SLOT_ORDER = [
  'assetSelection',
  'amountEntry',
  'feePanel',
  'destinationEntry',
  'quoteCompare',
  'rescueCeremony',
  'sessionStatus',
] as const satisfies ReadonlyArray<keyof SwapWidgetSlots>

/**
 * Tone is one of the four independent mechanisms of the primary-action law
 * (label / tone / disabled / busy). It only ever selects colour: accent is
 * the solid actionable control, danger the refusal, neutral everything else.
 * The palette values are the Khala theme's `accent`, `danger`, `surface`,
 * `border`, `textInverse`, and `textMuted`.
 */
const primaryActionToneClass = {
  accent: 'border-khala-energy bg-khala-energy text-khala-void',
  danger: 'border-khala-danger bg-khala-surface text-khala-danger',
  neutral: 'border-khala-border bg-khala-surface text-khala-text-muted',
} satisfies Record<PrimaryActionTone, string>

export type SwapWidgetRenderModel = Data.TaggedEnum<{
  PreSession: {
    readonly widgetState: SwapWidgetState
    readonly primaryAction: PrimaryActionModel
  }
  Session: { readonly viewModel: SwapSessionViewModel }
}>
export const SwapWidgetRenderModel = Data.taggedEnum<SwapWidgetRenderModel>()

export type ResolvedSwapWidgetRenderModel = Readonly<
  Pick<
    SwapSessionViewModel,
    'widgetState' | 'primaryAction' | 'fundingGate' | 'progress'
  >
>

export const resolveSwapWidgetRenderModel = (
  model: SwapWidgetRenderModel,
): ResolvedSwapWidgetRenderModel =>
  SwapWidgetRenderModel.$match(model, {
    PreSession: ({ widgetState, primaryAction }) => ({
      widgetState,
      primaryAction,
      fundingGate: null,
      progress: null,
    }),
    Session: ({ viewModel }) => {
      const resolved = swapSessionViewModel(viewModel)
      return {
        widgetState: resolved.widgetState,
        primaryAction: resolved.primaryAction,
        fundingGate: resolved.fundingGate,
        progress: resolved.progress,
      }
    },
  })

/**
 * The disconnected page still derives a pre-session action locally until
 * Immortal's generated ABI supplies the engine-owned session model.
 */
export const provisionalPreSessionSwapWidgetModel = (
  catalog: Catalog,
  settings: SwapSettings,
  widgetState: SwapWidgetState,
): SwapWidgetRenderModel =>
  SwapWidgetRenderModel.PreSession({
    widgetState,
    primaryAction: derivePrimaryAction(
      widgetState,
      catalog,
      settings.denomination,
      settings.decimalSeparator,
    ),
  })

/**
 * The primary action. Exactly one is rendered, always: `derivePrimaryAction`
 * (or the engine-owned session view model) decides the label, the tone, and
 * — independently of the label — whether it is blocked and whether it is
 * busy. This component decides nothing; it lowers that decision to a real
 * `<button>`. The busy spinner is decoration beside the label, not a
 * replacement for it, so the reason the control cannot proceed stays legible
 * and stays in the accessibility tree.
 */
function SwapPrimaryAction({
  action,
  onPress,
}: Readonly<{
  action: PrimaryActionModel
  onPress?: (() => void) | undefined
}>) {
  const blocked = action.disabled || action.busy
  return (
    <button
      {...(action.busy ? { 'aria-busy': true } : {})}
      className={[
        'khala-focus flex w-full items-center justify-center gap-2 border p-4 font-mono text-sm font-semibold',
        'disabled:cursor-not-allowed disabled:opacity-50',
        action.busy ? 'cursor-wait' : '',
        primaryActionToneClass[action.tone],
      ]
        .filter(part => part !== '')
        .join(' ')}
      data-swap-primary-action=""
      data-swap-primary-action-busy={String(action.busy)}
      data-swap-primary-action-tone={action.tone}
      disabled={blocked}
      onClick={onPress}
      type="button"
    >
      {action.busy ? (
        <span
          aria-hidden="true"
          className="size-4 shrink-0 animate-spin rounded-full border-2 border-current border-r-transparent border-b-transparent"
        />
      ) : null}
      {action.label}
    </button>
  )
}

/**
 * The widget's composition order: every filled slot in `SLOT_ORDER`, then the
 * primary action last.
 */
export function SwapWidgetContent({
  model,
  onPrimaryAction,
  slots = {},
}: Readonly<{
  model: SwapWidgetRenderModel
  onPrimaryAction?: (() => void) | undefined
  slots?: SwapWidgetSlots | undefined
}>) {
  const { primaryAction } = resolveSwapWidgetRenderModel(model)
  return (
    <div
      className="flex flex-col gap-4 border border-khala-border bg-khala-surface p-6"
      data-swap-widget-content=""
    >
      {SLOT_ORDER.map(slot => {
        const child = slots[slot]
        return child === undefined ? null : (
          <div className="flex flex-col" data-swap-widget-slot={slot} key={slot}>
            {child}
          </div>
        )
      })}
      <SwapPrimaryAction action={primaryAction} onPress={onPrimaryAction} />
    </div>
  )
}

// Converted from an Effect Native view tree to plain React (#9325). /swap is
// a page people move money on, so its host is the DOM directly: real buttons,
// real labels, real focus order.
export function SwapWidget({
  model,
  onPrimaryAction,
  slots,
}: Readonly<{
  model: SwapWidgetRenderModel
  onPrimaryAction?: (() => void) | undefined
  slots?: SwapWidgetSlots | undefined
}>) {
  const resolved = resolveSwapWidgetRenderModel(model)
  return (
    <section
      data-swap-widget=""
      data-swap-widget-state={resolved.widgetState._tag}
      style={desktopThemeCssVariables(khalaTheme)}
    >
      <SwapWidgetContent
        model={model}
        onPrimaryAction={onPrimaryAction}
        slots={slots}
      />
    </section>
  )
}
