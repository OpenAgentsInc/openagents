import { SyncSnapshot } from '@openagentsinc/sync-schema'
import { Effect, Schema as S } from 'effect'
import { Command } from 'foldkit'

import {
  errorMessageFromUnknown,
  requestJson,
} from '../commands/api'
import {
  FailedLoadSyncSnapshot,
  SucceededLoadSyncSnapshot,
} from '../message'

export const LoadSyncSnapshot = Command.define(
  'LoadSyncSnapshot',
  { href: S.String, scope: S.String },
  SucceededLoadSyncSnapshot,
  FailedLoadSyncSnapshot,
)(({ href, scope }) =>
  Effect.gen(function* () {
    const snapshot = yield* requestJson({
      init: {
        cache: 'no-store',
        credentials: 'include',
        headers: { accept: 'application/json' },
      },
      name: 'loggedIn.sync.snapshot.load',
      request: href,
      schema: SyncSnapshot,
    })

    return SucceededLoadSyncSnapshot({ scope, snapshot })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedLoadSyncSnapshot({
          error: errorMessageFromUnknown(error),
          scope,
        }),
      ),
    ),
  ),
)
