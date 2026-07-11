import { Schema as S } from "effect"

// NOTE: keep this module free of imports from "./index.js" — index re-exports
// this file (`export *`), so an import back into index would be a circular
// dependency. The `SyncScope`-typed helpers `deviceLocalScope` /
// `isDeviceLocalScope` live in index.ts beside the other scope constructors.

export const LocalIdentityRef = S.String.check(S.isPattern(/^local_[A-Za-z0-9._:-]{4,160}$/)).pipe(S.brand("LocalIdentityRef"))
export type LocalIdentityRef = typeof LocalIdentityRef.Type
export const LocalRevision = S.Number.check(S.isInt(),S.isGreaterThan(0)).pipe(S.brand("LocalRevision"))
export type LocalRevision = typeof LocalRevision.Type
export const LocalIdentityRecord = S.Struct({schemaVersion:S.Literal(1),identityRef:LocalIdentityRef,createdAt:S.String.check(S.isMinLength(1),S.isMaxLength(64))})
export type LocalIdentityRecord=typeof LocalIdentityRecord.Type
export const LocalAccountLink = S.Struct({schemaVersion:S.Literal(1),identityRef:LocalIdentityRef,ownerUserId:S.String.check(S.isMinLength(1),S.isMaxLength(256)),linkedAt:S.String.check(S.isMinLength(1),S.isMaxLength(64)),linkReceiptRef:S.String.check(S.isMinLength(1),S.isMaxLength(256))})
export type LocalAccountLink=typeof LocalAccountLink.Type
