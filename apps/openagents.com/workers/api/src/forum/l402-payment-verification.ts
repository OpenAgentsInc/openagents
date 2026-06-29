import { Effect } from 'effect'

import type { BuyerPaymentLedgerAmount } from '../buyer-payment-ledger'
import {
  type OpenAgentsL402SigningBoundary,
  verifyOpenAgentsL402Credential,
} from '../l402-credential-service'
import { parseOpenAgentsPaymentHeaders } from '../l402-payment-headers'
import { forumPaidActionProductId } from './paid-actions'
import {
  ForumPaidActionError,
  type ForumVerifiedPaymentEventInput,
} from './paid-actions'
import { type ForumL402Challenge, type ForumMoneyAmount } from './schemas'

export type ForumL402SigningBoundaryProvider =
  () => Promise<OpenAgentsL402SigningBoundary | null>

export type ForumL402PaymentVerificationInput = Readonly<{
  challenge: ForumL402Challenge
  headers: Headers
  l402ProofRef: string
  nowIso: string
  signingBoundary?: ForumL402SigningBoundaryProvider | undefined
}>

const refSegmentPattern = /[^A-Za-z0-9_-]+/g

const cleanRefSegment = (value: string): string =>
  value.replace(refSegmentPattern, '_').slice(0, 120)

const forumL402ChallengeRef = (challengeId: string): string =>
  `challenge.forum_l402.${cleanRefSegment(challengeId)}`

const forumL402ProductId = (actionKind: string): string =>
  forumPaidActionProductId(actionKind)

const forumAmountToBuyerPaymentAmount = (
  amount: ForumMoneyAmount,
): BuyerPaymentLedgerAmount =>
  amount.asset === 'sats'
    ? {
        amountMinorUnits: amount.amount * 1000,
        asset: 'bitcoin',
        denomination: 'bitcoin_millisatoshi',
      }
    : amount.asset === 'usd'
      ? {
          amountMinorUnits: amount.amount,
          asset: 'usd',
          denomination: 'usd_cent',
        }
      : {
          amountMinorUnits: amount.amount,
          asset: 'credits',
          denomination: 'credit',
        }

const verificationFailed = (reason: string): ForumPaidActionError =>
  new ForumPaidActionError({
    kind: 'payment_verification_failed',
    reason,
  })

const parsePaymentHeaders = (headers: Headers) =>
  Effect.try({
    catch: error =>
      verificationFailed(
        error instanceof Error
          ? error.message
          : 'Forum L402 payment headers could not be parsed.',
      ),
    try: () => parseOpenAgentsPaymentHeaders(headers),
  })

const resolveSigningBoundary = (
  provider: ForumL402SigningBoundaryProvider | undefined,
) =>
  provider === undefined
    ? Effect.fail(
        verificationFailed('Forum L402 payment verifier is not configured.'),
      )
    : Effect.tryPromise({
        catch: error =>
          verificationFailed(
            error instanceof Error
              ? error.message
              : 'Forum L402 payment verifier could not be loaded.',
          ),
        try: provider,
      }).pipe(
        Effect.flatMap(signer =>
          signer === null
            ? Effect.fail(
                verificationFailed(
                  'Forum L402 payment verifier is not configured.',
                ),
              )
            : Effect.succeed(signer),
        ),
      )

export const verifyForumL402PaymentEvent = (
  input: ForumL402PaymentVerificationInput,
): Effect.Effect<ForumVerifiedPaymentEventInput, ForumPaidActionError> =>
  Effect.gen(function* () {
    const l402 = input.challenge.l402

    if (l402 === null) {
      return yield* verificationFailed(
        'Forum paid-action challenge does not have an L402 payment binding.',
      )
    }

    const parsed = yield* parsePaymentHeaders(input.headers)

    if (parsed.credential === null || parsed.proofRef === null) {
      return yield* verificationFailed(parsed.reasonRef)
    }

    if (parsed.proofRef !== input.l402ProofRef) {
      return yield* verificationFailed(
        'Forum L402 proof ref must match the payment credential header.',
      )
    }

    const signer = yield* resolveSigningBoundary(input.signingBoundary)
    const verification = yield* Effect.tryPromise({
      catch: error =>
        verificationFailed(
          error instanceof Error
            ? error.message
            : 'Forum L402 credential verification failed.',
        ),
      try: () =>
        verifyOpenAgentsL402Credential(parsed.credential ?? '', signer, {
          amount: forumAmountToBuyerPaymentAmount(input.challenge.price),
          challengeRef: forumL402ChallengeRef(input.challenge.challengeId),
          endpointRef: l402.endpointRef,
          entitlementScopeRefs: l402.entitlementScopeRefs,
          method: input.challenge.method,
          nowIso: input.nowIso,
          path: input.challenge.path,
          paymentProofRef: parsed.proofRef,
          productId: forumL402ProductId(input.challenge.actionKind),
          requestBodyDigest: input.challenge.requestBodyDigest,
          requirePaymentProof: true,
        }),
    })

    if (verification.status !== 'valid' || verification.payload === null) {
      return yield* verificationFailed(verification.reasonRef)
    }

    if (
      verification.payload.credentialRef !== l402.credentialRef ||
      verification.payload.replayNonceRef !== l402.replayNonceRef
    ) {
      return yield* verificationFailed(
        'Forum L402 credential does not match the stored challenge credential refs.',
      )
    }

    return {
      externalRef: `external.forum_l402.${cleanRefSegment(verification.payload.credentialRef)}`,
      paymentMode: l402.sandbox ? 'sandbox' : 'live',
      providerRef: l402.providerRef,
      redactedEvidenceRef: `evidence.forum_l402.${cleanRefSegment(parsed.proofRef)}`,
      status: 'confirmed',
    }
  })
