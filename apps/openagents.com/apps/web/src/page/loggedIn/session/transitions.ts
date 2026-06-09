import { Match as M, Option } from 'effect'
import { evo } from 'foldkit/struct'

import {
  Message,
  RequestedLogout,
} from '../message'
import {
  FailedInviteCodeAction,
  IdleInviteCodeAction,
  Model,
} from '../model'
import {
  noUpdate,
  type UpdateReturn,
} from '../transition'

const withUpdateReturn = M.withReturnType<UpdateReturn>()

export const updateSessionChrome = (
  model: Model,
  message: Message,
): UpdateReturn =>
  M.value(message).pipe(
    withUpdateReturn,
    M.tags({
      ClickedLogout: () => [model, [], Option.some(RequestedLogout())],
      UpdatedInviteCode: ({ value }) => [
        evo(model, {
          inviteCodeAction: () => IdleInviteCodeAction(),
          inviteCodeValue: () => value,
        }),
        [],
        Option.none(),
      ],
      SubmittedInviteCode: () => [
        evo(model, {
          inviteCodeAction: () =>
            FailedInviteCodeAction({ error: 'Invalid invite code.' }),
        }),
        [],
        Option.none(),
      ],
      UpdatedChatComposer: ({ value }) => [
        evo(model, { chatComposerValue: () => value }),
        [],
        Option.none(),
      ],
      CompletedSetAutopilotThreadUrl: () => noUpdate(model),
      CompletedInstallAccountMenuOutsideClick: () => noUpdate(model),
      CompletedScrollChatTimelineToEnd: () => noUpdate(model),
      CompletedFocusChatComposer: () => noUpdate(model),
    }),
    M.orElse(() => noUpdate(model)),
  )
