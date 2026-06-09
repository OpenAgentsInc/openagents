export type OpenAgentsUnsafeRedactionFixture = Readonly<{
  label: string
  value: string
}>

export const OPENAGENTS_PAYMENT_UNSAFE_REDACTION_FIXTURES:
  ReadonlyArray<OpenAgentsUnsafeRedactionFixture> = [
    {
      label: 'MDK access token',
      value: 'mdk_access_token=secret.openagents.fixture',
    },
    {
      label: 'MDK mnemonic',
      value: 'mdk_mnemonic=secret.seed_phrase.openagents.fixture',
    },
    {
      label: 'MDK webhook secret',
      value: 'mdk_webhook_secret=secret.webhook.openagents.fixture',
    },
    {
      label: 'agent wallet home',
      value: 'wallet_state=/Users/openagents/.mdk-wallet/config.json',
    },
    {
      label: 'raw BOLT11 invoice',
      value:
        'lnbc10n1popenagentsfixtureabcdefghijklmnopqrstuvwxyz0123456789',
    },
    {
      label: 'raw BOLT12 offer',
      value:
        'lno1popenagentsfixtureabcdefghijklmnopqrstuvwxyz0123456789',
    },
    {
      label: 'raw payment hash',
      value:
        'raw_payment_hash=abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890',
    },
    {
      label: 'raw payment preimage',
      value:
        'payment_preimage=abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890',
    },
    {
      label: 'provider grant',
      value: 'provider_grant.secret.raw_checkout_provider',
    },
    {
      label: 'Stripe secret',
      value: 'sk-test-secret-openagents-fixture',
    },
    {
      label: 'Treasury secret',
      value: 'treasury_secret.raw_fixture',
    },
    {
      label: 'raw payout target',
      value:
        'raw_payout_target=lnbc10n1popenagentsfixtureabcdefghijklmnopqrstuvwxyz0123456789',
    },
    {
      label: 'raw wallet balance',
      value: 'raw_payment_balance_sats=50000',
    },
    {
      label: 'private checkout ref',
      value: 'checkout_id=secret_checkout_private_fixture_123456',
    },
    {
      label: 'private customer or operator data',
      value: 'raw_customer_email_secret=buyer@example.invalid',
    },
  ]

export const OPENAGENTS_UNSAFE_REDACTION_FIXTURES:
  ReadonlyArray<OpenAgentsUnsafeRedactionFixture> = [
    { label: 'secret token', value: 'sk-openagents-secret-test-value' },
    { label: 'provider grant', value: 'provider_grant.chatgpt_account' },
    { label: 'callback token', value: 'callback_token.oauth_return' },
    { label: 'private prompt', value: 'raw_prompt.customer_strategy' },
    { label: 'payment proof', value: 'payment_proof.raw_preimage' },
    { label: 'wallet material', value: 'wallet.mnemonic.local' },
    { label: 'private repo', value: 'github.com/acme/private' },
    { label: 'raw payload', value: 'raw_payload.provider_response' },
    { label: 'raw runner log', value: 'raw_runner_log.full' },
    { label: 'raw timestamp', value: '2026-06-06T22:45:00.000Z' },
  ]

export const openAgentsUnsafeRedactionFixtureValues =
  OPENAGENTS_UNSAFE_REDACTION_FIXTURES.map(fixture => fixture.value)

export const openAgentsUnsafePaymentRedactionFixtureValues =
  OPENAGENTS_PAYMENT_UNSAFE_REDACTION_FIXTURES.map(fixture => fixture.value)

export const openAgentsSerializedValueContainsUnsafeFixture = (
  value: unknown,
): boolean => {
  const serialized = JSON.stringify(value)

  return openAgentsUnsafeRedactionFixtureValues.some(fixtureValue =>
    serialized.includes(fixtureValue)
  )
}
