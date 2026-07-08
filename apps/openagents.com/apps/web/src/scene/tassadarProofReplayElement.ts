import type { Attribute, Html } from 'foldkit/html'
import { html } from 'foldkit/html'

export const OPENAGENTS_PUBLIC_ORIGIN = 'https://openagents.com'
export const TASSADAR_REPLAY_ORIGIN_DATA_KEY = 'tassadar-replay-origin'
export const TASSADAR_REPLAY_SLUG_DATA_KEY = 'tassadar-replay-slug'

export type ArchivedProofReplayBundle = Readonly<Record<string, unknown>>

export const tassadarProofReplayView = <Message>(
  attributes: ReadonlyArray<Attribute<Message>>,
  _bundle?: ArchivedProofReplayBundle | null,
): Html => {
  const h = html<Message>()
  return h.div(attributes, [
    h.div([h.Class('proof-replay-archived')], [
      h.p([], ['Proof replay archived']),
      h.p([], ['The retired Tassadar/Psionic replay bundle is preserved in backroom.']),
    ]),
  ])
}
