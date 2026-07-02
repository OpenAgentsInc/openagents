import { Schema as S } from "effect"
import { m } from "foldkit/message"

import { KhalaCodeFoldkitHostPortMessage } from "./ports.js"

export const FoldkitDemoClickedPing = m("FoldkitDemoClickedPing")
export const FoldkitDemoReceivedHostPort = m("FoldkitDemoReceivedHostPort", {
  message: KhalaCodeFoldkitHostPortMessage,
})
export const FoldkitDemoMounted = m("FoldkitDemoMounted")
export const FoldkitDemoUnmounted = m("FoldkitDemoUnmounted")
export const FoldkitDemoCompletedPortEmit = m("FoldkitDemoCompletedPortEmit")

export const KhalaCodeFoldkitMessage = S.Union([
  FoldkitDemoClickedPing,
  FoldkitDemoReceivedHostPort,
  FoldkitDemoMounted,
  FoldkitDemoUnmounted,
  FoldkitDemoCompletedPortEmit,
])
export type KhalaCodeFoldkitMessage = typeof KhalaCodeFoldkitMessage.Type
