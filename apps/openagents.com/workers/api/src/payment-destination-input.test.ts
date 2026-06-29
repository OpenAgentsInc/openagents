import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import type { OpenAgentsPaymentDestinationInput } from './payment-destination-input'
import {
  OpenAgentsPaymentDestinationProjection,
  OpenAgentsPaymentDestinationUnsafe,
  classifyOpenAgentsPaymentDestinationInput,
  openAgentsPaymentDestinationHasPrivateMaterial,
} from './payment-destination-input'

const sampleBolt11 =
  'lnbc20m1pn7qa2ndqqnp4q0d3p2sfluzdx45tqcsh2pu5qc7lgq0xs578ngs6s0s68ua4h7cvspp5kwzshmne5zw3lnfqdk8cv26mg9ndjapqzhcxn2wtn9d6ew5e2jfqsp5h3u5f0l522vs488h6n8zm5ca2lkpva532fnl2kp4wnvsuq445erq9qyysgqcqpcxqppz4395v2sjh3t5pzckgeelk9qf0z3fm9jzxtjqpqygayt4xyy7tpjvq5pe7f6727du2mg3t2tfe0cd53de2027ff7es7smtew8xx5x2spwuvkdz'
const sampleBolt12 =
  'lno1qgs0v8hw8d368q9yw7sx8tejk2aujlyll8cp7tzzyh5h8xyppqqqqqqgqvqcdgq2qenxzatrv46pvggrv64u366d5c0rr2xjc3fq6vw2hh6ce3f9p7z4v4ee0u7avfynjw9q'
const sampleLnurl =
  'LNURL1DP68GURN8GHJ7MRWW4EXCTNDW46XJMNEDEJHGTNRDAKJ7TNHV4KXCTTTDEHHWM30D3H82UNVWQHHYETXW4HXG0AH8NK'
const sampleBitcoinUri =
  'bitcoin:1andreas3batLhQa2FawWjeyjCqyBzypd?amount=50&label=Luke-Jr&message=Donation%20for%20project%20xyz'
const sampleBitcoinUriWithInvoice =
  `bitcoin:BC1QYLH3U67J673H6Y6ALV70M0PL2YZ53TZHVXGG7U?amount=0.00001&label=sbddesign%3A%20For%20lunch%20Tuesday&message=For%20lunch%20Tuesday&lightning=${sampleBolt11}`

const baseInput = (
  rawInput: string,
  overrides: Partial<OpenAgentsPaymentDestinationInput> = {},
): OpenAgentsPaymentDestinationInput => ({
  allowCashu: false,
  allowNetworkResolution: false,
  allowOnchain: true,
  inputRef: 'input.payment_destination.test',
  rawInput,
  source: 'raw_text',
  ...overrides,
})

describe('OpenAgents payment destination input parser', () => {
  test('classifies BOLT11 and BOLT12 without projecting raw payment strings', () => {
    const bolt11 = classifyOpenAgentsPaymentDestinationInput(
      baseInput(sampleBolt11, { inputRef: 'input.payment_destination.bolt11' }),
    )
    const bolt12 = classifyOpenAgentsPaymentDestinationInput(
      baseInput(`lightning:${sampleBolt12}`, {
        inputRef: 'input.payment_destination.bolt12',
      }),
    )

    expect(S.decodeUnknownSync(OpenAgentsPaymentDestinationProjection)(bolt11))
      .toEqual(bolt11)
    expect(bolt11.kind).toBe('bolt11')
    expect(bolt11.classificationStatus).toBe('supported_parse_only')
    expect(bolt11.dispatchAllowed).toBe(false)
    expect(bolt11.payoutAuthorityCreated).toBe(false)
    expect(openAgentsPaymentDestinationHasPrivateMaterial(bolt11)).toBe(false)
    expect(JSON.stringify(bolt11)).not.toContain(sampleBolt11)

    expect(bolt12.kind).toBe('bolt12')
    expect(bolt12.source).toBe('lightning_uri')
    expect(bolt12.methodRefs).toEqual(['method.lightning.bolt12_offer'])
    expect(openAgentsPaymentDestinationHasPrivateMaterial(bolt12)).toBe(false)
    expect(JSON.stringify(bolt12)).not.toContain(sampleBolt12)
  })

  test('classifies LNURL and Lightning Address as requiring resolution outside the Worker parser', () => {
    const lnurl = classifyOpenAgentsPaymentDestinationInput(
      baseInput(sampleLnurl, { inputRef: 'input.payment_destination.lnurl' }),
    )
    const address = classifyOpenAgentsPaymentDestinationInput(
      baseInput('agent@payments.example', {
        inputRef: 'input.payment_destination.lightning_address',
      }),
    )

    expect(lnurl.kind).toBe('lnurl')
    expect(lnurl.requiresResolution).toBe(true)
    expect(lnurl.classificationStatus).toBe('requires_resolution')
    expect(lnurl.runtimeDecision)
      .toBe('rust_wasm_or_sidecar_required_for_resolution')
    expect(JSON.stringify(lnurl)).not.toContain(sampleLnurl)

    expect(address.kind).toBe('lightning_address')
    expect(address.requiresResolution).toBe(true)
    expect(address.redactedDestinationRef)
      .toBe('payment_destination.lightning_address.raw_text.unknown.input.payment_destination.lightning_address')
    expect(openAgentsPaymentDestinationHasPrivateMaterial(address)).toBe(false)
    expect(JSON.stringify(address)).not.toContain('agent@payments.example')
  })

  test('classifies BIP321 bitcoin URIs and structured fallback methods', () => {
    const onchain = classifyOpenAgentsPaymentDestinationInput(
      baseInput(sampleBitcoinUri, {
        inputRef: 'input.payment_destination.bitcoin_uri',
      }),
    )
    const fallback = classifyOpenAgentsPaymentDestinationInput(
      baseInput(sampleBitcoinUriWithInvoice, {
        inputRef: 'input.payment_destination.bitcoin_uri_with_invoice',
      }),
    )

    expect(onchain.kind).toBe('bitcoin_uri')
    expect(onchain.methodRefs).toEqual(['method.bitcoin.onchain'])
    expect(onchain.networkHint).toBe('bitcoin')
    expect(JSON.stringify(onchain)).not.toContain(sampleBitcoinUri)

    expect(fallback.kind).toBe('bitcoin_uri')
    expect(fallback.methodRefs).toEqual([
      'method.bitcoin.onchain',
      'method.lightning.bolt11',
    ])
    expect(fallback.classificationStatus).toBe('supported_parse_only')
    expect(JSON.stringify(fallback)).not.toContain(sampleBolt11)
  })

  test('marks unsupported, malformed, and ambiguous inputs without creating authority', () => {
    const unsupported = classifyOpenAgentsPaymentDestinationInput(
      baseInput('https://example.com/pay', {
        inputRef: 'input.payment_destination.unsupported',
      }),
    )
    const malformed = classifyOpenAgentsPaymentDestinationInput(
      baseInput('lightning:lnbc1bad', {
        inputRef: 'input.payment_destination.malformed',
      }),
    )
    const ambiguous = classifyOpenAgentsPaymentDestinationInput(
      baseInput(`${sampleBolt11} ${sampleLnurl}`, {
        inputRef: 'input.payment_destination.ambiguous',
      }),
    )

    expect(unsupported.kind).toBe('unsupported')
    expect(unsupported.classificationStatus).toBe('unsupported')
    expect(malformed.kind).toBe('malformed')
    expect(malformed.classificationStatus).toBe('malformed')
    expect(ambiguous.kind).toBe('ambiguous')
    expect(ambiguous.classificationStatus).toBe('ambiguous')

    expect([
      unsupported,
      malformed,
      ambiguous,
    ].every(projection =>
      projection.approvalRequired &&
      !projection.dispatchAllowed &&
      !projection.payoutAuthorityCreated &&
      !projection.rawDestinationProjected,
    )).toBe(true)
  })

  test('rejects private wallet, provider, and preimage material', () => {
    expect(() =>
      classifyOpenAgentsPaymentDestinationInput(
        baseInput('payment_preimage=abc123', {
          inputRef: 'input.payment_destination.preimage',
        }),
      ),
    ).toThrow(OpenAgentsPaymentDestinationUnsafe)
    expect(() =>
      classifyOpenAgentsPaymentDestinationInput(
        baseInput('/Users/chris/.mdk-wallet/config.json', {
          inputRef: 'input.payment_destination.wallet_path',
        }),
      ),
    ).toThrow(OpenAgentsPaymentDestinationUnsafe)
    expect(() =>
      classifyOpenAgentsPaymentDestinationInput(
        baseInput('MDK_ACCESS_TOKEN=secret', {
          inputRef: 'input.payment_destination.provider_secret',
        }),
      ),
    ).toThrow(OpenAgentsPaymentDestinationUnsafe)
  })
})
