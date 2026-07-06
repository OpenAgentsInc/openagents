import { Schema as S } from 'effect'
import { createSubjects } from '@openauthjs/openauth/subject'

/**
 * Mirrors the `user` subject shape minted by the shared OpenAuth issuer in
 * `apps/openagents.com/workers/api/src/index.ts` (`UserSubject`). Aiur does
 * not run its own issuer — it is a CLIENT of the same
 * `auth.openagents.com` issuer — so this schema only needs to decode the
 * subject properties the issuer already signs; it must stay
 * field-for-field compatible with the upstream shape or `client.verify`
 * will fail to decode a legitimately signed token.
 */
const TrimmedString = S.Trim
const NonEmptyTrimmedString = TrimmedString.check(S.isNonEmpty())
const SIMPLE_EMAIL_PATTERN = /^[^\s@]+@[^\s@]+\.[^\s@]+$/
const EmailString = NonEmptyTrimmedString.check(S.isPattern(SIMPLE_EMAIL_PATTERN))

export const AiurUserSubject = S.Struct({
  userId: NonEmptyTrimmedString,
  provider: S.Literals(['github', 'email']),
  githubId: S.optionalKey(NonEmptyTrimmedString),
  login: S.optionalKey(NonEmptyTrimmedString),
  email: EmailString,
  name: NonEmptyTrimmedString,
  avatarUrl: S.String,
})

export type AiurUserSubject = typeof AiurUserSubject.Type

export const aiurSubjects = createSubjects({
  user: S.toStandardSchemaV1(AiurUserSubject),
})
