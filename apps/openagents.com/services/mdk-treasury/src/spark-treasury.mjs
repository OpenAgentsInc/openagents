import { createHash } from 'node:crypto'
import { mkdirSync } from 'node:fs'
import * as nodePath from 'node:path'

import { toSatNumber } from './sat-number.mjs'
import { SparkBunStorage } from './spark-bun-storage.ts'

// Same owner-authorized Breez/Spark service API key already used by Pylon for
// Spark receive/send. It is not wallet material; SPARK_TREASURY_API_KEY can
// override it. Spend authority comes from the treasury mnemonic: prefer an
// explicit SPARK_TREASURY_MNEMONIC override, otherwise use MDK_TREASURY_MNEMONIC.
const DEFAULT_OPENAGENTS_SPARK_API_KEY =
  'MIIBfjCCATCgAwIBAgIHPYzgGw0A+zAFBgMrZXAwEDEOMAwGA1UEAxMFQnJlZXowHhcNMjQxMTI0MjIxOTMzWhcNMzQxMTIyMjIxOTMzWjA3MRkwFwYDVQQKExBPcGVuQWdlbnRzLCBJbmMuMRowGAYDVQQDExFDaHJpc3RvcGhlciBEYXZpZDAqMAUGAytlcAMhANCD9cvfIDwcoiDKKYdT9BunHLS2/OuKzV8NS0SzqV13o4GBMH8wDgYDVR0PAQH/BAQDAgWgMAwGA1UdEwEB/wQCMAAwHQYDVR0OBBYEFNo5o+5ea0sNMlW/75VgGJCv2AcJMB8GA1UdIwQYMBaAFN6q1pJW843ndJIW/Ey2ILJrKJhrMB8GA1UdEQQYMBaBFGNocmlzQG9wZW5hZ2VudHMuY29tMAUGAytlcANBABvQIfNsop0kGIk0bgO/2kPum5B5lv6pYaSBXz73G1RV+eZj/wuW88lNQoGwVER+rA9+kWWTaR/dpdi8AFwjxw0='

const DEFAULT_SPARK_TIMEOUT_MS = 20_000

const secret = name => {
  const value = process.env[name]?.trim()

  return value === undefined || value === '' ? undefined : value
}

const sparkApiKey = () =>
  secret('SPARK_TREASURY_API_KEY') ??
  secret('OPENAGENTS_SPARK_API_KEY') ??
  secret('BREEZ_API_KEY') ??
  DEFAULT_OPENAGENTS_SPARK_API_KEY

const sparkMnemonic = () =>
  secret('SPARK_TREASURY_MNEMONIC') ?? secret('MDK_TREASURY_MNEMONIC')

export const sparkTreasuryConfiguredFlags = () => ({
  sparkApiKeyConfigured: sparkApiKey() !== undefined,
  sparkMnemonicConfigured: sparkMnemonic() !== undefined,
})

export const sparkTreasuryUnavailableReason = () =>
  sparkMnemonic() === undefined ? 'spark_treasury_unconfigured' : null

const timeoutMs = () => {
  const parsed = Number(process.env.SPARK_TREASURY_TIMEOUT_MS)

  return Number.isFinite(parsed) && parsed > 0
    ? Math.floor(parsed)
    : DEFAULT_SPARK_TIMEOUT_MS
}

const network = () =>
  process.env.SPARK_TREASURY_NETWORK === 'regtest' ? 'regtest' : 'mainnet'

const storageDir = () =>
  process.env.SPARK_TREASURY_STORAGE_DIR?.trim() || '/data/spark-treasury'

const withTimeout = async (promise, ms, label) => {
  let timer
  try {
    return await Promise.race([
      promise,
      new Promise((_, reject) => {
        timer = setTimeout(() => reject(new Error(`${label} timed out`)), ms)
      }),
    ])
  } finally {
    if (timer !== undefined) {
      clearTimeout(timer)
    }
  }
}

const publicRef = (prefix, value) =>
  `${prefix}.${createHash('sha256').update(value).digest('hex').slice(0, 32)}`

const publicStatus = value =>
  typeof value === 'string' && value.trim() !== '' ? value.trim() : null

const paymentValue = payload => payload?.payment ?? payload

const looksLikeLnurlPayDestination = destination => {
  const value = destination.trim()
  return /^lnurl/i.test(value) || /^[^@\s]+@[^@\s]+\.[^@\s]+$/.test(value)
}

let runningSparkSdk = null

const loadBreezSparkModule = async () => {
  const mod = await import('@breeztech/breez-sdk-spark')

  if (typeof mod?.defaultConfig !== 'function') {
    throw new Error('breez sdk spark module missing defaultConfig')
  }
  if (typeof mod?.SdkBuilder?.new !== 'function') {
    throw new Error('breez sdk spark module missing SdkBuilder.new')
  }

  return mod
}

const buildSparkSdk = async () => {
  if (runningSparkSdk !== null) {
    return runningSparkSdk
  }

  const mnemonic = sparkMnemonic()
  if (mnemonic === undefined) {
    throw new Error('spark treasury mnemonic missing')
  }

  const mod = await withTimeout(
    loadBreezSparkModule(),
    timeoutMs(),
    'spark sdk load',
  )
  const sdkConfig = mod.defaultConfig(network())
  sdkConfig.apiKey = sparkApiKey()
  mkdirSync(storageDir(), { recursive: true })
  const storage = new SparkBunStorage(
    nodePath.join(storageDir(), 'storage.sql'),
  )
  runningSparkSdk = await withTimeout(
    mod.SdkBuilder.new(sdkConfig, {
      type: 'mnemonic',
      mnemonic,
      passphrase: undefined,
    })
      .withStorage(storage)
      .build(),
    timeoutMs(),
    'spark sdk build',
  )

  return runningSparkSdk
}

const syncWallet = async sdk => {
  if (typeof sdk.syncWallet === 'function') {
    await withTimeout(
      sdk.syncWallet({}),
      timeoutMs(),
      'spark syncWallet',
    ).catch(() => undefined)
  }
}

export const sparkTreasuryBalancePayload = async () => {
  const sdk = await buildSparkSdk()
  await syncWallet(sdk)
  const info = await withTimeout(
    sdk.getInfo({ ensureSynced: true }),
    timeoutMs(),
    'spark getInfo',
  )
  const balanceSat = toSatNumber(info?.balanceSats ?? info?.balance_sats)

  return {
    balanceSat,
    maxSendableSat: balanceSat,
    rail: 'spark',
  }
}

const sparkAddress = async sdk => {
  const response = await withTimeout(
    sdk.receivePayment({ paymentMethod: { type: 'sparkAddress' } }),
    timeoutMs(),
    'spark receivePayment',
  )
  return typeof response?.paymentRequest === 'string'
    ? response.paymentRequest
    : null
}

const lightningAddress = async sdk => {
  if (
    typeof sdk.getLightningAddress !== 'function' ||
    typeof sdk.registerLightningAddress !== 'function'
  ) {
    return null
  }

  const existing = await withTimeout(
    sdk.getLightningAddress(),
    timeoutMs(),
    'spark getLightningAddress',
  ).catch(() => undefined)

  if (
    typeof existing?.lightningAddress === 'string' &&
    existing.lightningAddress !== ''
  ) {
    return existing.lightningAddress
  }

  const address = await sparkAddress(sdk)
  if (address === null) {
    return null
  }
  const username = `oa${createHash('sha256').update(address).digest('hex').slice(0, 16)}`
  const registered = await withTimeout(
    sdk.registerLightningAddress({
      description: 'OpenAgents treasury Spark rail',
      username,
    }),
    timeoutMs(),
    'spark registerLightningAddress',
  )

  return typeof registered?.lightningAddress === 'string'
    ? registered.lightningAddress
    : null
}

export const sparkTreasuryFundingPayload = async () => {
  const sdk = await buildSparkSdk()
  await syncWallet(sdk)

  return {
    lightningAddress: await lightningAddress(sdk),
    rail: 'spark',
    sparkAddress: await sparkAddress(sdk),
  }
}

export const sparkTreasuryPayPayload = async input => {
  const sdk = await buildSparkSdk()
  await syncWallet(sdk)

  const destination = String(input?.destination ?? '').trim()
  const amountSat = Number(input?.amountSat)
  const idempotencyKey = String(input?.idempotencyKey ?? '').trim()

  if (destination === '') {
    return { error: 'destination_required', status: 400 }
  }
  if (!Number.isInteger(amountSat) || amountSat <= 0) {
    return { error: 'amount_sat_must_be_positive_integer', status: 400 }
  }
  if (idempotencyKey === '') {
    return { error: 'idempotency_key_required', status: 400 }
  }

  const before = await sparkTreasuryBalancePayload()
  if (
    typeof before.maxSendableSat !== 'number' ||
    before.maxSendableSat < amountSat
  ) {
    return {
      balanceSatBefore: before.balanceSat,
      error: 'spark_treasury_insufficient_spendable_balance',
      reasonRef: 'reason.public.treasury_payout.insufficient_spendable_balance',
      status: 409,
    }
  }

  const send = async () => {
    if (
      looksLikeLnurlPayDestination(destination) &&
      typeof sdk.parse === 'function' &&
      typeof sdk.prepareLnurlPay === 'function' &&
      typeof sdk.lnurlPay === 'function'
    ) {
      const parsed = await withTimeout(
        sdk.parse(destination),
        timeoutMs(),
        'spark parse',
      )
      const payRequest =
        parsed?.type === 'lightningAddress'
          ? parsed.payRequest
          : parsed?.type === 'lnurlPay'
            ? parsed
            : null
      if (payRequest === null || payRequest === undefined) {
        throw new Error(`unsupported parsed input:${parsed?.type ?? 'unknown'}`)
      }
      const prepareResponse = await withTimeout(
        sdk.prepareLnurlPay({
          amount: BigInt(amountSat),
          payRequest,
        }),
        timeoutMs(),
        'spark prepareLnurlPay',
      )
      const sent = await withTimeout(
        sdk.lnurlPay({ idempotencyKey, prepareResponse }),
        timeoutMs(),
        'spark lnurlPay',
      )
      return { method: 'lnurl_pay', payment: paymentValue(sent) }
    }

    if (
      typeof sdk.prepareSendPayment !== 'function' ||
      typeof sdk.sendPayment !== 'function'
    ) {
      throw new Error('spark send unsupported')
    }

    const prepareResponse = await withTimeout(
      sdk.prepareSendPayment({
        amount: BigInt(amountSat),
        paymentRequest: destination,
      }),
      timeoutMs(),
      'spark prepareSendPayment',
    )
    const paymentMethod = prepareResponse?.paymentMethod
    const sent = await withTimeout(
      sdk.sendPayment({
        idempotencyKey,
        options:
          paymentMethod?.type === 'bolt11Invoice'
            ? {
                completionTimeoutSecs: 60,
                preferSpark: true,
                type: 'bolt11Invoice',
              }
            : undefined,
        prepareResponse,
      }),
      timeoutMs(),
      'spark sendPayment',
    )

    return { method: 'payment_request', payment: paymentValue(sent) }
  }

  let sent
  try {
    sent = await send()
  } catch (error) {
    return {
      balanceSatBefore: before.balanceSat,
      error: 'spark_treasury_pay_failed',
      failureStage: 'spark_pay',
      reasonRef:
        error instanceof Error &&
        error.message.toLowerCase().includes('insufficient')
          ? 'reason.public.treasury_payout.insufficient_spendable_balance'
          : 'reason.public.treasury_payout.failed',
      resultReturned: false,
      status: 502,
    }
  }
  const after = await sparkTreasuryBalancePayload()
  const amount = toSatNumber(sent.payment?.amount) ?? amountSat
  const fee = toSatNumber(sent.payment?.fees)
  const paymentRef = publicRef(
    'payment.redacted.spark_treasury',
    [
      typeof sent.payment?.id === 'string' ? sent.payment.id : 'id-redacted',
      publicStatus(sent.payment?.status) ?? 'status-redacted',
      sent.method,
      String(amount),
      fee === null ? 'fee-unknown' : String(fee),
      idempotencyKey,
    ].join(':'),
  )

  return {
    amountSat: amount,
    balanceChanged:
      typeof before.balanceSat === 'number' &&
      typeof after.balanceSat === 'number'
        ? before.balanceSat !== after.balanceSat
        : null,
    balanceDeltaSat:
      typeof before.balanceSat === 'number' &&
      typeof after.balanceSat === 'number'
        ? after.balanceSat - before.balanceSat
        : null,
    balanceSatAfter: after.balanceSat,
    balanceSatBefore: before.balanceSat,
    destinationKind: looksLikeLnurlPayDestination(destination)
      ? 'lightning_address'
      : 'spark_address',
    feeSats: fee,
    method: sent.method,
    moneyMovement: 'real_bitcoin',
    paymentIdPresent: typeof sent.payment?.id === 'string',
    paymentRef,
    preimagePresent: false,
    rail: 'spark',
    resultReturned: true,
    sparkPaymentRef: paymentRef,
    status: 'succeeded',
  }
}
