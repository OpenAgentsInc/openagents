import { Schema as S } from 'effect'

import { Demo, LoggedIn, LoggedOut } from './page'

export const Model = S.Union([LoggedOut.Model, LoggedIn.Model, Demo.Model])

export type Model = typeof Model.Type

export { Demo, LoggedOut, LoggedIn }
