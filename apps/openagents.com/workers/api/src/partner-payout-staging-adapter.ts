// Partner payout staging-test settlement adapter (#7021).
//
// This adapter satisfies the same PartnerPayoutAdapter contract as the
// owner-armed live adapter. It is for staging/tests only: it has no wallet
// client, no destination resolver, and no rail call, and it fails closed unless
// explicitly enabled.

import type { PartnerPayoutAdapter } from './partner-payout-dispatch'

export class PartnerPayoutStagingAdapterError extends Error {
  readonly _tag = 'PartnerPayoutStagingAdapterError'
  readonly reason: string

  constructor(reason: string) {
    super(reason)
    this.name = 'PartnerPayoutStagingAdapterError'
    this.reason = reason
  }
}

export type PartnerPayoutStagingAdapterConfig = Readonly<{
  enabled: boolean
}>

const textEncoder = new TextEncoder()

const sha256Hex = async (value: string): Promise<string> => {
  const digest = await crypto.subtle.digest(
    'SHA-256',
    textEncoder.encode(value),
  )

  return [...new Uint8Array(digest)]
    .map(byte => byte.toString(16).padStart(2, '0'))
    .join('')
}

export const partnerPayoutStagingReceiptRef = async (
  payoutRef: string,
  amountSats: number,
): Promise<string> =>
  `receipt.partner_payout.staging_test.${(
    await sha256Hex(`${payoutRef}:${amountSats}`)
  ).slice(0, 32)}`

export const makePartnerPayoutStagingAdapter = (
  config: PartnerPayoutStagingAdapterConfig,
): PartnerPayoutAdapter => ({
  adapterKind: 'staging_test',
  dispatch: async input => {
    if (!config.enabled) {
      throw new PartnerPayoutStagingAdapterError(
        'partner_payout_staging_adapter_not_enabled',
      )
    }

    return {
      receiptRef: await partnerPayoutStagingReceiptRef(
        input.payoutRef,
        input.amountSats,
      ),
    }
  },
})
