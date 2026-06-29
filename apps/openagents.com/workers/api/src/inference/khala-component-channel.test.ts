import { describe, expect, test } from 'vitest'

import {
  KHALA_COMPONENT_CATALOG_PROMPT,
  KHALA_COMPONENT_NAMES,
  OA_COMPONENT_SSE_EVENT,
  OA_COMPONENT_WIRE_VERSION,
  buildComponentRepairInstruction,
  isComponentChannelEnabled,
  isKnownKhalaComponent,
  runComponentChannel,
  serializeComponentFrame,
  splitMixedStream,
  stripComponentFences,
  validateComponentCandidate,
  validateWithBoundedRepair,
  type KhalaComponentFrame,
} from './khala-component-channel'

// Helper: wrap a raw JSON string as a fenced oa-component block.
const fenced = (json: string): string =>
  ['```oa-component', json, '```'].join('\n')

describe('component-channel feature flag', () => {
  test('defaults off and only enables on explicit truthy tokens', () => {
    expect(isComponentChannelEnabled(undefined)).toBe(false)
    expect(isComponentChannelEnabled('')).toBe(false)
    expect(isComponentChannelEnabled('false')).toBe(false)
    expect(isComponentChannelEnabled('0')).toBe(false)
    expect(isComponentChannelEnabled('true')).toBe(true)
    expect(isComponentChannelEnabled('TRUE')).toBe(true)
    expect(isComponentChannelEnabled('1')).toBe(true)
    expect(isComponentChannelEnabled('on')).toBe(true)
  })
})

describe('closed catalog (v1)', () => {
  test('exposes exactly the 6 v1 components', () => {
    expect([...KHALA_COMPONENT_NAMES].sort()).toEqual(
      [
        'consent_gate',
        'credit_kickoff',
        'dashboard_preview',
        'human_handoff',
        'intake_progress',
        'quick_win_card',
      ].sort(),
    )
  })

  test('isKnownKhalaComponent recognizes catalog names and rejects others', () => {
    expect(isKnownKhalaComponent('credit_kickoff')).toBe(true)
    expect(isKnownKhalaComponent('consent_gate')).toBe(true)
    // An unknown / model-invented component is NOT in the closed catalog.
    expect(isKnownKhalaComponent('arbitrary_widget')).toBe(false)
    expect(isKnownKhalaComponent('script')).toBe(false)
    expect(isKnownKhalaComponent('')).toBe(false)
  })

  test('the catalog prompt lists every component name', () => {
    for (const name of KHALA_COMPONENT_NAMES) {
      expect(KHALA_COMPONENT_CATALOG_PROMPT).toContain(name)
    }
    // And forbids putting provider identity inside a card.
    expect(KHALA_COMPONENT_CATALOG_PROMPT.toLowerCase()).toContain('provider')
  })
})

describe('splitMixedStream / stripComponentFences', () => {
  test('a completion with no fence is all prose', () => {
    const segments = splitMixedStream('just some prose, no card here')
    expect(segments).toEqual([
      { kind: 'prose', text: 'just some prose, no card here' },
    ])
  })

  test('splits prose around a fenced component block, in order', () => {
    const completion = [
      'Here is your kickoff:',
      fenced('{"component":"credit_kickoff","props":{"amountCents":50000,"label":"Go"}}'),
      'Click to continue.',
    ].join('\n\n')
    const segments = splitMixedStream(completion)
    expect(segments.map(s => s.kind)).toEqual(['prose', 'component', 'prose'])
    const component = segments.find(s => s.kind === 'component')
    expect(component?.kind).toBe('component')
  })

  test('extracts multiple component blocks in document order', () => {
    const completion = [
      fenced('{"component":"intake_progress","props":{"steps":["a","b"],"current":0}}'),
      'mid prose',
      fenced('{"component":"human_handoff","props":{"reason":"x","contact":"y"}}'),
    ].join('\n\n')
    const segments = splitMixedStream(completion)
    expect(segments.filter(s => s.kind === 'component')).toHaveLength(2)
  })

  test('stripComponentFences returns prose only', () => {
    const completion = [
      'Prose line.',
      fenced('{"component":"credit_kickoff","props":{"amountCents":1,"label":"x"}}'),
    ].join('\n\n')
    expect(stripComponentFences(completion)).toBe('Prose line.')
  })
})

describe('validateComponentCandidate — closed-enum + schema', () => {
  test('accepts a valid credit_kickoff card', () => {
    const result = validateComponentCandidate(
      '{"component":"credit_kickoff","props":{"amountCents":50000,"label":"Kick off with $500 in credits"}}',
      { id: 'cmp_01' },
    )
    expect(result.ok).toBe(true)
    if (result.ok) {
      expect(result.frame.component).toBe('credit_kickoff')
      expect(result.frame.v).toBe(OA_COMPONENT_WIRE_VERSION)
      expect(result.frame.id).toBe('cmp_01')
      expect(result.frame.props['amountCents']).toBe(50000)
    }
  })

  test('REJECTS an unknown component name (closed-enum enforcement)', () => {
    const result = validateComponentCandidate(
      '{"component":"steal_credentials","props":{"x":1}}',
      { id: 'cmp_01' },
    )
    expect(result.ok).toBe(false)
    if (!result.ok) {
      expect(result.reason).toBe('unknown_component')
      // The rejection names the closed catalog, never executes the unknown card.
      expect(result.detail).toContain('credit_kickoff')
    }
  })

  test('rejects invalid props (wrong type) against the schema', () => {
    const result = validateComponentCandidate(
      '{"component":"credit_kickoff","props":{"amountCents":"lots","label":"x"}}',
      { id: 'cmp_01' },
    )
    expect(result.ok).toBe(false)
    if (!result.ok) expect(result.reason).toBe('invalid_props')
  })

  test('rejects a non-positive amountCents', () => {
    const result = validateComponentCandidate(
      '{"component":"credit_kickoff","props":{"amountCents":0,"label":"x"}}',
      { id: 'cmp_01' },
    )
    expect(result.ok).toBe(false)
    if (!result.ok) expect(result.reason).toBe('invalid_props')
  })

  test('rejects missing props field', () => {
    const result = validateComponentCandidate(
      '{"component":"credit_kickoff"}',
      { id: 'cmp_01' },
    )
    expect(result.ok).toBe(false)
    if (!result.ok) expect(result.reason).toBe('invalid_props')
  })

  test('rejects a missing component name', () => {
    const result = validateComponentCandidate('{"props":{}}', { id: 'cmp_01' })
    expect(result.ok).toBe(false)
    if (!result.ok) expect(result.reason).toBe('missing_component')
  })

  test('rejects non-JSON', () => {
    const result = validateComponentCandidate('not json at all', {
      id: 'cmp_01',
    })
    expect(result.ok).toBe(false)
    if (!result.ok) expect(result.reason).toBe('invalid_json')
  })

  test('rejects a JSON array (must be an object)', () => {
    const result = validateComponentCandidate('[1,2,3]', { id: 'cmp_01' })
    expect(result.ok).toBe(false)
    if (!result.ok) expect(result.reason).toBe('invalid_json')
  })

  test('intake_progress.current must index into steps (cross-field bound)', () => {
    const ok = validateComponentCandidate(
      '{"component":"intake_progress","props":{"steps":["a","b","c"],"current":2}}',
      { id: 'cmp_01' },
    )
    expect(ok.ok).toBe(true)
    const outOfRange = validateComponentCandidate(
      '{"component":"intake_progress","props":{"steps":["a","b"],"current":5}}',
      { id: 'cmp_01' },
    )
    expect(outOfRange.ok).toBe(false)
    if (!outOfRange.ok) expect(outOfRange.reason).toBe('invalid_props')
  })

  test('validates every catalog component with a good payload', () => {
    const goods: Record<string, string> = {
      consent_gate:
        '{"component":"consent_gate","props":{"scope":"intake","dataPractices":"redacted before inference","required":true}}',
      credit_kickoff:
        '{"component":"credit_kickoff","props":{"amountCents":50000,"label":"Go"}}',
      dashboard_preview:
        '{"component":"dashboard_preview","props":{"workspaceRef":"ws_1","seededFacts":["fact a"]}}',
      human_handoff:
        '{"component":"human_handoff","props":{"reason":"needs a person","contact":"team@x"}}',
      intake_progress:
        '{"component":"intake_progress","props":{"steps":["a","b"],"current":1}}',
      quick_win_card:
        '{"component":"quick_win_card","props":{"title":"NDA","scope":"one doc","etaDays":3}}',
    }
    for (const name of KHALA_COMPONENT_NAMES) {
      const result = validateComponentCandidate(goods[name]!, { id: 'cmp_x' })
      expect(result.ok, `component ${name} should validate`).toBe(true)
    }
  })
})

describe('provider-identity non-leakage on the structured channel', () => {
  test('drops a card whose props leak a provider identity', () => {
    // A card label that asserts a first-person provider identity ("we are built
    // on Gemini") trips the SAME identity backstop the prose channel runs.
    const result = validateComponentCandidate(
      '{"component":"quick_win_card","props":{"title":"We are built on Gemini","scope":"x","etaDays":1}}',
      { id: 'cmp_01' },
    )
    expect(result.ok).toBe(false)
    if (!result.ok) expect(result.reason).toBe('provider_identity_leak')
  })

  test('a card that merely mentions a third party factually is NOT dropped', () => {
    // The identity backstop only flags a FIRST-PERSON provenance claim, so a
    // neutral third-party mention passes (mirrors the prose-channel behavior).
    const result = validateComponentCandidate(
      '{"component":"quick_win_card","props":{"title":"Gemini is a Google model","scope":"x","etaDays":1}}',
      { id: 'cmp_01' },
    )
    expect(result.ok).toBe(true)
  })
})

describe('bounded repair turn', () => {
  test('a valid candidate needs no repair', async () => {
    const result = await validateWithBoundedRepair(
      '{"component":"credit_kickoff","props":{"amountCents":50000,"label":"Go"}}',
      { id: 'cmp_01' },
    )
    expect(result.outcome).toBe('valid')
    if (result.outcome === 'valid') expect(result.repaired).toBe(false)
  })

  test('ONE repair turn fixes an invalid candidate', async () => {
    let reaskCalls = 0
    const result = await validateWithBoundedRepair(
      '{"component":"credit_kickoff","props":{"amountCents":"bad"}}',
      {
        id: 'cmp_01',
        reask: async () => {
          reaskCalls += 1
          return '{"component":"credit_kickoff","props":{"amountCents":50000,"label":"Fixed"}}'
        },
      },
    )
    expect(reaskCalls).toBe(1)
    expect(result.outcome).toBe('valid')
    if (result.outcome === 'valid') expect(result.repaired).toBe(true)
  })

  test('the repair turn is BOUNDED to one — still-invalid output is DROPPED', async () => {
    let reaskCalls = 0
    const result = await validateWithBoundedRepair(
      '{"component":"credit_kickoff","props":{"amountCents":"bad"}}',
      {
        id: 'cmp_01',
        // Repair returns ANOTHER invalid candidate; the card must be dropped,
        // never shipped, and the reask is attempted exactly once.
        reask: async () => {
          reaskCalls += 1
          return '{"component":"credit_kickoff","props":{"amountCents":"still bad"}}'
        },
      },
    )
    expect(reaskCalls).toBe(1)
    expect(result.outcome).toBe('dropped')
  })

  test('no reask wired => an invalid candidate is dropped without a repair', async () => {
    const result = await validateWithBoundedRepair(
      '{"component":"credit_kickoff","props":{"amountCents":"bad"}}',
      { id: 'cmp_01' },
    )
    expect(result.outcome).toBe('dropped')
  })

  test('the repair instruction names the closed catalog + the rejection', () => {
    const rejection = {
      detail: 'amountCents must be a number',
      ok: false as const,
      reason: 'invalid_props' as const,
    }
    const instruction = buildComponentRepairInstruction(rejection)
    expect(instruction).toContain('invalid_props')
    expect(instruction).toContain('credit_kickoff')
  })
})

describe('serializeComponentFrame', () => {
  test('emits a custom-event SSE frame standard OpenAI clients ignore', () => {
    const frame: KhalaComponentFrame = {
      component: 'credit_kickoff',
      id: 'cmp_01',
      props: { amountCents: 50000, label: 'Go' },
      v: 1,
    }
    const wire = serializeComponentFrame(frame)
    expect(wire.startsWith(`event: ${OA_COMPONENT_SSE_EVENT}\n`)).toBe(true)
    expect(wire).toContain('data: ')
    expect(wire.endsWith('\n\n')).toBe(true)
    const dataLine = wire
      .split('\n')
      .find(line => line.startsWith('data: '))!
      .replace('data: ', '')
    const parsed = JSON.parse(dataLine) as KhalaComponentFrame
    expect(parsed.component).toBe('credit_kickoff')
    expect(parsed.v).toBe(1)
  })
})

describe('runComponentChannel (the gateway transform)', () => {
  test('splits prose + emits validated frames; drops invalid ones', async () => {
    const completion = [
      'Great, let us kick this off.',
      fenced('{"component":"credit_kickoff","props":{"amountCents":50000,"label":"Kick off with $500 in credits"}}'),
      'And here is an invalid card that must be dropped:',
      fenced('{"component":"credit_kickoff","props":{"amountCents":"bad"}}'),
      'And an unknown one too:',
      fenced('{"component":"totally_made_up","props":{}}'),
    ].join('\n\n')
    const output = await runComponentChannel(completion, {})
    // One valid frame survives; the invalid + unknown are dropped.
    expect(output.frames).toHaveLength(1)
    expect(output.frames[0]?.component).toBe('credit_kickoff')
    expect(output.dropped).toHaveLength(2)
    // Prose is preserved (fences stripped).
    expect(output.prose).toContain('kick this off')
    expect(output.prose).not.toContain('oa-component')
  })

  test('identity-guards the prose channel', async () => {
    const completion = 'I am built on Gemini and here to help.'
    const output = await runComponentChannel(completion, {})
    // The leaked first-person provenance is redacted from the prose.
    expect(output.prose.toLowerCase()).not.toContain('built on gemini')
  })

  test('mints stable per-card ids', async () => {
    const completion = [
      fenced('{"component":"human_handoff","props":{"reason":"a","contact":"b"}}'),
      fenced('{"component":"human_handoff","props":{"reason":"c","contact":"d"}}'),
    ].join('\n\n')
    const output = await runComponentChannel(completion, {})
    expect(output.frames.map(f => f.id)).toEqual(['cmp_1', 'cmp_2'])
  })

  test('a card-only turn yields empty prose + the frame', async () => {
    const completion = fenced(
      '{"component":"consent_gate","props":{"scope":"intake","dataPractices":"redacted","required":true}}',
    )
    const output = await runComponentChannel(completion, {})
    expect(output.prose).toBe('')
    expect(output.frames).toHaveLength(1)
  })
})
