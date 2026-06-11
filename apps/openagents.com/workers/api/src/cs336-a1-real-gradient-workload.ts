/**
 * Real-gradient CS336 A1 training workload (issue #4678).
 *
 * This module is the dispatchable monorepo mirror of the Psionic
 * `psion_cs336_a1_real_gradient_reference_v1` lane: a tiny A1-shaped
 * language model (embedding -> RMSNorm -> single-head causal softmax
 * attention -> RMSNorm -> SwiGLU -> RMSNorm -> unembedding ->
 * cross-entropy) trained with hand-derived analytic gradients in f64.
 * Contributor devices each compute the full-batch gradient of their own
 * data shard per synchronous SGD step; the aggregation averages shard
 * gradients and applies the step; validation loss is measured on a
 * held-out stream after every step.
 *
 * Verification contract:
 * - every shard gradient, parameter state, and validation evaluation has
 *   a deterministic SHA-256 digest commitment, so the Worker's
 *   `deterministic_recompute` class can verify any shard step by
 *   re-execution on another device;
 * - all transcendentals (exp/log) are computed with engine-independent
 *   IEEE-754 arithmetic (`detExp`/`detLn`), so digests match bit-for-bit
 *   across x86_64 Linux and Apple Silicon macOS;
 * - the analytic gradient of every parameter tensor is pinned against
 *   central finite differences in the committed test, mirroring the
 *   Psionic lane's gradient-correctness bar.
 *
 * Everything here is public-safe by construction: symbol ids, digests,
 * losses, and refs only. No wallet, payment, or private material.
 */

export type Cs336A1RealGradientConfig = Readonly<{
  dFf: number
  dModel: number
  learningRate: number
  sequenceLength: number
  shardCount: number
  stepCount: number
  structureBias: number
  trainSequencesPerShard: number
  validationSequenceCount: number
  vocabularySize: number
}>

/**
 * The bounded run configuration for the #4678 multi-device run. Sized so
 * a full shard-gradient computation is real but bounded work on any
 * contributor CPU, while the parameter state stays small enough for
 * digest-committed state transport between devices.
 */
export const cs336A1RealGradientRunConfig: Cs336A1RealGradientConfig = {
  dFf: 24,
  dModel: 12,
  learningRate: 1.5,
  sequenceLength: 16,
  shardCount: 2,
  stepCount: 6,
  structureBias: 0.85,
  trainSequencesPerShard: 48,
  validationSequenceCount: 32,
  vocabularySize: 32,
}

const RmsNormEpsilon = 1e-5
const Ln2 = 0.6931471805599453

/** Exact power of two by binary exponentiation (every product is exact). */
const pow2 = (exponent: number): number => {
  let result = 1
  let base = exponent >= 0 ? 2 : 0.5
  let remaining = Math.abs(exponent)

  while (remaining > 0) {
    if ((remaining & 1) === 1) {
      result *= base
    }

    base *= base
    remaining >>= 1
  }

  return result
}

/**
 * Deterministic exp using only IEEE-754 +,-,*,/ and Math.round, so the
 * result is bit-identical across JS engines and CPU architectures
 * (Math.exp is implementation-defined and must not be used here).
 */
export const detExp = (x: number): number => {
  if (x < -700) {
    return 0
  }

  if (x > 700) {
    return Number.POSITIVE_INFINITY
  }

  const k = Math.round(x / Ln2)
  const r = x - k * Ln2
  let term = 1
  let sum = 1

  for (let i = 1; i <= 18; i += 1) {
    term = (term * r) / i
    sum += term
  }

  return sum * pow2(k)
}

const float64View = new DataView(new ArrayBuffer(8))

/**
 * Deterministic natural log for finite positive inputs, using exponent
 * extraction plus an atanh series (only IEEE +,-,*,/), bit-identical
 * across engines.
 */
export const detLn = (x: number): number => {
  float64View.setFloat64(0, x)
  let exponent = ((float64View.getUint32(0) >>> 20) & 0x7ff) - 1023
  let mantissa = x / pow2(exponent)

  if (mantissa > 1.5) {
    mantissa /= 2
    exponent += 1
  }

  const t = (mantissa - 1) / (mantissa + 1)
  const tSquared = t * t
  let term = t
  let sum = 0

  for (let i = 1; i <= 37; i += 2) {
    sum += term / i
    term *= tSquared
  }

  return exponent * Ln2 + 2 * sum
}

const sha256Hex = async (value: string): Promise<string> => {
  const digest = await crypto.subtle.digest(
    'SHA-256',
    new TextEncoder().encode(value),
  )

  return [...new Uint8Array(digest)]
    .map(byte => byte.toString(16).padStart(2, '0'))
    .join('')
}

const seededUint32s = async (
  seed: string,
  count: number,
): Promise<ReadonlyArray<number>> => {
  const values: number[] = []
  let counter = 0

  while (values.length < count) {
    const block = await sha256Hex(`${seed}:${counter}`)

    for (
      let offset = 0;
      offset + 8 <= block.length && values.length < count;
      offset += 8
    ) {
      values.push(Number.parseInt(block.slice(offset, offset + 8), 16))
    }

    counter += 1
  }

  return values
}

const TwoPow32 = 4294967296

const seededUnitFloats = async (
  seed: string,
  count: number,
): Promise<ReadonlyArray<number>> =>
  (await seededUint32s(seed, count)).map(value => value / TwoPow32)

type ParameterLayout = Readonly<{
  emb: number
  g1: number
  g2: number
  g3: number
  size: number
  w1: number
  w2: number
  w3: number
  wk: number
  wo: number
  wq: number
  wu: number
  wv: number
}>

export const parameterLayout = (
  config: Cs336A1RealGradientConfig,
): ParameterLayout => {
  const { dFf: h, dModel: d, vocabularySize: v } = config
  let offset = 0
  const take = (count: number): number => {
    const start = offset
    offset += count

    return start
  }

  return {
    emb: take(v * d),
    g1: take(d),
    wq: take(d * d),
    wk: take(d * d),
    wv: take(d * d),
    wo: take(d * d),
    g2: take(d),
    w1: take(d * h),
    w3: take(d * h),
    w2: take(h * d),
    g3: take(d),
    wu: take(d * v),
    size: offset,
  }
}

export const parameterCount = (config: Cs336A1RealGradientConfig): number =>
  parameterLayout(config).size

/**
 * Deterministic seeded initial parameters: uniform in [-scale, scale]
 * for matrices, exactly 1 for RMSNorm gains.
 */
export const initialParameters = async (
  config: Cs336A1RealGradientConfig,
  seedDigestHex: string,
): Promise<Float64Array> => {
  const layout = parameterLayout(config)
  const params = new Float64Array(layout.size)
  const floats = await seededUnitFloats(
    `cs336_a1.real_grad.init.${seedDigestHex}`,
    layout.size,
  )
  const matrixScale = 0.5 / Math.sqrt(config.dModel)

  for (let index = 0; index < layout.size; index += 1) {
    params[index] = (floats[index]! * 2 - 1) * matrixScale
  }

  for (let index = 0; index < config.dModel; index += 1) {
    params[layout.g1 + index] = 1
    params[layout.g2 + index] = 1
    params[layout.g3 + index] = 1
  }

  return params
}

const seededPermutation = async (
  seed: string,
  size: number,
): Promise<ReadonlyArray<number>> => {
  const values = Array.from({ length: size }, (_, index) => index)
  const draws = await seededUint32s(`${seed}:perm`, size)

  for (let index = size - 1; index > 0; index -= 1) {
    const swap = draws[size - 1 - index]! % (index + 1)
    const held = values[index]!

    values[index] = values[swap]!
    values[swap] = held
  }

  return values
}

/**
 * Deterministic synthetic symbol stream with learnable structure: the
 * next symbol follows a seeded permutation of the current symbol with
 * probability `structureBias`, otherwise it is uniform. The stream seed
 * binds every sequence to the master seed digest (the committed A1
 * tokenizer shard digest), the same provenance binding the A3 sweep
 * used.
 */
const generateSequence = async (
  config: Cs336A1RealGradientConfig,
  seedDigestHex: string,
  sequenceSeed: string,
  permutation: ReadonlyArray<number>,
): Promise<ReadonlyArray<number>> => {
  const length = config.sequenceLength + 1
  const draws = await seededUint32s(
    `cs336_a1.real_grad.stream.${seedDigestHex}.${sequenceSeed}`,
    2 * length + 1,
  )
  const symbols: number[] = [draws[0]! % config.vocabularySize]

  for (let index = 0; index < length - 1; index += 1) {
    const previous = symbols[index]!
    const branch = draws[2 * index + 1]! / TwoPow32
    const next =
      branch < config.structureBias
        ? permutation[previous]!
        : draws[2 * index + 2]! % config.vocabularySize

    symbols.push(next)
  }

  return symbols
}

const shardSequences = async (
  config: Cs336A1RealGradientConfig,
  seedDigestHex: string,
  shardIndex: number,
): Promise<ReadonlyArray<ReadonlyArray<number>>> => {
  const permutation = await seededPermutation(
    `cs336_a1.real_grad.structure.${seedDigestHex}`,
    config.vocabularySize,
  )

  return Promise.all(
    Array.from({ length: config.trainSequencesPerShard }, (_, sequence) =>
      generateSequence(
        config,
        seedDigestHex,
        `train:${shardIndex}:${sequence}`,
        permutation,
      ),
    ),
  )
}

const validationSequences = async (
  config: Cs336A1RealGradientConfig,
  seedDigestHex: string,
): Promise<ReadonlyArray<ReadonlyArray<number>>> => {
  const permutation = await seededPermutation(
    `cs336_a1.real_grad.structure.${seedDigestHex}`,
    config.vocabularySize,
  )

  return Promise.all(
    Array.from({ length: config.validationSequenceCount }, (_, sequence) =>
      generateSequence(config, seedDigestHex, `val:${sequence}`, permutation),
    ),
  )
}

const rmsNormForward = (
  x: Float64Array,
  offset: number,
  gains: Float64Array,
  gainOffset: number,
  out: Float64Array,
  outOffset: number,
  d: number,
): number => {
  let sumSquares = 0

  for (let index = 0; index < d; index += 1) {
    const value = x[offset + index]!

    sumSquares += value * value
  }

  const rms = Math.sqrt(sumSquares / d + RmsNormEpsilon)

  for (let index = 0; index < d; index += 1) {
    out[outOffset + index] = (x[offset + index]! / rms) * gains[gainOffset + index]!
  }

  return rms
}

const rmsNormBackward = (
  x: Float64Array,
  offset: number,
  gains: Float64Array,
  gainOffset: number,
  rms: number,
  dy: Float64Array,
  dyOffset: number,
  dx: Float64Array,
  dxOffset: number,
  dGains: Float64Array,
  dGainOffset: number,
  d: number,
): void => {
  let dot = 0

  for (let index = 0; index < d; index += 1) {
    dot += dy[dyOffset + index]! * gains[gainOffset + index]! * x[offset + index]!
  }

  const cubed = rms * rms * rms

  for (let index = 0; index < d; index += 1) {
    dx[dxOffset + index] = dx[dxOffset + index]! +
      (dy[dyOffset + index]! * gains[gainOffset + index]!) / rms -
      (x[offset + index]! * dot) / (d * cubed)
    dGains[dGainOffset + index] = dGains[dGainOffset + index]! + (dy[dyOffset + index]! * x[offset + index]!) / rms
  }
}

type SequenceWork = Readonly<{
  gradient: Float64Array | null
  loss: number
}>

/**
 * Forward (and optionally backward) pass over one sequence. The
 * backward pass accumulates analytic parameter gradients scaled by
 * `lossWeight` into `gradientOut`.
 */
const sequencePass = (
  config: Cs336A1RealGradientConfig,
  params: Float64Array,
  symbols: ReadonlyArray<number>,
  lossWeight: number,
  gradientOut: Float64Array | null,
): SequenceWork => {
  const { dFf: h, dModel: d, sequenceLength: t, vocabularySize: v } = config
  const layout = parameterLayout(config)
  const sqrtD = Math.sqrt(d)

  const h0 = new Float64Array(t * d)
  const n1 = new Float64Array(t * d)
  const rms1 = new Float64Array(t)
  const q = new Float64Array(t * d)
  const k = new Float64Array(t * d)
  const vv = new Float64Array(t * d)
  const alpha = new Float64Array(t * t)
  const mix = new Float64Array(t * d)
  const h1 = new Float64Array(t * d)
  const n2 = new Float64Array(t * d)
  const rms2 = new Float64Array(t)
  const gateIn = new Float64Array(t * h)
  const gateSig = new Float64Array(t * h)
  const valuePath = new Float64Array(t * h)
  const swiglu = new Float64Array(t * h)
  const h2 = new Float64Array(t * d)
  const n3 = new Float64Array(t * d)
  const rms3 = new Float64Array(t)
  const probs = new Float64Array(t * v)

  let loss = 0

  for (let pos = 0; pos < t; pos += 1) {
    const input = symbols[pos]!

    for (let i = 0; i < d; i += 1) {
      h0[pos * d + i] = params[layout.emb + input * d + i]!
    }

    rms1[pos] = rmsNormForward(h0, pos * d, params, layout.g1, n1, pos * d, d)

    for (let j = 0; j < d; j += 1) {
      let sumQ = 0
      let sumK = 0
      let sumV = 0

      for (let i = 0; i < d; i += 1) {
        const activation = n1[pos * d + i]!

        sumQ += activation * params[layout.wq + i * d + j]!
        sumK += activation * params[layout.wk + i * d + j]!
        sumV += activation * params[layout.wv + i * d + j]!
      }

      q[pos * d + j] = sumQ
      k[pos * d + j] = sumK
      vv[pos * d + j] = sumV
    }
  }

  for (let pos = 0; pos < t; pos += 1) {
    let maxScore = Number.NEGATIVE_INFINITY
    const scores = new Float64Array(pos + 1)

    for (let u = 0; u <= pos; u += 1) {
      let score = 0

      for (let i = 0; i < d; i += 1) {
        score += q[pos * d + i]! * k[u * d + i]!
      }

      score /= sqrtD
      scores[u] = score

      if (score > maxScore) {
        maxScore = score
      }
    }

    let total = 0

    for (let u = 0; u <= pos; u += 1) {
      const value = detExp(scores[u]! - maxScore)

      alpha[pos * t + u] = value
      total += value
    }

    for (let u = 0; u <= pos; u += 1) {
      alpha[pos * t + u] = alpha[pos * t + u]! / total
    }

    for (let i = 0; i < d; i += 1) {
      let sum = 0

      for (let u = 0; u <= pos; u += 1) {
        sum += alpha[pos * t + u]! * vv[u * d + i]!
      }

      mix[pos * d + i] = sum
    }

    for (let j = 0; j < d; j += 1) {
      let sum = 0

      for (let i = 0; i < d; i += 1) {
        sum += mix[pos * d + i]! * params[layout.wo + i * d + j]!
      }

      h1[pos * d + j] = h0[pos * d + j]! + sum
    }

    rms2[pos] = rmsNormForward(h1, pos * d, params, layout.g2, n2, pos * d, d)

    for (let j = 0; j < h; j += 1) {
      let sumGate = 0
      let sumValue = 0

      for (let i = 0; i < d; i += 1) {
        const activation = n2[pos * d + i]!

        sumGate += activation * params[layout.w1 + i * h + j]!
        sumValue += activation * params[layout.w3 + i * h + j]!
      }

      const sigmoid = 1 / (1 + detExp(-sumGate))

      gateIn[pos * h + j] = sumGate
      gateSig[pos * h + j] = sigmoid
      valuePath[pos * h + j] = sumValue
      swiglu[pos * h + j] = sumGate * sigmoid * sumValue
    }

    for (let i = 0; i < d; i += 1) {
      let sum = 0

      for (let j = 0; j < h; j += 1) {
        sum += swiglu[pos * h + j]! * params[layout.w2 + j * d + i]!
      }

      h2[pos * d + i] = h1[pos * d + i]! + sum
    }

    rms3[pos] = rmsNormForward(h2, pos * d, params, layout.g3, n3, pos * d, d)

    let maxLogit = Number.NEGATIVE_INFINITY
    const logits = new Float64Array(v)

    for (let j = 0; j < v; j += 1) {
      let sum = 0

      for (let i = 0; i < d; i += 1) {
        sum += n3[pos * d + i]! * params[layout.wu + i * v + j]!
      }

      logits[j] = sum

      if (sum > maxLogit) {
        maxLogit = sum
      }
    }

    let expSum = 0

    for (let j = 0; j < v; j += 1) {
      const value = detExp(logits[j]! - maxLogit)

      probs[pos * v + j] = value
      expSum += value
    }

    for (let j = 0; j < v; j += 1) {
      probs[pos * v + j] = probs[pos * v + j]! / expSum
    }

    const target = symbols[pos + 1]!

    loss += detLn(expSum) + maxLogit - logits[target]!
  }

  if (gradientOut === null) {
    return { gradient: null, loss }
  }

  const grad = gradientOut
  const dH0 = new Float64Array(t * d)
  const dN1 = new Float64Array(t * d)
  const dQ = new Float64Array(t * d)
  const dK = new Float64Array(t * d)
  const dV = new Float64Array(t * d)

  for (let pos = t - 1; pos >= 0; pos -= 1) {
    const target = symbols[pos + 1]!
    const dZ = new Float64Array(v)

    for (let j = 0; j < v; j += 1) {
      dZ[j] = (probs[pos * v + j]! - (j === target ? 1 : 0)) * lossWeight
    }

    const dN3 = new Float64Array(d)

    for (let i = 0; i < d; i += 1) {
      let sum = 0

      for (let j = 0; j < v; j += 1) {
        grad[layout.wu + i * v + j] = grad[layout.wu + i * v + j]! + n3[pos * d + i]! * dZ[j]!
        sum += params[layout.wu + i * v + j]! * dZ[j]!
      }

      dN3[i] = sum
    }

    const dH2 = new Float64Array(d)

    rmsNormBackward(
      h2,
      pos * d,
      params,
      layout.g3,
      rms3[pos]!,
      dN3,
      0,
      dH2,
      0,
      grad,
      layout.g3,
      d,
    )

    const dH1 = new Float64Array(d)

    for (let i = 0; i < d; i += 1) {
      dH1[i] = dH2[i]!
    }

    const dSwiglu = new Float64Array(h)

    for (let j = 0; j < h; j += 1) {
      let sum = 0

      for (let i = 0; i < d; i += 1) {
        grad[layout.w2 + j * d + i] = grad[layout.w2 + j * d + i]! + swiglu[pos * h + j]! * dH2[i]!
        sum += params[layout.w2 + j * d + i]! * dH2[i]!
      }

      dSwiglu[j] = sum
    }

    const dN2 = new Float64Array(d)

    for (let j = 0; j < h; j += 1) {
      const sigmoid = gateSig[pos * h + j]!
      const gate = gateIn[pos * h + j]!
      const silu = gate * sigmoid
      const dValue = dSwiglu[j]! * silu
      const dGate =
        dSwiglu[j]! *
        valuePath[pos * h + j]! *
        (sigmoid * (1 + gate * (1 - sigmoid)))

      for (let i = 0; i < d; i += 1) {
        grad[layout.w1 + i * h + j] = grad[layout.w1 + i * h + j]! + n2[pos * d + i]! * dGate
        grad[layout.w3 + i * h + j] = grad[layout.w3 + i * h + j]! + n2[pos * d + i]! * dValue
        dN2[i] = dN2[i]! +
          params[layout.w1 + i * h + j]! * dGate +
          params[layout.w3 + i * h + j]! * dValue
      }
    }

    rmsNormBackward(
      h1,
      pos * d,
      params,
      layout.g2,
      rms2[pos]!,
      dN2,
      0,
      dH1,
      0,
      grad,
      layout.g2,
      d,
    )

    for (let i = 0; i < d; i += 1) {
      dH0[pos * d + i] = dH0[pos * d + i]! + dH1[i]!
    }

    const dMix = new Float64Array(d)

    for (let i = 0; i < d; i += 1) {
      let sum = 0

      for (let j = 0; j < d; j += 1) {
        grad[layout.wo + i * d + j] = grad[layout.wo + i * d + j]! + mix[pos * d + i]! * dH1[j]!
        sum += params[layout.wo + i * d + j]! * dH1[j]!
      }

      dMix[i] = sum
    }

    const dAlphaRow = new Float64Array(pos + 1)
    let rowDot = 0

    for (let u = 0; u <= pos; u += 1) {
      let sum = 0

      for (let i = 0; i < d; i += 1) {
        sum += dMix[i]! * vv[u * d + i]!
      }

      dAlphaRow[u] = sum
      rowDot += alpha[pos * t + u]! * sum
    }

    for (let u = 0; u <= pos; u += 1) {
      const dScore = alpha[pos * t + u]! * (dAlphaRow[u]! - rowDot)

      for (let i = 0; i < d; i += 1) {
        dQ[pos * d + i] = dQ[pos * d + i]! + (dScore * k[u * d + i]!) / sqrtD
        dK[u * d + i] = dK[u * d + i]! + (dScore * q[pos * d + i]!) / sqrtD
        dV[u * d + i] = dV[u * d + i]! + alpha[pos * t + u]! * dMix[i]!
      }
    }
  }

  for (let pos = 0; pos < t; pos += 1) {
    for (let i = 0; i < d; i += 1) {
      let sum = 0

      for (let j = 0; j < d; j += 1) {
        grad[layout.wq + i * d + j] = grad[layout.wq + i * d + j]! + n1[pos * d + i]! * dQ[pos * d + j]!
        grad[layout.wk + i * d + j] = grad[layout.wk + i * d + j]! + n1[pos * d + i]! * dK[pos * d + j]!
        grad[layout.wv + i * d + j] = grad[layout.wv + i * d + j]! + n1[pos * d + i]! * dV[pos * d + j]!
        sum +=
          params[layout.wq + i * d + j]! * dQ[pos * d + j]! +
          params[layout.wk + i * d + j]! * dK[pos * d + j]! +
          params[layout.wv + i * d + j]! * dV[pos * d + j]!
      }

      dN1[pos * d + i] = sum
    }

    rmsNormBackward(
      h0,
      pos * d,
      params,
      layout.g1,
      rms1[pos]!,
      dN1,
      pos * d,
      dH0,
      pos * d,
      grad,
      layout.g1,
      d,
    )

    const input = symbols[pos]!

    for (let i = 0; i < d; i += 1) {
      grad[layout.emb + input * d + i] = grad[layout.emb + input * d + i]! + dH0[pos * d + i]!
    }
  }

  return { gradient: grad, loss }
}

export type Cs336A1ShardGradientResult = Readonly<{
  dataUnitCount: number
  digestHex: string
  gradient: Float64Array
  gradientL2Norm: number
  shardIndex: number
  shardLoss: number
  stepIndex: number
}>

const digestForLabeledValues = async (
  label: string,
  values: ReadonlyArray<number>,
): Promise<string> => sha256Hex(JSON.stringify({ label, values }))

export const parameterStateDigest = async (
  seedDigestHex: string,
  stepIndex: number,
  params: Float64Array,
): Promise<string> =>
  digestForLabeledValues(
    `cs336_a1.real_grad.state.${seedDigestHex}.step_${stepIndex}`,
    Array.from(params),
  )

/**
 * The shard-gradient unit of contributor work: full-batch analytic
 * gradient of the shard's sequences at the given parameter state, with
 * a deterministic digest commitment over the exact gradient values.
 */
export const computeCs336A1ShardGradient = async (
  input: Readonly<{
    config: Cs336A1RealGradientConfig
    params: Float64Array
    seedDigestHex: string
    shardIndex: number
    stepIndex: number
  }>,
): Promise<Cs336A1ShardGradientResult> => {
  const { config } = input
  const sequences = await shardSequences(
    config,
    input.seedDigestHex,
    input.shardIndex,
  )
  const layout = parameterLayout(config)
  const gradient = new Float64Array(layout.size)
  const lossWeight = 1 / (sequences.length * config.sequenceLength)
  let totalLoss = 0

  for (const sequence of sequences) {
    totalLoss += sequencePass(
      config,
      input.params,
      sequence,
      lossWeight,
      gradient,
    ).loss
  }

  let normSquared = 0

  for (let index = 0; index < gradient.length; index += 1) {
    normSquared += gradient[index]! * gradient[index]!
  }

  const digestHex = await digestForLabeledValues(
    `cs336_a1.real_grad.shard_gradient.${input.seedDigestHex}.step_${input.stepIndex}.shard_${input.shardIndex}`,
    Array.from(gradient),
  )

  return {
    dataUnitCount: sequences.length * config.sequenceLength,
    digestHex,
    gradient,
    gradientL2Norm: Math.sqrt(normSquared),
    shardIndex: input.shardIndex,
    shardLoss: totalLoss * lossWeight,
    stepIndex: input.stepIndex,
  }
}

/**
 * Synchronous data-parallel SGD aggregation: average the shard
 * gradients (equal shard sizes) and apply one step.
 */
export const applyAggregatedSgdStep = (
  config: Cs336A1RealGradientConfig,
  params: Float64Array,
  shardGradients: ReadonlyArray<Float64Array>,
): Float64Array => {
  const next = new Float64Array(params.length)
  const shardCount = shardGradients.length

  for (let index = 0; index < params.length; index += 1) {
    let sum = 0

    for (const gradient of shardGradients) {
      sum += gradient[index]!
    }

    next[index] = params[index]! - (config.learningRate * sum) / shardCount
  }

  return next
}

export type Cs336A1ValidationResult = Readonly<{
  dataUnitCount: number
  digestHex: string
  validationLoss: number
}>

export const computeCs336A1ValidationLoss = async (
  input: Readonly<{
    config: Cs336A1RealGradientConfig
    params: Float64Array
    seedDigestHex: string
    stepIndex: number
  }>,
): Promise<Cs336A1ValidationResult> => {
  const { config } = input
  const sequences = await validationSequences(config, input.seedDigestHex)
  const lossWeight = 1 / (sequences.length * config.sequenceLength)
  let totalLoss = 0

  for (const sequence of sequences) {
    totalLoss += sequencePass(config, input.params, sequence, 1, null).loss
  }

  const validationLoss = totalLoss * lossWeight
  const digestHex = await digestForLabeledValues(
    `cs336_a1.real_grad.validation.${input.seedDigestHex}.step_${input.stepIndex}`,
    [validationLoss],
  )

  return {
    dataUnitCount: sequences.length * config.sequenceLength,
    digestHex,
    validationLoss,
  }
}

/** Numeric (central finite difference) loss gradient for the test bar. */
export const numericShardLossGradient = async (
  input: Readonly<{
    config: Cs336A1RealGradientConfig
    epsilon: number
    params: Float64Array
    seedDigestHex: string
    shardIndex: number
  }>,
): Promise<Float64Array> => {
  const { config } = input
  const sequences = await shardSequences(
    config,
    input.seedDigestHex,
    input.shardIndex,
  )
  const lossWeight = 1 / (sequences.length * config.sequenceLength)
  const shardLossAt = (params: Float64Array): number => {
    let total = 0

    for (const sequence of sequences) {
      total += sequencePass(config, params, sequence, 1, null).loss
    }

    return total * lossWeight
  }
  const gradient = new Float64Array(input.params.length)

  for (let index = 0; index < input.params.length; index += 1) {
    const held = input.params[index]!

    input.params[index] = held + input.epsilon

    const plus = shardLossAt(input.params)

    input.params[index] = held - input.epsilon

    const minus = shardLossAt(input.params)

    input.params[index] = held
    gradient[index] = (plus - minus) / (2 * input.epsilon)
  }

  return gradient
}

export type Cs336A1TrainingStepRecord = Readonly<{
  aggregatedStateDigestHex: string
  shardResults: ReadonlyArray<
    Readonly<{
      dataUnitCount: number
      digestHex: string
      gradientL2Norm: number
      shardIndex: number
      shardLoss: number
    }>
  >
  stepIndex: number
  validationLoss: number
}>

export type Cs336A1TrainingTrajectory = Readonly<{
  finalStateDigestHex: string
  initialStateDigestHex: string
  initialValidationLoss: number
  steps: ReadonlyArray<Cs336A1TrainingStepRecord>
}>

/**
 * Reference single-process trajectory runner (used by tests and by the
 * operator-side aggregation recompute). The multi-device run executes
 * exactly this trajectory with shard gradients computed on different
 * physical devices and digest-verified at each step.
 */
export const runCs336A1RealGradientTraining = async (
  config: Cs336A1RealGradientConfig,
  seedDigestHex: string,
): Promise<Cs336A1TrainingTrajectory> => {
  let params = await initialParameters(config, seedDigestHex)
  const initialStateDigestHex = await parameterStateDigest(
    seedDigestHex,
    0,
    params,
  )
  const initialValidationLoss = (
    await computeCs336A1ValidationLoss({
      config,
      params,
      seedDigestHex,
      stepIndex: 0,
    })
  ).validationLoss
  const steps: Cs336A1TrainingStepRecord[] = []

  for (let stepIndex = 0; stepIndex < config.stepCount; stepIndex += 1) {
    const shardResults = await Promise.all(
      Array.from({ length: config.shardCount }, (_, shardIndex) =>
        computeCs336A1ShardGradient({
          config,
          params,
          seedDigestHex,
          shardIndex,
          stepIndex,
        }),
      ),
    )

    params = applyAggregatedSgdStep(
      config,
      params,
      shardResults.map(result => result.gradient),
    )

    const aggregatedStateDigestHex = await parameterStateDigest(
      seedDigestHex,
      stepIndex + 1,
      params,
    )
    const validation = await computeCs336A1ValidationLoss({
      config,
      params,
      seedDigestHex,
      stepIndex: stepIndex + 1,
    })

    steps.push({
      aggregatedStateDigestHex,
      shardResults: shardResults.map(result => ({
        dataUnitCount: result.dataUnitCount,
        digestHex: result.digestHex,
        gradientL2Norm: result.gradientL2Norm,
        shardIndex: result.shardIndex,
        shardLoss: result.shardLoss,
      })),
      stepIndex,
      validationLoss: validation.validationLoss,
    })
  }

  return {
    finalStateDigestHex: steps[steps.length - 1]!.aggregatedStateDigestHex,
    initialStateDigestHex,
    initialValidationLoss,
    steps,
  }
}
