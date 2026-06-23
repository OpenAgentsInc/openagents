/**
 * Pylon worker self-benchmark BEFORE registration (book P1-6,
 * openagents#6089).
 *
 * The book's rule: benchmark a worker against a realistic workload before it
 * makes a serving product claim, and disclose engine/version/quant/GPU. This
 * module measures a node's OWN serving capability and records a deterministic,
 * public-safe self-benchmark receipt. The capability evidence
 * (`serving-capability.ts`) only publishes the serving ref when such a receipt
 * exists and passes parity.
 *
 * Two adapters, one gate:
 *
 *   - The DEFAULT adapter is deterministic and fixture-backed. It replays a
 *     pinned, digest-known prompt through a pure reference scorer and produces
 *     a stable receipt (same input -> byte-identical metrics + parity digest).
 *     It touches NO GPU and NO network, so it runs in CI.
 *
 *   - The REAL-GPU adapter is COMPUTE/OWNER-GATED. It is only reachable when
 *     `PYLON_SERVING_REAL_GPU_BENCH=1` is set in the environment. The sync
 *     selector still refuses unless explicitly configured; the async HTTP
 *     runner can call an owner-provided vLLM/SGLang OpenAI-compatible endpoint
 *     and turn the measured response into a public-safe receipt.
 *
 * NO MONEY MOVES. NO REAL GPU/NETWORK in the default path or in tests.
 */
import { createHash } from "node:crypto"
import {
  PYLON_SERVING_REAL_GPU_GATED_BLOCKER_REF,
  PYLON_SERVING_SELF_BENCH_RECEIPT_PREFIX,
  type QuantizationMode,
  type ServingEngine,
} from "./serving-capability.js"
import { assertPublicProjectionSafe } from "./state.js"

/** Environment flag that gates the real-GPU benchmark adapter. */
export const PYLON_SERVING_REAL_GPU_BENCH_ENV = "PYLON_SERVING_REAL_GPU_BENCH"
export const PYLON_SERVING_REAL_GPU_ENDPOINT_ENV = "PYLON_SERVING_REAL_GPU_ENDPOINT"
export const PYLON_SERVING_REAL_GPU_API_KEY_ENV = "PYLON_SERVING_REAL_GPU_API_KEY"
export const PYLON_SERVING_REAL_GPU_OWNER_APPROVAL_REF_ENV =
  "PYLON_SERVING_REAL_GPU_OWNER_APPROVAL_REF"

/**
 * A pinned, digest-known self-benchmark workload. Deterministic by design: a
 * fixed prompt and a fixed expected-output digest let the fixture adapter
 * produce a stable parity result without a real model. The book's "no parity,
 * no pay" lives on top of this — a benchmark that does not match the expected
 * digest fails parity.
 */
export type ServingBenchmarkWorkload = {
  workloadRef: string
  modelRef: string
  engine: ServingEngine
  quantization: QuantizationMode
  prompt: string
  // Expected canonical output digest for a parity-passing serve of this
  // workload. Disclosing precision/backend matters per the book: a different
  // quantization mode is allowed to produce a different digest, so parity is
  // scoped to (modelRef, engine, quantization).
  expectedOutputDigest: string
  maxNewTokens: number
}

/**
 * The deterministic self-benchmark workload pinned for whole-small-model
 * serving. Fixture-backed: the digest is the canonical hash of the pinned
 * prompt + workload identity, so the fixture adapter can reproduce it exactly.
 */
export const PINNED_SELF_BENCHMARK_WORKLOAD: ServingBenchmarkWorkload = (() => {
  const base = {
    workloadRef: "workload.pylon.serving.self_bench.whole_small.v1",
    modelRef: "model.psionic.qwen35.0_8b.q8_0",
    engine: "llama_cpp" as ServingEngine,
    quantization: "gguf_q8_0" as QuantizationMode,
    prompt: "Return the canonical Pylon serving self-benchmark acknowledgement.",
    maxNewTokens: 64,
  }
  const expectedOutputDigest = canonicalOutputDigest(base)
  return { ...base, expectedOutputDigest }
})()

/** Public-safe self-benchmark receipt. Carries no raw paths or secrets. */
export type ServingSelfBenchmarkReceipt = {
  schema: "openagents.pylon.serving_self_benchmark.v0.6"
  receiptRef: string
  observedAt: string
  workloadRef: string
  modelRef: string
  engine: ServingEngine
  engineVersion: string
  quantization: QuantizationMode
  gpuClass: string
  warmState: "warm" | "cold"
  adapter: "fixture" | "real_gpu"
  metrics: {
    ttftMs: number
    tokensPerSecond: number
    totalTokens: number
    wallClockMs: number
  }
  // The book's "no parity, no pay" gate, recorded here so a verifier can decide
  // before payout. `parityPassed` requires the measured output digest to match
  // the workload's expected digest for (modelRef, engine, quantization).
  parity: {
    expectedOutputDigest: string
    measuredOutputDigest: string
    parityPassed: boolean
  }
  blockerRefs: string[]
}

function canonicalOutputDigest(input: {
  workloadRef: string
  modelRef: string
  engine: ServingEngine
  quantization: QuantizationMode
  prompt: string
}): string {
  // The fixture's "served output" is a pure function of the disclosed
  // (modelRef, engine, quantization, prompt). This makes parity deterministic:
  // change the quantization and the digest changes, which is exactly the book's
  // "FP8 model is a different product" rule expressed mechanically.
  return createHash("sha256")
    .update(
      [input.workloadRef, input.modelRef, input.engine, input.quantization, input.prompt].join(
        "\0",
      ),
    )
    .digest("base64url")
}

function textOutputDigest(text: string): string {
  return createHash("sha256").update(text).digest("base64url")
}

function receiptRefFor(workload: ServingBenchmarkWorkload, observedAt: string): string {
  const stamp = createHash("sha256")
    .update([workload.workloadRef, workload.modelRef, observedAt].join("\0"))
    .digest("base64url")
    .slice(0, 16)
  return `${PYLON_SERVING_SELF_BENCH_RECEIPT_PREFIX}${stamp}`
}

export type ServingBenchmarkAdapterInput = {
  workload: ServingBenchmarkWorkload
  observedAt: string
  gpuClass: string
  engineVersion: string
  env?: NodeJS.ProcessEnv
}

export type ServingBenchmarkAdapter = (
  input: ServingBenchmarkAdapterInput,
) => ServingSelfBenchmarkReceipt

/**
 * Deterministic fixture adapter. Produces a stable, parity-passing receipt
 * with fixed metrics derived from the workload identity. Touches no GPU and no
 * network. Same input -> byte-identical receipt (modulo the supplied
 * observedAt), so tests can assert exact determinism.
 */
export const fixtureServingBenchmarkAdapter: ServingBenchmarkAdapter = (input) => {
  const measuredOutputDigest = canonicalOutputDigest(input.workload)
  const parityPassed = measuredOutputDigest === input.workload.expectedOutputDigest
  // Stable pseudo-metrics derived from the workload digest. Not a real
  // measurement — clearly the fixture path (adapter: "fixture").
  const seed = parseInt(
    createHash("sha256").update(input.workload.workloadRef).digest("hex").slice(0, 8),
    16,
  )
  const tokensPerSecond = 20 + (seed % 80) // 20..99 tok/s, deterministic
  const totalTokens = input.workload.maxNewTokens
  const wallClockMs = Math.round((totalTokens / tokensPerSecond) * 1000)
  const ttftMs = 50 + (seed % 50)

  const receipt: ServingSelfBenchmarkReceipt = {
    schema: "openagents.pylon.serving_self_benchmark.v0.6",
    receiptRef: receiptRefFor(input.workload, input.observedAt),
    observedAt: input.observedAt,
    workloadRef: input.workload.workloadRef,
    modelRef: input.workload.modelRef,
    engine: input.workload.engine,
    engineVersion: input.engineVersion,
    quantization: input.workload.quantization,
    gpuClass: input.gpuClass,
    warmState: "warm",
    adapter: "fixture",
    metrics: {
      ttftMs,
      tokensPerSecond,
      totalTokens,
      wallClockMs,
    },
    parity: {
      expectedOutputDigest: input.workload.expectedOutputDigest,
      measuredOutputDigest,
      parityPassed,
    },
    blockerRefs: parityPassed ? [] : ["blocker.pylon.serving.self_bench_parity_failed"],
  }
  assertPublicProjectionSafe(receipt)
  return receipt
}

function blockedRealGpuReceipt(
  input: ServingBenchmarkAdapterInput,
  blockerRefs: string[],
): ServingSelfBenchmarkReceipt {
  const receipt: ServingSelfBenchmarkReceipt = {
    schema: "openagents.pylon.serving_self_benchmark.v0.6",
    receiptRef: receiptRefFor(input.workload, input.observedAt),
    observedAt: input.observedAt,
    workloadRef: input.workload.workloadRef,
    modelRef: input.workload.modelRef,
    engine: input.workload.engine,
    engineVersion: input.engineVersion,
    quantization: input.workload.quantization,
    gpuClass: input.gpuClass,
    warmState: "cold",
    adapter: "real_gpu",
    metrics: { ttftMs: 0, tokensPerSecond: 0, totalTokens: 0, wallClockMs: 0 },
    parity: {
      expectedOutputDigest: input.workload.expectedOutputDigest,
      measuredOutputDigest: "",
      parityPassed: false,
    },
    blockerRefs,
  }
  assertPublicProjectionSafe(receipt)
  return receipt
}

/**
 * Real-GPU benchmark adapter — COMPUTE/OWNER-GATED. Reachable only when
 * `PYLON_SERVING_REAL_GPU_BENCH=1`. Until that flag is set (and until the real
 * vLLM/SGLang serving path is actually wired by an owner), it refuses with a
 * typed blocker rather than fabricating a measurement. This is the precise
 * seam for the honestly out-of-scope real-serving work.
 */
export const realGpuServingBenchmarkAdapter: ServingBenchmarkAdapter = (input) => {
  const env = input.env ?? process.env
  const enabled = env[PYLON_SERVING_REAL_GPU_BENCH_ENV]?.trim() === "1"
  if (!enabled) {
    return blockedRealGpuReceipt(input, [PYLON_SERVING_REAL_GPU_GATED_BLOCKER_REF])
  }
  return blockedRealGpuReceipt(input, ["blocker.pylon.serving.real_gpu_http_runner_required"])
}

type OpenAiCompatibleCompletionResponse = {
  choices?: Array<{ message?: { content?: unknown }; text?: unknown }>
  usage?: { completion_tokens?: unknown; total_tokens?: unknown }
}

export type RealGpuServingHttpClient = (request: {
  endpoint: string
  apiKey: string | null
  payload: {
    model: string
    messages: Array<{ role: "user"; content: string }>
    max_tokens: number
    temperature: number
  }
}) => Promise<OpenAiCompatibleCompletionResponse>

const defaultRealGpuServingHttpClient: RealGpuServingHttpClient = async (request) => {
  const headers: Record<string, string> = { "content-type": "application/json" }
  if (request.apiKey !== null) {
    headers.authorization = `Bearer ${request.apiKey}`
  }
  const response = await fetch(request.endpoint, {
    method: "POST",
    headers,
    body: JSON.stringify(request.payload),
  })
  if (!response.ok) {
    throw new Error(`real GPU serving endpoint returned HTTP ${response.status}`)
  }
  return (await response.json()) as OpenAiCompatibleCompletionResponse
}

function completionText(response: OpenAiCompatibleCompletionResponse): string {
  const choice = response.choices?.[0]
  const content = choice?.message?.content ?? choice?.text
  return typeof content === "string" ? content : ""
}

function completionTokenCount(
  response: OpenAiCompatibleCompletionResponse,
  fallbackText: string,
): number {
  const completionTokens = response.usage?.completion_tokens ?? response.usage?.total_tokens
  if (typeof completionTokens === "number" && Number.isFinite(completionTokens) && completionTokens > 0) {
    return completionTokens
  }
  return Math.max(1, fallbackText.trim().split(/\s+/).filter(Boolean).length)
}

/**
 * Owner-gated vLLM/SGLang benchmark path. This is the only function in this
 * module that can call a network endpoint, and it requires both the real-GPU
 * flag and a public approval ref. Endpoint/API-key values are never copied into
 * the receipt.
 */
export async function runRealGpuServingSelfBenchmark(input: {
  observedAt: string
  gpuClass: string
  engineVersion: string
  workload: ServingBenchmarkWorkload
  env?: NodeJS.ProcessEnv
  httpClient?: RealGpuServingHttpClient
  now?: () => number
}): Promise<ServingSelfBenchmarkReceipt> {
  const env = input.env ?? process.env
  const adapterInput = {
    workload: input.workload,
    observedAt: input.observedAt,
    gpuClass: input.gpuClass,
    engineVersion: input.engineVersion,
    env,
  }
  if (env[PYLON_SERVING_REAL_GPU_BENCH_ENV]?.trim() !== "1") {
    return blockedRealGpuReceipt(adapterInput, [PYLON_SERVING_REAL_GPU_GATED_BLOCKER_REF])
  }
  const approvalRef = env[PYLON_SERVING_REAL_GPU_OWNER_APPROVAL_REF_ENV]?.trim() ?? ""
  if (approvalRef.length === 0) {
    return blockedRealGpuReceipt(adapterInput, [
      "blocker.pylon.serving.real_gpu_owner_approval_ref_missing",
    ])
  }
  const endpoint = env[PYLON_SERVING_REAL_GPU_ENDPOINT_ENV]?.trim() ?? ""
  if (endpoint.length === 0) {
    return blockedRealGpuReceipt(adapterInput, [
      "blocker.pylon.serving.real_gpu_endpoint_missing",
    ])
  }
  if (input.workload.engine !== "vllm" && input.workload.engine !== "sglang") {
    return blockedRealGpuReceipt(adapterInput, [
      "blocker.pylon.serving.real_gpu_engine_not_http_proven",
    ])
  }

  const startedAt = input.now?.() ?? Date.now()
  try {
    const response = await (input.httpClient ?? defaultRealGpuServingHttpClient)({
      endpoint,
      apiKey: env[PYLON_SERVING_REAL_GPU_API_KEY_ENV]?.trim() || null,
      payload: {
        model: input.workload.modelRef,
        messages: [{ role: "user", content: input.workload.prompt }],
        max_tokens: input.workload.maxNewTokens,
        temperature: 0,
      },
    })
    const endedAt = input.now?.() ?? Date.now()
    const output = completionText(response)
    const measuredOutputDigest = textOutputDigest(output)
    const totalTokens = completionTokenCount(response, output)
    const wallClockMs = Math.max(1, Math.round(endedAt - startedAt))
    const parityPassed = measuredOutputDigest === input.workload.expectedOutputDigest
    const receipt: ServingSelfBenchmarkReceipt = {
      schema: "openagents.pylon.serving_self_benchmark.v0.6",
      receiptRef: receiptRefFor(input.workload, input.observedAt),
      observedAt: input.observedAt,
      workloadRef: input.workload.workloadRef,
      modelRef: input.workload.modelRef,
      engine: input.workload.engine,
      engineVersion: input.engineVersion,
      quantization: input.workload.quantization,
      gpuClass: input.gpuClass,
      warmState: "warm",
      adapter: "real_gpu",
      metrics: {
        ttftMs: wallClockMs,
        tokensPerSecond: Number((totalTokens / (wallClockMs / 1000)).toFixed(6)),
        totalTokens,
        wallClockMs,
      },
      parity: {
        expectedOutputDigest: input.workload.expectedOutputDigest,
        measuredOutputDigest,
        parityPassed,
      },
      blockerRefs: parityPassed ? [] : ["blocker.pylon.serving.self_bench_parity_failed"],
    }
    assertPublicProjectionSafe(receipt)
    return receipt
  } catch {
    return blockedRealGpuReceipt(adapterInput, ["blocker.pylon.serving.real_gpu_http_failed"])
  }
}

/**
 * Select the benchmark adapter. Defaults to the deterministic fixture adapter.
 * The real-GPU adapter is only selected when the gate flag is set; otherwise we
 * stay on the fixture path so registration never depends on owner-gated
 * compute.
 */
export function selectServingBenchmarkAdapter(
  env: NodeJS.ProcessEnv = process.env,
): { adapter: ServingBenchmarkAdapter; realGpu: boolean } {
  if (env[PYLON_SERVING_REAL_GPU_BENCH_ENV]?.trim() === "1") {
    return { adapter: realGpuServingBenchmarkAdapter, realGpu: true }
  }
  return { adapter: fixtureServingBenchmarkAdapter, realGpu: false }
}

/**
 * Run the self-benchmark and return the receipt. Default path is the
 * deterministic fixture adapter. A real-GPU run is owner-gated and currently
 * refuses with a typed blocker (see `realGpuServingBenchmarkAdapter`).
 */
export function runServingSelfBenchmark(input: {
  observedAt: string
  gpuClass: string
  engineVersion: string
  workload?: ServingBenchmarkWorkload
  adapter?: ServingBenchmarkAdapter
  env?: NodeJS.ProcessEnv
}): ServingSelfBenchmarkReceipt {
  const env = input.env ?? process.env
  const workload = input.workload ?? PINNED_SELF_BENCHMARK_WORKLOAD
  const adapter = input.adapter ?? selectServingBenchmarkAdapter(env).adapter
  return adapter({
    workload,
    observedAt: input.observedAt,
    gpuClass: input.gpuClass,
    engineVersion: input.engineVersion,
    env,
  })
}

export { canonicalOutputDigest, textOutputDigest }
