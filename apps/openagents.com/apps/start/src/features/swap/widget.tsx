import {
  Button,
  IntentRef,
  type IntentReporter,
  Stack,
  type View,
} from '@effect-native/core'
import {
  EffectNativeReactDomStyleSheet,
  renderReactDomView,
} from '@effect-native/render-dom/react'
import { type ToneToken, khalaTheme } from '@effect-native/tokens'
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

import type { SwapSettings } from './settings'

export type SwapWidgetSlots = Readonly<{
  assetSelection?: View
  amountEntry?: View
  feePanel?: View
  destinationEntry?: View
  quoteCompare?: View
  rescueCeremony?: View
  sessionStatus?: View
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

const buttonTone = {
  accent: 'accent',
  danger: 'danger',
  neutral: 'secondary',
} satisfies Record<PrimaryActionTone, ToneToken>

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

export const swapWidgetView = (
  model: SwapWidgetRenderModel,
  slots: SwapWidgetSlots = {},
): View => {
  const { primaryAction: action } = resolveSwapWidgetRenderModel(model)
  const children: Array<View> = SLOT_ORDER.flatMap(slot => {
    const child = slots[slot]
    return child === undefined
      ? []
      : [
          Stack({ direction: 'column', key: `swap-widget-slot-${slot}` }, [
            child,
          ]),
        ]
  })

  children.push(
    Button({
      block: true,
      disabled: action.disabled,
      key: 'swap-primary-action',
      label: action.label,
      loading: action.busy,
      onPress: IntentRef('swap.primary_action'),
      size: 'lg',
      style: {
        backgroundColor: action.tone === 'accent' ? 'accent' : 'surface',
        borderColor: action.tone === 'neutral' ? 'border' : action.tone,
        borderRadius: 'none',
        borderWidth: 1,
        color:
          action.tone === 'accent'
            ? 'textInverse'
            : action.tone === 'danger'
              ? 'danger'
              : 'textMuted',
        fontWeight: 'semibold',
        minHeight: 'xs',
        padding: '4',
        typeScale: 'label',
        width: 'full',
      },
      tone: buttonTone[action.tone],
      variant: action.tone === 'accent' ? 'solid' : 'soft',
    }),
  )

  return Stack(
    {
      direction: 'column',
      gap: '4',
      key: 'swap-widget-content',
      padding: '6',
      style: {
        backgroundColor: 'surface',
        borderColor: 'border',
        borderWidth: 1,
      },
    },
    children,
  )
}

export function SwapWidget({
  model,
  report,
  slots,
}: Readonly<{
  model: SwapWidgetRenderModel
  report: IntentReporter
  slots?: SwapWidgetSlots
}>) {
  const resolved = resolveSwapWidgetRenderModel(model)
  return (
    <section
      data-effect-native-surface="dom"
      data-swap-widget=""
      data-swap-widget-state={resolved.widgetState._tag}
      style={desktopThemeCssVariables(khalaTheme)}
    >
      <EffectNativeReactDomStyleSheet theme={khalaTheme} />
      {renderReactDomView(swapWidgetView(model, slots), {
        report,
        theme: khalaTheme,
      })}
    </section>
  )
}
