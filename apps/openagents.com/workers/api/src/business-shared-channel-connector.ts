import { Schema as S } from 'effect'

import { currentIsoTimestamp } from './runtime-primitives'

export const BUSINESS_SHARED_CHANNEL_CONNECTOR_RECEIPT_SCHEMA =
  'openagents.business.shared_channel_connector.receipt.v1' as const

export const BusinessSharedChannelPlatform = S.Literals([
  'slack',
  'discord',
  'microsoft_teams',
])
export type BusinessSharedChannelPlatform =
  typeof BusinessSharedChannelPlatform.Type

export const BusinessSharedChannelCommandScope = S.Literals([
  'engagement_reply_draft',
])
export type BusinessSharedChannelCommandScope =
  typeof BusinessSharedChannelCommandScope.Type

export const BusinessSharedChannelConnectorReceipt = S.Struct({
  schema: S.Literal(BUSINESS_SHARED_CHANNEL_CONNECTOR_RECEIPT_SCHEMA),
  receiptKind: S.Literal('business.shared_channel_connector.connected_run'),
  receiptRef: S.String,
  engagementRef: S.String,
  workspaceRef: S.String,
  platform: BusinessSharedChannelPlatform,
  connectorRef: S.String,
  channelRef: S.String,
  inviteRequestRef: S.String,
  inviteCreatedRef: S.String,
  requestedByActorRef: S.String,
  mentionRef: S.String,
  commandRef: S.String,
  commandScope: BusinessSharedChannelCommandScope,
  commandVerified: S.Literal(true),
  replyDraftRef: S.String,
  outboundAllowed: S.Literal(false),
  autoInviteAllowed: S.Literal(false),
  externalSendAuthorized: S.Literal(false),
  createdAt: S.String,
  sourceRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
})
export type BusinessSharedChannelConnectorReceipt =
  typeof BusinessSharedChannelConnectorReceipt.Type

export type BusinessSharedChannelConnectorReceiptInput = Readonly<{
  autoInviteAllowed?: boolean | undefined
  caveatRefs?: ReadonlyArray<string> | undefined
  channelRef: string
  commandRef: string
  commandScope: BusinessSharedChannelCommandScope
  commandVerified: boolean
  connectorRef: string
  createdAt?: string | undefined
  engagementRef: string
  externalSendAuthorized?: boolean | undefined
  inviteCreatedRef: string
  inviteRequestRef: string
  mentionRef: string
  outboundAllowed?: boolean | undefined
  platform: BusinessSharedChannelPlatform
  replyDraftRef: string
  requestedByActorRef: string
  sourceRefs: ReadonlyArray<string>
  workspaceRef: string
}>

export class BusinessSharedChannelConnectorInvariantError extends S.TaggedErrorClass<BusinessSharedChannelConnectorInvariantError>()(
  'BusinessSharedChannelConnectorInvariantError',
  { reason: S.String },
) {
  override get message() {
    return this.reason
  }
}

const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9._:/=#-]{2,220}$/
const UNSAFE_REF_PATTERN =
  /(@|access[_-]?token|\bauth\b|auth[_-]?(content|json|token)|bearer|channel[_-]?(name|url)|client[_-]?(email|name|phone)|contact[_-]?(email|name|phone)|cookie|customer[_-]?(email|name|phone)|email|invite[_-]?url|oauth|phone|private|raw[_-]?(body|channel|command|connector|invite|message|payload|reply|text|webhook)|secret|slack\.com\/archives|teams\.microsoft\.com|token|webhook)/i

const DEFAULT_CAVEAT_REFS = [
  'caveat.business.shared_channel.opt_in_only',
  'caveat.business.shared_channel.invite_on_request_never_auto',
  'caveat.business.shared_channel.reply_draft_only',
  'caveat.business.shared_channel.no_external_send_authority',
] as const

const trimOrNull = (value: string | null | undefined): string | null => {
  if (value === undefined || value === null) {
    return null
  }
  const trimmed = value.trim()
  return trimmed === '' ? null : trimmed
}

const requirePublicSafeRef = (field: string, value: string): string => {
  const trimmed = trimOrNull(value)
  if (trimmed === null) {
    throw new BusinessSharedChannelConnectorInvariantError({
      reason: `${field} is required.`,
    })
  }
  if (!SAFE_REF_PATTERN.test(trimmed) || UNSAFE_REF_PATTERN.test(trimmed)) {
    throw new BusinessSharedChannelConnectorInvariantError({
      reason: `${field} must be an opaque public-safe ref.`,
    })
  }
  return trimmed
}

const requirePublicSafeRefs = (
  field: string,
  values: ReadonlyArray<string>,
): ReadonlyArray<string> => {
  if (values.length === 0) {
    throw new BusinessSharedChannelConnectorInvariantError({
      reason: `${field} must contain at least one public-safe ref.`,
    })
  }
  return values.map(value => requirePublicSafeRef(field, value))
}

const requireIsoLike = (field: string, value: string): string => {
  const trimmed = trimOrNull(value)
  if (trimmed === null || Number.isNaN(Date.parse(trimmed))) {
    throw new BusinessSharedChannelConnectorInvariantError({
      reason: `${field} must be an ISO-like timestamp.`,
    })
  }
  return trimmed
}

const refSuffix = (value: string): string =>
  value.toLowerCase().replace(/[^a-z0-9]+/g, '_').replace(/^_+|_+$/g, '')

const receiptRefFor = (
  engagementRef: string,
  commandRef: string,
): string =>
  `receipt.business.shared_channel.${refSuffix(engagementRef)}.${refSuffix(commandRef)}`

export const buildBusinessSharedChannelConnectorReceipt = (
  input: BusinessSharedChannelConnectorReceiptInput,
): BusinessSharedChannelConnectorReceipt => {
  if (input.autoInviteAllowed === true) {
    throw new BusinessSharedChannelConnectorInvariantError({
      reason: 'shared-channel invites must be created only after an explicit request.',
    })
  }
  if (input.commandVerified !== true) {
    throw new BusinessSharedChannelConnectorInvariantError({
      reason: 'shared-channel commands must be verified before drafting a reply.',
    })
  }
  if (input.outboundAllowed === true || input.externalSendAuthorized === true) {
    throw new BusinessSharedChannelConnectorInvariantError({
      reason: 'shared-channel connector may draft replies but must not authorize external sends.',
    })
  }

  const engagementRef = requirePublicSafeRef(
    'engagementRef',
    input.engagementRef,
  )
  const commandRef = requirePublicSafeRef('commandRef', input.commandRef)

  return {
    schema: BUSINESS_SHARED_CHANNEL_CONNECTOR_RECEIPT_SCHEMA,
    receiptKind: 'business.shared_channel_connector.connected_run',
    receiptRef: receiptRefFor(engagementRef, commandRef),
    engagementRef,
    workspaceRef: requirePublicSafeRef('workspaceRef', input.workspaceRef),
    platform: input.platform,
    connectorRef: requirePublicSafeRef('connectorRef', input.connectorRef),
    channelRef: requirePublicSafeRef('channelRef', input.channelRef),
    inviteRequestRef: requirePublicSafeRef(
      'inviteRequestRef',
      input.inviteRequestRef,
    ),
    inviteCreatedRef: requirePublicSafeRef(
      'inviteCreatedRef',
      input.inviteCreatedRef,
    ),
    requestedByActorRef: requirePublicSafeRef(
      'requestedByActorRef',
      input.requestedByActorRef,
    ),
    mentionRef: requirePublicSafeRef('mentionRef', input.mentionRef),
    commandRef,
    commandScope: input.commandScope,
    commandVerified: true,
    replyDraftRef: requirePublicSafeRef('replyDraftRef', input.replyDraftRef),
    outboundAllowed: false,
    autoInviteAllowed: false,
    externalSendAuthorized: false,
    createdAt: requireIsoLike(
      'createdAt',
      input.createdAt ?? currentIsoTimestamp(),
    ),
    sourceRefs: [
      ...requirePublicSafeRefs('sourceRefs', input.sourceRefs),
      'docs/fable/ROADMAP_BIZ.md#BF-6.2',
    ],
    caveatRefs: [
      ...requirePublicSafeRefs(
        'caveatRefs',
        input.caveatRefs ?? DEFAULT_CAVEAT_REFS,
      ),
    ],
  }
}

export const assertBusinessSharedChannelConnectorReceipt = (
  receipt: BusinessSharedChannelConnectorReceipt,
): void => {
  const rebuilt = buildBusinessSharedChannelConnectorReceipt(receipt)
  if (rebuilt.receiptRef !== receipt.receiptRef) {
    throw new BusinessSharedChannelConnectorInvariantError({
      reason: 'receiptRef does not match the engagement and command refs.',
    })
  }
}

export const publicBusinessSharedChannelConnectorProjection = (
  receipt: BusinessSharedChannelConnectorReceipt,
) => ({
  schema: receipt.schema,
  receiptKind: receipt.receiptKind,
  receiptRef: receipt.receiptRef,
  engagementRef: receipt.engagementRef,
  workspaceRef: receipt.workspaceRef,
  platform: receipt.platform,
  commandScope: receipt.commandScope,
  commandVerified: receipt.commandVerified,
  outboundAllowed: receipt.outboundAllowed,
  autoInviteAllowed: receipt.autoInviteAllowed,
  externalSendAuthorized: receipt.externalSendAuthorized,
  createdAt: receipt.createdAt,
  sourceRefs: receipt.sourceRefs,
  caveatRefs: receipt.caveatRefs,
})
