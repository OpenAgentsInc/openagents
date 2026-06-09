import { Effect, Schema as S } from 'effect'
import { Command, Dom } from 'foldkit'
import { pushUrl } from 'foldkit/navigation'

import { threadRouter } from '../../../route'
import { CHAT_PROMPT_ID } from '../chatDom'
import {
  CompletedFocusChatComposer,
  CompletedInstallAccountMenuOutsideClick,
  CompletedScrollChatTimelineToEnd,
  CompletedSetAutopilotThreadUrl,
} from '../message'
import { errorFromUnknown } from './api'

export const ScrollChatTimelineToEnd = Command.define(
  'ScrollChatTimelineToEnd',
  CompletedScrollChatTimelineToEnd,
)(
  Dom.scrollIntoViewAfterPaint('[data-chat-timeline-end="true"]', {
    block: 'end',
  }).pipe(
    Effect.as(CompletedScrollChatTimelineToEnd()),
    Effect.catch(() => Effect.succeed(CompletedScrollChatTimelineToEnd())),
  ),
)

export const FocusChatComposer = Command.define(
  'FocusChatComposer',
  CompletedFocusChatComposer,
)(
  Dom.focus(`#${CHAT_PROMPT_ID}`).pipe(
    Effect.ignore,
    Effect.as(CompletedFocusChatComposer()),
  ),
)

export const SetAutopilotThreadUrl = Command.define(
  'SetAutopilotThreadUrl',
  { threadId: S.String },
  CompletedSetAutopilotThreadUrl,
)(({ threadId }) =>
  pushUrl(threadRouter({ threadId })).pipe(
    Effect.as(CompletedSetAutopilotThreadUrl()),
  ),
)

export const InstallAccountMenuOutsideClick = Command.define(
  'InstallAccountMenuOutsideClick',
  CompletedInstallAccountMenuOutsideClick,
)(
  Effect.try({
    try: () => {
      if (typeof document === 'undefined') {
        return
      }

      const root = document.documentElement

      if (root.dataset.accountMenuOutsideClick === 'installed') {
        return
      }

      root.dataset.accountMenuOutsideClick = 'installed'
      document.addEventListener(
        'click',
        event => {
          const target = event.target

          if (!(target instanceof Node)) {
            return
          }

          document
            .querySelectorAll<HTMLDetailsElement>(
              '[data-component="account-menu"][open]',
            )
            .forEach(menu => {
              if (!menu.contains(target)) {
                menu.open = false
              }
            })
        },
        true,
      )
    },
    catch: errorFromUnknown,
  }).pipe(
    Effect.as(CompletedInstallAccountMenuOutsideClick()),
    Effect.catch(() =>
      Effect.succeed(CompletedInstallAccountMenuOutsideClick()),
    ),
  ),
)
