import { Option } from 'effect'
import { Command } from 'foldkit'

import { Message, type OutMessage } from './message'
import { Model } from './model'

export type UpdateReturn = readonly [
  Model,
  ReadonlyArray<Command.Command<Message>>,
  Option.Option<OutMessage>,
]

export const noUpdate = (model: Model): UpdateReturn => [
  model,
  [],
  Option.none(),
]
