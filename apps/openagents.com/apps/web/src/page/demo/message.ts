import { Schema as S } from 'effect'
import { m } from 'foldkit/message'

import * as LoggedIn from '../loggedIn'
import { DemoCue } from './model'

export const GotLoggedInDemoMessage = m('GotLoggedInDemoMessage', {
  message: LoggedIn.Message,
})
export const AdvancedDemoCue = m('AdvancedDemoCue', { cue: DemoCue })
export const ClickedReplayDemo = m('ClickedReplayDemo')
export const ClickedPauseDemo = m('ClickedPauseDemo')
export const ClickedResumeDemo = m('ClickedResumeDemo')
export const ClickedPreviousDemoStep = m('ClickedPreviousDemoStep')
export const ClickedNextDemoStep = m('ClickedNextDemoStep')
export const PressedDemoSpacebar = m('PressedDemoSpacebar')
export const SelectedTrainingSceneNode = m('SelectedTrainingSceneNode', {
  nodeId: S.String,
})
export const TickedDemoPlayback = m('TickedDemoPlayback', { deltaMs: S.Number })

export const Message = S.Union([
  GotLoggedInDemoMessage,
  AdvancedDemoCue,
  ClickedReplayDemo,
  ClickedPauseDemo,
  ClickedResumeDemo,
  ClickedPreviousDemoStep,
  ClickedNextDemoStep,
  PressedDemoSpacebar,
  SelectedTrainingSceneNode,
  TickedDemoPlayback,
])
export type Message = typeof Message.Type
