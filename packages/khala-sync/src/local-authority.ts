import { Schema as S } from "effect"
import type { SyncScope } from "./index.js"

export const LocalIdentityRef = S.String.check(S.isPattern(/^local_[A-Za-z0-9._:-]{4,160}$/)).pipe(S.brand("LocalIdentityRef"))
export type LocalIdentityRef = typeof LocalIdentityRef.Type
export const LocalRevision = S.Number.check(S.isInt(),S.isGreaterThan(0)).pipe(S.brand("LocalRevision"))
export type LocalRevision = typeof LocalRevision.Type
export const LocalIdentityRecord = S.Struct({schemaVersion:S.Literal(1),identityRef:LocalIdentityRef,createdAt:S.String.check(S.isMinLength(1),S.isMaxLength(64))})
export type LocalIdentityRecord=typeof LocalIdentityRecord.Type
export const LocalAccountLink = S.Struct({schemaVersion:S.Literal(1),identityRef:LocalIdentityRef,ownerUserId:S.String.check(S.isMinLength(1),S.isMaxLength(256)),linkedAt:S.String.check(S.isMinLength(1),S.isMaxLength(64)),linkReceiptRef:S.String.check(S.isMinLength(1),S.isMaxLength(256))})
export type LocalAccountLink=typeof LocalAccountLink.Type
export const deviceLocalScope=(identityRef:LocalIdentityRef):SyncScope=>`scope.device_local.${identityRef}` as SyncScope
export const isDeviceLocalScope=(scope:SyncScope):boolean=>String(scope).startsWith("scope.device_local.")
