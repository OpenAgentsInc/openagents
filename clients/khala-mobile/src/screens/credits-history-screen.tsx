import { useNavigation } from "@react-navigation/native"
import { useEffect, useMemo, useRef, useState } from "react"
import { SafeAreaView } from "react-native-safe-area-context"

import { useKhalaAuth } from "../auth/khala-auth-context"
import { EffectNativeHost } from "../effect-native/effect-native-host"
import { khalaEffectNativeTheme } from "../effect-native/khala-effect-native-theme"
import { khalaMobileTheme } from "../theme/tokens"
import {
  fetchKhalaMobileCreditsTransactions,
  type KhalaMobileCreditsTransaction,
} from "../sync/khala-mobile-credits-api"
import {
  buildCreditsHistoryProgram,
  type CreditsHistoryCallbacks,
  type CreditsHistoryViewModel,
  initialCreditsHistoryViewModel,
  renderCreditsHistoryView,
} from "./credits-history-effect-native-core"

type LoadState =
  | Readonly<{ status: "loading" }>
  | Readonly<{ status: "unavailable" }>
  | Readonly<{ status: "error" }>
  | Readonly<{
      status: "ready"
      nextCursor: string | null
      transactions: ReadonlyArray<KhalaMobileCreditsTransaction>
    }>

const toViewModel = (state: LoadState): CreditsHistoryViewModel =>
  state.status === "ready"
    ? { status: "ready", hasMore: state.nextCursor !== null, transactions: state.transactions }
    : { status: state.status }

/**
 * MB-EN (#8597): transaction history for the mobile credits balance, re-authored
 * with the Effect Native component set and rendered through
 * `@effect-native/render-rn` on iOS + Android (the first data-driven Khala-mobile
 * screen converted off the ported Ignite RN primitives). Its INTERNAL UI is a
 * typed `View` tree (see `credits-history-effect-native-core.ts`); this shell
 * keeps owning the DATA and NAV as services — the same `khala-mobile-credits-api`
 * endpoint contract, and the same honest 404/network degradation to a "not yet
 * available"/"unavailable" screen rather than an empty or fabricated list.
 */
export const CreditsHistoryScreen = () => {
  const { baseUrl, token } = useKhalaAuth()
  const navigation = useNavigation()
  const [state, setState] = useState<LoadState>({ status: "loading" })

  const loadPage = async (cursor: string | undefined, append: boolean) => {
    if (token === "") {
      setState({ status: "error" })
      return
    }
    const result = await fetchKhalaMobileCreditsTransactions(baseUrl, token, { cursor, limit: 50 })
    if (!result.ok) {
      setState(result.kind === "unavailable" ? { status: "unavailable" } : { status: "error" })
      return
    }
    setState(previous => {
      const priorTransactions = append && previous.status === "ready" ? previous.transactions : []
      return {
        nextCursor: result.value.nextCursor,
        status: "ready",
        transactions: [...priorTransactions, ...result.value.transactions],
      }
    })
  }

  useEffect(() => {
    void loadPage(undefined, false)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  // Stable callbacks object the Effect Native intent handlers dispatch into.
  // Mutated each render so Back / Load more always run against current state.
  const callbacks = useMemo<CreditsHistoryCallbacks>(
    () => ({ onBack: () => {}, onLoadMore: () => {} }),
    [],
  )
  const stateRef = useRef<LoadState>(state)
  stateRef.current = state
  callbacks.onBack = () => {
    if (navigation.canGoBack()) navigation.goBack()
  }
  callbacks.onLoadMore = () => {
    const current = stateRef.current
    if (current.status === "ready" && current.nextCursor !== null) {
      void loadPage(current.nextCursor, true)
    }
  }

  // Build the Effect Native program (state ref + intent registry + reporter)
  // once per mount, then push each derived view-model into it as data loads.
  const program = useMemo(() => buildCreditsHistoryProgram(callbacks), [callbacks])
  useEffect(() => {
    program.setViewModel(toViewModel(state))
  }, [program, state])

  return (
    <SafeAreaView
      edges={["top"]}
      style={{ flex: 1, backgroundColor: khalaMobileTheme.background }}
    >
      <EffectNativeHost
        viewStream={program.viewStream}
        report={program.report}
        theme={khalaEffectNativeTheme}
        platform="ios"
        initialView={renderCreditsHistoryView(initialCreditsHistoryViewModel)}
      />
    </SafeAreaView>
  )
}
