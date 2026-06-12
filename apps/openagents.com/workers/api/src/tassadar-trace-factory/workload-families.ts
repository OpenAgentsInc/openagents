/**
 * Workload program families v0.1 for the Tassadar verified trace factory
 * (issue #4748). Each family is a deterministic builder: (familyId,
 * inputSeed, stepCount) regenerates the exact compiled numeric model and
 * input steps, which is what makes replay-from-clean-checkout possible
 * without shipping models inside the corpus.
 *
 * Families are the unit of the training split policy (held-out FAMILIES,
 * never seeds). Five synthetic families exercise the executor's full
 * v1 surface — affine wiring, hard-max keyed reads, cum-sum channels,
 * relu-gated multiplication, and channel writes — and one anchor family
 * replays the committed psionic-compiled loop-sum fixture for
 * cross-implementation (Rust/TS) digest parity.
 *
 * Iron rule honored here: builders produce workloads only. Expected
 * digests are computed FROM execution and never ship in a generation
 * assignment.
 */
import type { TassadarAlmNumericModel } from '@openagentsinc/tassadar-executor'

import { canonicalJson, sha256HexOfText } from './trace-record'

export const TASSADAR_FAMILY_BUILDER_VERSION = 'family_builder.v0.1'

export const TASSADAR_TRACE_FAMILY_IDS = [
  'family.arithmetic_carry.v1',
  'family.memory_load_store.v1',
  'family.branch_gated_control.v1',
  'family.application_state_machine.v1',
  'family.near_miss_lookup.v1',
  'family.stack_loop_sum.compiled.v1',
] as const
export type TassadarTraceFamilyId = (typeof TASSADAR_TRACE_FAMILY_IDS)[number]

/** Step bound keeping every cum-sum inside the 2^53 exactness window. */
export const TASSADAR_FAMILY_MAX_STEP_COUNT = 4096

export type TassadarFamilyWorkload = Readonly<{
  familyId: TassadarTraceFamilyId
  inputSeed: string
  model: TassadarAlmNumericModel
  steps: ReadonlyArray<ReadonlyArray<number>>
  compilerHash: string
}>

export type TassadarFamilyBuildFailure = Readonly<
  | { kind: 'unknown_family'; detail: string }
  | { kind: 'step_count_out_of_range'; detail: string }
  | { kind: 'invalid_seed'; detail: string }
>

export type TassadarFamilyBuildResult =
  | Readonly<{ ok: true; workload: TassadarFamilyWorkload }>
  | Readonly<{ ok: false; failure: TassadarFamilyBuildFailure }>

/** splitmix64 — the deterministic PRNG behind every generated input. */
const SPLITMIX_GAMMA = 0x9e3779b97f4a7c15n
const U64 = (1n << 64n) - 1n

const splitmix64 = (seed: bigint): (() => bigint) => {
  let state = seed & U64

  return () => {
    state = (state + SPLITMIX_GAMMA) & U64
    let z = state
    z = ((z ^ (z >> 30n)) * 0xbf58476d1ce4e5b9n) & U64
    z = ((z ^ (z >> 27n)) * 0x94d049bb133111ebn) & U64

    return (z ^ (z >> 31n)) & U64
  }
}

const seedFromHex = (inputSeed: string): bigint | null =>
  /^[0-9a-f]{1,16}$/.test(inputSeed) ? BigInt(`0x${inputSeed}`) : null

const boundedInt = (next: () => bigint, bound: number): number =>
  Number(next() % BigInt(bound))

export const deriveRecordSeed = async (
  masterSeed: string,
  familyId: string,
  recordIndex: number,
): Promise<string> => {
  const digest = await sha256HexOfText(
    `${TASSADAR_FAMILY_BUILDER_VERSION}|${masterSeed}|${familyId}|${recordIndex}`,
  )

  return digest.slice(0, 16)
}

type ModelDraft = Readonly<{
  inputFieldCount: number
  slotCount: number
  layerCount: number
  seedWrites: ReadonlyArray<readonly [number, number, number]>
  wiring: TassadarAlmNumericModel['wiring']
  attention: TassadarAlmNumericModel['attention']
  ffn: TassadarAlmNumericModel['ffn']
  writes: TassadarAlmNumericModel['writes']
  outputSlots: ReadonlyArray<number>
}>

const finishModel = async (
  familyId: TassadarTraceFamilyId,
  inputSeed: string,
  draft: ModelDraft,
): Promise<{ compilerHash: string; model: TassadarAlmNumericModel }> => {
  const body = {
    attention: draft.attention,
    ffn: draft.ffn,
    input_field_count: draft.inputFieldCount,
    layer_count: draft.layerCount,
    model_id: `${familyId}.${inputSeed}`,
    output_slots: draft.outputSlots,
    schema_version: 1,
    seed_writes: draft.seedWrites,
    slot_count: draft.slotCount,
    wiring: draft.wiring,
    writes: draft.writes,
  }
  const graphDigest = await sha256HexOfText(
    `tassadar_alm_numeric_model|${canonicalJson(body)}`,
  )
  const compilerHash = await sha256HexOfText(
    `${TASSADAR_FAMILY_BUILDER_VERSION}|${familyId}`,
  )

  return {
    compilerHash,
    model: {
      ...body,
      bundle_digest: graphDigest,
      graph_digest: graphDigest,
    },
  }
}

const inputWire = (
  outSlot: number,
  inputField: number,
  phase: number,
): TassadarAlmNumericModel['wiring'][number] => ({
  bias: 0,
  input_field: inputField,
  out_slot: outSlot,
  phase,
  terms: [],
})

const INPUT_VALUE_BOUND = 1 << 20
const MEMORY_VALUE_BOUND = 1 << 30
const LEDGER_AMOUNT_BOUND = 1 << 16

/**
 * family.arithmetic_carry.v1 — affine sums, relu-gated products, and
 * two carry-style cum-sum accumulators over four bounded input lanes.
 */
const buildArithmeticCarry = async (
  inputSeed: string,
  seed: bigint,
  stepCount: number,
): Promise<{ compilerHash: string; model: TassadarAlmNumericModel; steps: Array<Array<number>> }> => {
  const { compilerHash, model } = await finishModel(
    'family.arithmetic_carry.v1',
    inputSeed,
    {
      attention: [
        { cum_sum: { channel: 0, out_slot: 6, phase: 2, value_slot: 4 } },
        { cum_sum: { channel: 1, out_slot: 7, phase: 2, value_slot: 5 } },
      ],
      ffn: [{ gate_slot: 1, out_slot: 5, phase: 1, value_slot: 0 }],
      inputFieldCount: 4,
      layerCount: 4,
      outputSlots: [4, 5, 6, 7, 8, 3],
      seedWrites: [],
      slotCount: 9,
      wiring: [
        inputWire(0, 0, 0),
        inputWire(1, 1, 0),
        inputWire(2, 2, 0),
        inputWire(3, 3, 0),
        {
          bias: 0,
          input_field: null,
          out_slot: 4,
          phase: 1,
          terms: [
            [1, 0],
            [1, 1],
            [1, 2],
          ],
        },
        {
          bias: 0,
          input_field: null,
          out_slot: 8,
          phase: 3,
          terms: [
            [1, 6],
            [-1, 7],
            [1, 3],
          ],
        },
      ],
      writes: [],
    },
  )
  const next = splitmix64(seed)
  const steps: Array<Array<number>> = []
  for (let step = 0; step < stepCount; step += 1) {
    steps.push([
      boundedInt(next, INPUT_VALUE_BOUND),
      boundedInt(next, INPUT_VALUE_BOUND),
      boundedInt(next, INPUT_VALUE_BOUND),
      boundedInt(next, INPUT_VALUE_BOUND),
    ])
  }

  return { compilerHash, model, steps }
}

const memoryModelDraft = (
  seedWrites: ReadonlyArray<readonly [number, number, number]>,
): ModelDraft => ({
  attention: [
    { keyed_read: { channel: 0, out_slot: 3, phase: 1, query_slot: 2 } },
  ],
  ffn: [],
  inputFieldCount: 3,
  layerCount: 3,
  outputSlots: [0, 1, 2, 3, 4],
  seedWrites,
  slotCount: 5,
  wiring: [
    inputWire(0, 0, 0),
    inputWire(1, 1, 0),
    inputWire(2, 2, 0),
    {
      bias: 0,
      input_field: null,
      out_slot: 4,
      phase: 2,
      terms: [
        [1, 3],
        [1, 1],
      ],
    },
  ],
  writes: [{ channel: 0, key_slot: 0, value_slot: 1 }],
})

/**
 * family.memory_load_store.v1 — keyed channel writes and exact-match
 * hard-max reads; every read key provably exists at read time.
 */
const buildMemoryLoadStore = async (
  inputSeed: string,
  seed: bigint,
  stepCount: number,
): Promise<{ compilerHash: string; model: TassadarAlmNumericModel; steps: Array<Array<number>> }> => {
  const next = splitmix64(seed)
  const seedWrites: Array<readonly [number, number, number]> = []
  const writtenKeys: Array<number> = []
  for (let index = 0; index < 16; index += 1) {
    const uniqueKey = (index + 1) * 17 + boundedInt(next, 13)
    seedWrites.push([0, uniqueKey, boundedInt(next, MEMORY_VALUE_BOUND)])
    writtenKeys.push(uniqueKey)
  }
  const { compilerHash, model } = await finishModel(
    'family.memory_load_store.v1',
    inputSeed,
    memoryModelDraft(seedWrites),
  )
  const steps: Array<Array<number>> = []
  for (let step = 0; step < stepCount; step += 1) {
    const writeKey = 1024 + boundedInt(next, INPUT_VALUE_BOUND)
    const writeValue = boundedInt(next, MEMORY_VALUE_BOUND)
    const readIndex = boundedInt(next, writtenKeys.length)
    const readKey = writtenKeys[readIndex] ?? writtenKeys[0] ?? 17
    steps.push([writeKey, writeValue, readKey])
    writtenKeys.push(writeKey)
  }

  return { compilerHash, model, steps }
}

/**
 * family.branch_gated_control.v1 — branch selection via relu gating
 * (out = b·x + (1−b)·y) with a cum-sum branch-taken counter standing in
 * for control-flow state.
 */
const buildBranchGatedControl = async (
  inputSeed: string,
  seed: bigint,
  stepCount: number,
): Promise<{ compilerHash: string; model: TassadarAlmNumericModel; steps: Array<Array<number>> }> => {
  const { compilerHash, model } = await finishModel(
    'family.branch_gated_control.v1',
    inputSeed,
    {
      attention: [
        { cum_sum: { channel: 0, out_slot: 7, phase: 4, value_slot: 2 } },
        { cum_sum: { channel: 1, out_slot: 8, phase: 4, value_slot: 6 } },
      ],
      ffn: [
        { gate_slot: 2, out_slot: 4, phase: 2, value_slot: 0 },
        { gate_slot: 3, out_slot: 5, phase: 2, value_slot: 1 },
      ],
      inputFieldCount: 3,
      layerCount: 5,
      outputSlots: [2, 6, 7, 8, 4, 5],
      seedWrites: [],
      slotCount: 9,
      wiring: [
        inputWire(0, 0, 0),
        inputWire(1, 1, 0),
        inputWire(2, 2, 0),
        { bias: 1, input_field: null, out_slot: 3, phase: 1, terms: [[-1, 2]] },
        {
          bias: 0,
          input_field: null,
          out_slot: 6,
          phase: 3,
          terms: [
            [1, 4],
            [1, 5],
          ],
        },
      ],
      writes: [],
    },
  )
  const next = splitmix64(seed)
  const steps: Array<Array<number>> = []
  for (let step = 0; step < stepCount; step += 1) {
    steps.push([
      boundedInt(next, INPUT_VALUE_BOUND),
      boundedInt(next, INPUT_VALUE_BOUND),
      boundedInt(next, 2),
    ])
  }

  return { compilerHash, model, steps }
}

/**
 * family.application_state_machine.v1 — the economic-workload family: a
 * ledger of account balances updated by keyed read-modify-write with a
 * cum-sum deposit-volume channel. Held out of training by policy.
 */
const buildApplicationStateMachine = async (
  inputSeed: string,
  seed: bigint,
  stepCount: number,
): Promise<{ compilerHash: string; model: TassadarAlmNumericModel; steps: Array<Array<number>> }> => {
  const next = splitmix64(seed)
  const accountCount = 8 + boundedInt(next, 25)
  const seedWrites: Array<readonly [number, number, number]> = []
  for (let account = 1; account <= accountCount; account += 1) {
    seedWrites.push([0, account, boundedInt(next, LEDGER_AMOUNT_BOUND)])
  }
  const { compilerHash, model } = await finishModel(
    'family.application_state_machine.v1',
    inputSeed,
    {
      attention: [
        { keyed_read: { channel: 0, out_slot: 3, phase: 1, query_slot: 0 } },
        { cum_sum: { channel: 1, out_slot: 6, phase: 3, value_slot: 5 } },
      ],
      ffn: [{ gate_slot: 1, out_slot: 5, phase: 2, value_slot: 2 }],
      inputFieldCount: 2,
      layerCount: 4,
      outputSlots: [0, 3, 4, 6, 1],
      seedWrites,
      slotCount: 7,
      wiring: [
        inputWire(0, 0, 0),
        inputWire(1, 1, 0),
        { bias: 1, input_field: null, out_slot: 2, phase: 0, terms: [] },
        {
          bias: 0,
          input_field: null,
          out_slot: 4,
          phase: 2,
          terms: [
            [1, 3],
            [1, 1],
          ],
        },
      ],
      writes: [{ channel: 0, key_slot: 0, value_slot: 4 }],
    },
  )
  const steps: Array<Array<number>> = []
  for (let step = 0; step < stepCount; step += 1) {
    const account = 1 + boundedInt(next, accountCount)
    const amount =
      boundedInt(next, 2 * LEDGER_AMOUNT_BOUND) - LEDGER_AMOUNT_BOUND
    steps.push([account, amount])
  }

  return { compilerHash, model, steps }
}

/**
 * family.near_miss_lookup.v1 — the lookup adversary: the key table is
 * seeded in tight ±1 clusters so the hard-max parabolic score must
 * discriminate exact matches from immediate neighbors.
 */
const buildNearMissLookup = async (
  inputSeed: string,
  seed: bigint,
  stepCount: number,
): Promise<{ compilerHash: string; model: TassadarAlmNumericModel; steps: Array<Array<number>> }> => {
  const next = splitmix64(seed)
  const seedWrites: Array<readonly [number, number, number]> = []
  const writtenKeys: Array<number> = []
  for (let cluster = 0; cluster < 12; cluster += 1) {
    const center = 100 + cluster * 7 + boundedInt(next, 3)
    for (const key of [center - 1, center, center + 1]) {
      if (!writtenKeys.includes(key)) {
        seedWrites.push([0, key, boundedInt(next, MEMORY_VALUE_BOUND)])
        writtenKeys.push(key)
      }
    }
  }
  const { compilerHash, model } = await finishModel(
    'family.near_miss_lookup.v1',
    inputSeed,
    memoryModelDraft(seedWrites),
  )
  const steps: Array<Array<number>> = []
  for (let step = 0; step < stepCount; step += 1) {
    const base = writtenKeys[boundedInt(next, writtenKeys.length)] ?? 100
    const writeKey = base + (boundedInt(next, 2) * 2 - 1)
    const writeValue = boundedInt(next, MEMORY_VALUE_BOUND)
    const readKey = writtenKeys[boundedInt(next, writtenKeys.length)] ?? 100
    steps.push([writeKey, writeValue, readKey])
    if (!writtenKeys.includes(writeKey)) writtenKeys.push(writeKey)
  }

  return { compilerHash, model, steps }
}

/**
 * Builds the deterministic workload for one (familyId, inputSeed,
 * stepCount) triple. The compiled anchor family is intentionally NOT
 * built here — its model is the committed psionic fixture and is
 * supplied by the caller through `anchorWorkloadFromFixture`.
 */
export const buildFamilyWorkload = async (
  input: Readonly<{
    familyId: string
    inputSeed: string
    stepCount: number
  }>,
): Promise<TassadarFamilyBuildResult> => {
  if (
    input.stepCount < 1 ||
    input.stepCount > TASSADAR_FAMILY_MAX_STEP_COUNT
  ) {
    return {
      failure: {
        detail: `step count ${input.stepCount} is outside [1, ${TASSADAR_FAMILY_MAX_STEP_COUNT}]`,
        kind: 'step_count_out_of_range',
      },
      ok: false,
    }
  }
  const seed = seedFromHex(input.inputSeed)
  if (seed === null) {
    return {
      failure: {
        detail: `input seed ${JSON.stringify(input.inputSeed)} is not 1-16 lowercase hex characters`,
        kind: 'invalid_seed',
      },
      ok: false,
    }
  }
  const builders: Record<
    string,
    | ((
        inputSeed: string,
        seed: bigint,
        stepCount: number,
      ) => Promise<{
        compilerHash: string
        model: TassadarAlmNumericModel
        steps: Array<Array<number>>
      }>)
    | undefined
  > = {
    'family.application_state_machine.v1': buildApplicationStateMachine,
    'family.arithmetic_carry.v1': buildArithmeticCarry,
    'family.branch_gated_control.v1': buildBranchGatedControl,
    'family.memory_load_store.v1': buildMemoryLoadStore,
    'family.near_miss_lookup.v1': buildNearMissLookup,
  }
  const builder = builders[input.familyId]
  if (builder === undefined) {
    return {
      failure: {
        detail: `family ${JSON.stringify(input.familyId)} has no v0.1 builder (the compiled anchor family uses the committed fixture)`,
        kind: 'unknown_family',
      },
      ok: false,
    }
  }
  const built = await builder(input.inputSeed, seed, input.stepCount)

  return {
    ok: true,
    workload: {
      compilerHash: built.compilerHash,
      familyId: input.familyId as TassadarTraceFamilyId,
      inputSeed: input.inputSeed,
      model: built.model,
      steps: built.steps,
    },
  }
}

/**
 * family.stack_loop_sum.compiled.v1 — the psionic-compiled anchor. The
 * caller provides the committed fixture model/steps; the input seed is
 * the executed prefix length, so every anchor record is a genuinely
 * executed prefix of the committed interpreter schedule.
 */
export const anchorWorkloadFromFixture = (
  input: Readonly<{
    model: TassadarAlmNumericModel
    fixtureSteps: ReadonlyArray<ReadonlyArray<number>>
    fixtureBundleDigest: string
    stepCount: number
  }>,
): TassadarFamilyBuildResult => {
  if (input.stepCount < 1 || input.stepCount > input.fixtureSteps.length) {
    return {
      failure: {
        detail: `anchor prefix ${input.stepCount} is outside [1, ${input.fixtureSteps.length}]`,
        kind: 'step_count_out_of_range',
      },
      ok: false,
    }
  }

  return {
    ok: true,
    workload: {
      compilerHash: input.fixtureBundleDigest,
      familyId: 'family.stack_loop_sum.compiled.v1',
      inputSeed: input.stepCount.toString(16).padStart(4, '0'),
      model: input.model,
      steps: input.fixtureSteps.slice(0, input.stepCount),
    },
  }
}
