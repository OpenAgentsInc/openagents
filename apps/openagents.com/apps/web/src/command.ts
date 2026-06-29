import { Console, Effect, Schema as S } from 'effect'
import { Command } from 'foldkit'

import { CompletedLogError } from './message'

export { ClearSession } from './commands/session'

export const LogError = Command.define(
  'LogError',
  { entries: S.Array(S.Unknown) },
  CompletedLogError,
)(({ entries }) =>
  Console.error(...entries).pipe(Effect.as(CompletedLogError())),
)
