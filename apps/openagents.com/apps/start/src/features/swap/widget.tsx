import {
  Button,
  IntentRef,
  type IntentReporter,
  Stack,
  type View,
} from '@effect-native/core'
import { renderReactDomView } from '@effect-native/render-dom/react'
import { type ToneToken, khalaTheme } from '@effect-native/tokens'
import {
  type PrimaryActionTone,
  type SwapWidgetState,
  derivePrimaryAction,
} from '@openagentsinc/mkt-swp/view'
import type { Catalog } from '@openagentsinc/swap-i18n'
import { desktopThemeCssVariables } from '@openagentsinc/ui/desktop-workbench'

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

export const swapWidgetView = (
  catalog: Catalog,
  settings: SwapSettings,
  state: SwapWidgetState,
  slots: SwapWidgetSlots = {},
): View => {
  const action = derivePrimaryAction(
    state,
    catalog,
    settings.denomination,
    settings.decimalSeparator,
  )
  const children: View[] = SLOT_ORDER.flatMap(slot => {
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
  catalog,
  report,
  settings,
  slots,
  state,
}: Readonly<{
  catalog: Catalog
  report: IntentReporter
  settings: SwapSettings
  slots?: SwapWidgetSlots
  state: SwapWidgetState
}>) {
  return (
    <section
      data-effect-native-surface="dom"
      data-swap-widget=""
      data-swap-widget-state={state._tag}
      style={desktopThemeCssVariables(khalaTheme)}
    >
      {renderReactDomView(swapWidgetView(catalog, settings, state, slots), {
        report,
        theme: khalaTheme,
      })}
    </section>
  )
}
