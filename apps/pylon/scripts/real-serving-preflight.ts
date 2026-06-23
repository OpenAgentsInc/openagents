#!/usr/bin/env bun
/**
 * Owner-gated live Pylon serving evidence runner (#6089).
 *
 * Calls an owner-approved vLLM/SGLang OpenAI-compatible endpoint, emits
 * public-safe self-benchmark/capability/serve/replay/preflight evidence, and
 * never stores endpoint URLs, API keys, prompts, or raw outputs in the evidence.
 */
import { createHash } from "node:crypto"
import { execFileSync } from "node:child_process"
import {
  buildServingCapabilityEvidence,
  preflightRealPylonServing,
  type BandwidthClass,
  type InterconnectPosture,
  type QuantizationMode,
  type ServingEngine,
} from "../src/serving-capability.js"
import {
  PYLON_SERVING_REAL_GPU_API_KEY_ENV,
  PYLON_SERVING_REAL_GPU_BENCH_ENV,
  PYLON_SERVING_REAL_GPU_ENDPOINT_ENV,
  PYLON_SERVING_REAL_GPU_OWNER_APPROVAL_REF_ENV,
  runRealGpuServingSelfBenchmark,
  textOutputDigest,
} from "../src/serving-benchmark.js"
import {
  buildServingReceipt,
  computeServingVerification,
  runServingReplayChallenge,
} from "../src/serving-receipt.js"
import { assertPublicProjectionSafe } from "../src/state.js"

type CompletionResponse = {
  model?: unknown
  choices?: Array<{ message?: { content?: unknown }; text?: unknown }>
  usage?: {
    prompt_tokens?: unknown
    completion_tokens?: unknown
    total_tokens?: unknown
  }
}

type TimedCompletion = {
  output: string
  outputDigest: string
  wallClockMs: number
  promptTokens: number
  completionTokens: number
  model: string
}

function envValue(name: string): string | null {
  const value = process.env[name]?.trim()
  return value && value.length > 0 ? value : null
}

function requireEnv(name: string): string {
  const value = envValue(name)
  if (value === null) {
    throw new Error(`${name} is required`)
  }
  return value
}

function boolEnv(name: string): boolean {
  const value = envValue(name)?.toLowerCase()
  return value === "1" || value === "true" || value === "yes"
}

function numberEnv(name: string, fallback: number): number {
  const value = envValue(name)
  if (value === null) return fallback
  const parsed = Number(value)
  if (!Number.isFinite(parsed) || parsed < 0) {
    throw new Error(`${name} must be a non-negative number`)
  }
  return parsed
}

function sha256Base64Url(text: string): string {
  return createHash("sha256").update(text).digest("base64url")
}

function completionText(response: CompletionResponse): string {
  const choice = response.choices?.[0]
  const content = choice?.message?.content ?? choice?.text
  return typeof content === "string" ? content : ""
}

function tokenCount(value: unknown, fallbackText: string): number {
  if (typeof value === "number" && Number.isFinite(value) && value >= 0) {
    return value
  }
  return Math.max(1, fallbackText.trim().split(/\s+/).filter(Boolean).length)
}

async function callCompletion(input: {
  endpoint: string
  apiKey: string | null
  modelRef: string
  prompt: string
  maxNewTokens: number
}): Promise<TimedCompletion> {
  const headers: Record<string, string> = { "content-type": "application/json" }
  if (input.apiKey !== null) {
    headers.authorization = `Bearer ${input.apiKey}`
  }
  const startedAt = Date.now()
  const response = await fetch(input.endpoint, {
    method: "POST",
    headers,
    body: JSON.stringify({
      model: input.modelRef,
      messages: [{ role: "user", content: input.prompt }],
      max_tokens: input.maxNewTokens,
      temperature: 0,
    }),
  })
  const endedAt = Date.now()
  if (!response.ok) {
    throw new Error(`serving endpoint returned HTTP ${response.status}`)
  }
  const json = (await response.json()) as CompletionResponse
  const output = completionText(json)
  const completionTokens = tokenCount(
    json.usage?.completion_tokens ?? json.usage?.total_tokens,
    output,
  )
  const promptTokens =
    typeof json.usage?.prompt_tokens === "number" && Number.isFinite(json.usage.prompt_tokens)
      ? json.usage.prompt_tokens
      : 0
  return {
    output,
    outputDigest: textOutputDigest(output),
    wallClockMs: Math.max(1, endedAt - startedAt),
    promptTokens,
    completionTokens,
    model: typeof json.model === "string" ? json.model : input.modelRef,
  }
}

function nvidiaMemoryGb(): { total: number; free: number } | null {
  try {
    const output = execFileSync(
      "nvidia-smi",
      ["--query-gpu=memory.total,memory.free", "--format=csv,noheader,nounits"],
      { encoding: "utf8", stdio: ["ignore", "pipe", "ignore"] },
    )
    const firstLine = output.trim().split("\n")[0]
    const [totalMiB, freeMiB] = firstLine?.split(",").map((part) => Number(part.trim())) ?? []
    if (
      typeof totalMiB === "number" &&
      typeof freeMiB === "number" &&
      Number.isFinite(totalMiB) &&
      Number.isFinite(freeMiB)
    ) {
      return {
        total: Number((totalMiB / 1024).toFixed(3)),
        free: Number((freeMiB / 1024).toFixed(3)),
      }
    }
  } catch {
    return null
  }
  return null
}

const observedAt = new Date().toISOString()
const endpoint = requireEnv(PYLON_SERVING_REAL_GPU_ENDPOINT_ENV)
const ownerApprovalRef = requireEnv(PYLON_SERVING_REAL_GPU_OWNER_APPROVAL_REF_ENV)
const admittedPylonRef = envValue("PYLON_SERVING_ADMITTED_PYLON_REF")
const apiKey = envValue(PYLON_SERVING_REAL_GPU_API_KEY_ENV)
const modelRef = envValue("PYLON_SERVING_MODEL_REF") ?? "model.psionic.qwen35.0_8b.q8_0"
const engine = (envValue("PYLON_SERVING_ENGINE") ?? "vllm") as ServingEngine
const engineVersion = envValue("PYLON_SERVING_ENGINE_VERSION") ?? `${engine}@unknown`
const quantization = (envValue("PYLON_SERVING_QUANTIZATION") ?? "bf16") as QuantizationMode
const gpuClass = envValue("PYLON_SERVING_GPU_CLASS") ?? "accelerator.nvidia_l4"
const prompt =
  envValue("PYLON_SERVING_SELF_BENCH_PROMPT") ?? "Respond with exactly OK and nothing else."
const expectedOutput = envValue("PYLON_SERVING_SELF_BENCH_EXPECTED_OUTPUT") ?? "OK"
const maxNewTokens = numberEnv("PYLON_SERVING_MAX_NEW_TOKENS", 1)
const expectedOutputDigest = sha256Base64Url(expectedOutput)
const gpuMemory = nvidiaMemoryGb()
const totalGpuMemoryGb = numberEnv("PYLON_SERVING_TOTAL_GPU_MEMORY_GB", gpuMemory?.total ?? 0)
const usableGpuMemoryGb = numberEnv("PYLON_SERVING_USABLE_GPU_MEMORY_GB", gpuMemory?.free ?? 0)
const fabricTransportReady = boolEnv("PYLON_SERVING_FABRIC_TRANSPORT_READY")
const gatewayRouteReady = boolEnv("PYLON_SERVING_GATEWAY_ROUTE_READY")
const requireGatewayRouteReady = boolEnv("PYLON_SERVING_REQUIRE_GATEWAY_ROUTE_READY")

if (process.env[PYLON_SERVING_REAL_GPU_BENCH_ENV]?.trim() !== "1") {
  throw new Error(`${PYLON_SERVING_REAL_GPU_BENCH_ENV}=1 is required`)
}
if (engine !== "vllm" && engine !== "sglang") {
  throw new Error("PYLON_SERVING_ENGINE must be vllm or sglang")
}

const selfBenchmarkReceipt = await runRealGpuServingSelfBenchmark({
  observedAt,
  gpuClass,
  engineVersion,
  workload: {
    workloadRef: "workload.pylon.serving.self_bench.known_answer.v1",
    modelRef,
    engine,
    quantization,
    prompt,
    expectedOutputDigest,
    maxNewTokens,
  },
})

const serve = await callCompletion({ endpoint, apiKey, modelRef, prompt, maxNewTokens })
const baseReceipt = buildServingReceipt({
  servedAt: observedAt,
  modelRef,
  engine,
  engineVersion,
  quantization,
  gpuClass,
  warmState: "warm",
  residencyAtServe: "warm",
  promptDigest: sha256Base64Url(prompt),
  outputDigest: serve.outputDigest,
  maxNewTokens,
  temperature: 0,
  samplingSeed: null,
  metrics: {
    ttftMs: serve.wallClockMs,
    tokensPerSecond: Number((serve.completionTokens / (serve.wallClockMs / 1000)).toFixed(6)),
    promptTokens: serve.promptTokens,
    completionTokens: serve.completionTokens,
    wallClockMs: serve.wallClockMs,
  },
  verification: computeServingVerification({
    parityPassed: serve.outputDigest === expectedOutputDigest,
    canary: {
      canaryRef: "canary.pylon.serving.known_answer.ok.v1",
      passed: serve.outputDigest === expectedOutputDigest,
    },
  }),
})

const replay = await callCompletion({ endpoint, apiKey, modelRef, prompt, maxNewTokens })
const replayChallenge = runServingReplayChallenge({
  challengedAt: new Date().toISOString(),
  receipt: baseReceipt,
  replayedOutputDigest: replay.outputDigest,
  replayEngine: engine,
  replayEngineVersion: engineVersion,
  replayQuantization: quantization,
})

const servingReceipt = buildServingReceipt({
  servedAt: observedAt,
  modelRef,
  engine,
  engineVersion,
  quantization,
  gpuClass,
  warmState: "warm",
  residencyAtServe: "warm",
  promptDigest: sha256Base64Url(prompt),
  outputDigest: serve.outputDigest,
  maxNewTokens,
  temperature: 0,
  samplingSeed: null,
  metrics: baseReceipt.metrics,
  verification: computeServingVerification({
    parityPassed: serve.outputDigest === expectedOutputDigest,
    canary: {
      canaryRef: "canary.pylon.serving.known_answer.ok.v1",
      passed: serve.outputDigest === expectedOutputDigest,
    },
    replay: replayChallenge,
  }),
})

const capability = buildServingCapabilityEvidence({
  observedAt,
  gpuClass,
  usableGpuMemoryGb,
  totalGpuMemoryGb,
  bandwidthClass: (envValue("PYLON_SERVING_BANDWIDTH_CLASS") ?? "gddr") as BandwidthClass,
  interconnect: (envValue("PYLON_SERVING_INTERCONNECT") ?? "single_gpu") as InterconnectPosture,
  engines: [engine],
  residency: [
    {
      modelRef,
      engine,
      quantization,
      residency: "warm",
      coldStart: {
        gpuProcurementMs: 0,
        imageLoadMs: 0,
        weightLoadMs: 0,
        engineStartupMs: 0,
      },
    },
  ],
  selfBenchmarkReceiptRef: selfBenchmarkReceipt.receiptRef,
  realGpuAdapter: true,
})

const preflight = preflightRealPylonServing({
  observedAt,
  capability,
  selfBenchmarkReceipt,
  servingReceipt,
  ownerConfirmed: true,
  ownerApprovalRef,
  admittedPylonRef,
  fabricTransportReady,
  gatewayRouteReady,
  requireGatewayRouteReady,
})

const evidence = {
  schema: "openagents.pylon.live_serving_preflight_run.v0.1",
  observedAt,
  endpointRef: "endpoint.redacted.openai_compatible.local_or_owner_supplied",
  ownerApprovalRef,
  admittedPylonRef,
  modelRef,
  engine,
  engineVersion,
  quantization,
  gpuClass,
  expectedOutputDigest,
  measuredOutputDigest: serve.outputDigest,
  replayedOutputDigest: replay.outputDigest,
  selfBenchmarkReceipt,
  capability,
  servingReceipt,
  replayChallenge,
  preflight,
}

assertPublicProjectionSafe(evidence)
console.log(JSON.stringify(evidence, null, 2))
