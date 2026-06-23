/**
 * Pylon proven-engine serving capability evidence (book P1-6,
 * openagents#6089).
 *
 * The inference-engineering book makes one point that this module exists to
 * honor: GPUs are not fungible, and a "served model id" is not a product
 * unless the engine, version, quantization mode, GPU class, and warm/cold
 * posture are all disclosed. A node that can run vLLM on an H100 with FP8
 * weights resident is a categorically different supplier than a node that
 * claims the same model id on a cold Lovelace card with an unknown engine.
 *
 * So a Pylon that wants to take whole-small-model serving work must publish
 * PRECISE, TYPED capability evidence (not just "GPU online"):
 *
 *   - usable GPU memory (the headroom after the OS/driver/other tenants),
 *   - a bandwidth class (decode is bandwidth-bound; the book's roofline),
 *   - an interconnect posture (single GPU, NVLink/NVSwitch node, InfiniBand
 *     multi-node, or none),
 *   - which proven engines it actually supports (vLLM / SGLang / llama.cpp /
 *     native), avoiding a bespoke engine per the book,
 *   - model residency (which model ids are warm/resident vs paged/cold),
 *   - a cold-start posture (the book's four cold-start factors: GPU
 *     procurement, image load, weight load, engine startup/compile).
 *
 * NO MONEY MOVES HERE and NO REAL GPU IS TOUCHED. This module owns the
 * EVIDENCE/SCHEMA/SELF-BENCHMARK/RECEIPT shape. The self-benchmark is
 * deterministic and fixture-backed by default; the real-GPU benchmark adapter
 * is explicitly FLAG-GATED off (see `serving-benchmark.ts`). Real
 * vLLM/SGLang GPU serving is compute/owner-gated and lives behind that flag.
 *
 * Ownership split (matches the workspace contract): Psionic owns execution
 * evidence; product surfaces own pricing/routing/payout/marketplace
 * authority. This module only produces public-safe capability evidence and
 * receipt fields that those surfaces can canary/replay BEFORE paying — it
 * never decides payment.
 */
import { assertPublicProjectionSafe } from "./state.js"
import type { ServingSelfBenchmarkReceipt } from "./serving-benchmark.js"
import type { PylonServingReceipt } from "./serving-receipt.js"

// Capability refs declared into the heartbeat/register capabilityRefs.
// A serving claim without a self-benchmark receipt ref never publishes (the
// same no-overclaim posture as the Tassadar executor capability).
export const PYLON_SERVING_CAPABILITY_REF = "pylon.capability.serving.whole_small_model.v0.6"
export const PYLON_SERVING_SELF_BENCH_RECEIPT_PREFIX = "receipt.pylon.serving.self_bench."
export const PYLON_SERVING_SELF_BENCH_FAILED_BLOCKER_REF =
  "blocker.pylon.serving.self_benchmark_failed"
export const PYLON_SERVING_NO_PARITY_BLOCKER_REF = "blocker.pylon.serving.no_parity"
export const PYLON_SERVING_REAL_GPU_GATED_BLOCKER_REF =
  "blocker.pylon.serving.real_gpu_adapter_gated"
export const PYLON_SERVING_PREFLIGHT_SCHEMA = "openagents.pylon.serving_preflight.v0.1"

/**
 * Proven inference engines per the book (ch.4): vLLM for broad support and
 * fast experiments; SGLang for Kimi/Qwen/DeepSeek/MoE or code-heavy lanes;
 * TensorRT-LLM only when the model/GPU path is stable enough to justify the
 * config work. `llama_cpp` covers quantized GGUF whole-small serving on
 * commodity/Apple hardware; `native` is an owned Psionic/Tassadar executor
 * path. No bespoke engine is invented here.
 */
export type ServingEngine = "vllm" | "sglang" | "tensorrt_llm" | "llama_cpp" | "native"

/**
 * Memory-bandwidth class for the accelerator. The book's roofline shows LLM
 * decode is bandwidth-bound, so bandwidth class is a first-class capability
 * fact, not a footnote. Classes are coarse public-safe buckets, never raw
 * device strings.
 */
export type BandwidthClass =
  | "hbm_high" // datacenter HBM (e.g. H100/B200-class), TB/s range
  | "hbm_mid" // older/fractional datacenter HBM
  | "gddr" // consumer/workstation GDDR
  | "unified" // Apple-silicon unified memory
  | "unknown"

/**
 * Interconnect posture (book ch.3): whether and how multiple GPUs are wired.
 * Whole-small-model serving needs only `single` or `none`; the richer postures
 * are declared so the marketplace can later place tensor/pipeline-parallel work
 * without re-probing.
 */
export type InterconnectPosture =
  | "none" // CPU/unified-memory only, no discrete GPU interconnect
  | "single_gpu" // one discrete GPU, no GPU-to-GPU interconnect
  | "nvlink_node" // NVLink/NVSwitch within a single node
  | "infiniband_multi_node" // InfiniBand (or equiv) node-to-node
  | "vendor_other" // a non-standard provider interconnect
  | "unknown"

/**
 * Quantization mode disclosed in capability + receipts. The book is explicit
 * that an FP8/MXFP8 model is NOT the same product as an unqualified model id,
 * so a served model alias must carry its precision/backend. `unquantized`
 * means full-precision weights as published.
 */
export type QuantizationMode =
  | "unquantized"
  | "fp8"
  | "mxfp8"
  | "int8"
  | "int4"
  | "gguf_q8_0"
  | "gguf_q4_k_m"
  | "awq"
  | "gptq"
  | "unknown"

/**
 * Warm/cold residency state for a model on this node. `warm` = weights
 * resident and engine started (no cold-start cost on next request);
 * `paged` = weights on local disk but not loaded (weight-load cost only);
 * `cold` = not present (full cold-start: image/weight load + engine startup).
 */
export type ResidencyState = "warm" | "paged" | "cold" | "unknown"

/** A single model this node can serve, with its disclosed precision/backend. */
export type ServingModelResidency = {
  modelRef: string // public-safe `model.*` ref, never a raw path
  engine: ServingEngine
  quantization: QuantizationMode
  residency: ResidencyState
  // The book's cold-start factors, estimated (ms). These are POSTURE, not a
  // measured guarantee; the self-benchmark records the actual observed value.
  coldStart: {
    gpuProcurementMs: number // warm pool -> ~0; cold node -> provider-bound
    imageLoadMs: number
    weightLoadMs: number
    engineStartupMs: number // includes any compile (torch.compile etc.)
  }
}

/**
 * The precise, typed serving capability evidence a Pylon publishes. Replaces
 * "GPU online" with the book's actual hardware/software posture.
 */
export type PylonServingCapabilityEvidence = {
  schema: "openagents.pylon.serving_capability.v0.6"
  observedAt: string
  capabilityRef: typeof PYLON_SERVING_CAPABILITY_REF
  // Engines this node has a usable runtime for (proven engines only).
  engines: ServingEngine[]
  hardware: {
    gpuClass: string // public-safe class ref, e.g. `gpu.class.hopper_h100`
    // Usable GPU memory in GB: total VRAM MINUS headroom reserved by OS/driver
    // and other tenants. The book warns any component can bottleneck; usable
    // memory is what actually constrains model+KV-cache residency.
    usableGpuMemoryGb: number
    totalGpuMemoryGb: number
    bandwidthClass: BandwidthClass
    interconnect: InterconnectPosture
  }
  residency: ServingModelResidency[]
  // True only when the self-benchmark below actually ran and passed parity.
  selfBenchmarked: boolean
  selfBenchmarkReceiptRef: string | null
  // True when the evidence came from the real-GPU adapter (compute/owner-gated)
  // rather than the deterministic fixture path. Always `false` in tests/CI.
  realGpuAdapter: boolean
  blockerRefs: string[]
}

export type PylonRealServingPreflightBlockerRef =
  | "blocker.pylon.serving_preflight.owner_confirmation_missing"
  | "blocker.pylon.serving_preflight.owner_approval_ref_missing"
  | "blocker.pylon.serving_preflight.pylon_admission_missing"
  | "blocker.pylon.serving_preflight.fabric_transport_missing"
  | "blocker.pylon.serving_preflight.gateway_route_not_ready"
  | "blocker.pylon.serving_preflight.capability_blocked"
  | "blocker.pylon.serving_preflight.real_gpu_capability_missing"
  | "blocker.pylon.serving_preflight.proven_engine_missing"
  | "blocker.pylon.serving_preflight.warm_residency_missing"
  | "blocker.pylon.serving_preflight.self_benchmark_receipt_missing"
  | "blocker.pylon.serving_preflight.self_benchmark_receipt_mismatch"
  | "blocker.pylon.serving_preflight.self_benchmark_not_real_gpu"
  | "blocker.pylon.serving_preflight.self_benchmark_blocked"
  | "blocker.pylon.serving_preflight.self_benchmark_parity_failed"
  | "blocker.pylon.serving_preflight.serving_receipt_missing"
  | "blocker.pylon.serving_preflight.serving_receipt_blocked"
  | "blocker.pylon.serving_preflight.serving_receipt_model_not_resident"
  | "blocker.pylon.serving_preflight.serving_receipt_engine_not_allowed"
  | "blocker.pylon.serving_preflight.serving_receipt_no_parity"
  | "blocker.pylon.serving_preflight.serving_receipt_canary_missing"
  | "blocker.pylon.serving_preflight.serving_receipt_replay_missing"
  | "blocker.pylon.serving_preflight.serving_receipt_unverified"
  | "blocker.pylon.serving_preflight.serving_receipt_not_payout_eligible"

export type PylonRealServingReadinessPreflight = {
  schema: typeof PYLON_SERVING_PREFLIGHT_SCHEMA
  observedAt: string
  canArmRealServing: boolean
  paidRoutingEligible: boolean
  ownerApprovalRef: string | null
  admittedPylonRef: string | null
  fabricTransportReady: boolean
  gatewayRouteReady: boolean
  realGpuAdapterReady: boolean
  allowedEngines: ServingEngine[]
  residentModelRefs: string[]
  evidenceRefs: string[]
  blockerRefs: PylonRealServingPreflightBlockerRef[]
}

const COLD_START_ZERO = {
  gpuProcurementMs: 0,
  imageLoadMs: 0,
  weightLoadMs: 0,
  engineStartupMs: 0,
} as const

const DEFAULT_REAL_SERVING_ENGINES: ServingEngine[] = ["vllm", "sglang"]

const normalizedNonBlank = (value: string | null | undefined): string | null => {
  if (value === null || value === undefined) return null
  const trimmed = value.trim()
  return trimmed === "" ? null : trimmed
}

/**
 * Total cold-start estimate for a residency entry (sum of the book's four
 * factors). A `warm` model should sum to ~0; a `cold` model carries the full
 * estimate. Surfaced so the marketplace can prefer warm supply for
 * latency-sensitive routes without re-deriving it.
 */
export function estimatedColdStartMs(residency: ServingModelResidency): number {
  const c = residency.coldStart
  return c.gpuProcurementMs + c.imageLoadMs + c.weightLoadMs + c.engineStartupMs
}

/**
 * Build deterministic, public-safe serving capability evidence from a typed
 * input. This is the FIXTURE/posture path: it does not touch a GPU. The real
 * adapter (flag-gated) produces the same shape with `realGpuAdapter: true`.
 *
 * The evidence is honest by construction: `selfBenchmarked` is only true when
 * a self-benchmark receipt ref is supplied, and a capability with no parity-
 * proven benchmark carries a blocker rather than a silent claim.
 */
export function buildServingCapabilityEvidence(input: {
  observedAt: string
  gpuClass: string
  usableGpuMemoryGb: number
  totalGpuMemoryGb: number
  bandwidthClass: BandwidthClass
  interconnect: InterconnectPosture
  engines: ServingEngine[]
  residency: ServingModelResidency[]
  selfBenchmarkReceiptRef?: string | null
  realGpuAdapter?: boolean
}): PylonServingCapabilityEvidence {
  const blockerRefs = new Set<string>()
  const receiptRef = input.selfBenchmarkReceiptRef ?? null
  const selfBenchmarked =
    receiptRef !== null && receiptRef.startsWith(PYLON_SERVING_SELF_BENCH_RECEIPT_PREFIX)

  if (!selfBenchmarked) {
    blockerRefs.add(PYLON_SERVING_SELF_BENCH_FAILED_BLOCKER_REF)
  }
  if (input.engines.length === 0) {
    blockerRefs.add("blocker.pylon.serving.no_proven_engine")
  }
  if (input.usableGpuMemoryGb > input.totalGpuMemoryGb) {
    blockerRefs.add("blocker.pylon.serving.usable_memory_exceeds_total")
  }
  if (input.usableGpuMemoryGb <= 0) {
    blockerRefs.add("blocker.pylon.serving.no_usable_gpu_memory")
  }
  // A residency entry's engine must be one the node actually supports.
  for (const entry of input.residency) {
    if (!input.engines.includes(entry.engine)) {
      blockerRefs.add("blocker.pylon.serving.residency_engine_unsupported")
    }
  }

  const evidence: PylonServingCapabilityEvidence = {
    schema: "openagents.pylon.serving_capability.v0.6",
    observedAt: input.observedAt,
    capabilityRef: PYLON_SERVING_CAPABILITY_REF,
    engines: [...new Set(input.engines)],
    hardware: {
      gpuClass: input.gpuClass,
      usableGpuMemoryGb: input.usableGpuMemoryGb,
      totalGpuMemoryGb: input.totalGpuMemoryGb,
      bandwidthClass: input.bandwidthClass,
      interconnect: input.interconnect,
    },
    residency: input.residency.map((entry) => ({
      ...entry,
      coldStart: { ...entry.coldStart },
    })),
    selfBenchmarked,
    selfBenchmarkReceiptRef: receiptRef,
    realGpuAdapter: input.realGpuAdapter ?? false,
    blockerRefs: [...blockerRefs],
  }
  assertPublicProjectionSafe(evidence)
  return evidence
}

/**
 * Read-only preflight for arming real whole-small-model Pylon serving.
 *
 * This function is intentionally stricter than publishable fixture capability
 * evidence: #6089 is only clear when an owner-approved admitted Pylon has real
 * GPU evidence, a proven engine (vLLM/SGLang by default), warm model residency,
 * a real-GPU self-benchmark receipt, a parity/canary/replay-eligible serving
 * receipt, and a ready fabric transport. It touches no GPU, network, wallet, or
 * gateway route; it only classifies the public-safe evidence already supplied
 * by those systems.
 */
export function preflightRealPylonServing(input: {
  observedAt: string
  capability: PylonServingCapabilityEvidence
  selfBenchmarkReceipt?: ServingSelfBenchmarkReceipt | null
  servingReceipt?: PylonServingReceipt | null
  ownerConfirmed: boolean
  ownerApprovalRef?: string | null
  admittedPylonRef?: string | null
  fabricTransportReady: boolean
  gatewayRouteReady?: boolean
  requireGatewayRouteReady?: boolean
  allowedEngines?: ServingEngine[]
}): PylonRealServingReadinessPreflight {
  const ownerApprovalRef = normalizedNonBlank(input.ownerApprovalRef)
  const admittedPylonRef = normalizedNonBlank(input.admittedPylonRef)
  const allowedEngines = [...new Set(input.allowedEngines ?? DEFAULT_REAL_SERVING_ENGINES)]
  const allowedEngineSet = new Set<ServingEngine>(allowedEngines)
  const selfBenchmarkReceipt = input.selfBenchmarkReceipt ?? null
  const servingReceipt = input.servingReceipt ?? null
  const blockerRefs = new Set<PylonRealServingPreflightBlockerRef>()

  if (!input.ownerConfirmed) {
    blockerRefs.add("blocker.pylon.serving_preflight.owner_confirmation_missing")
  }
  if (ownerApprovalRef === null) {
    blockerRefs.add("blocker.pylon.serving_preflight.owner_approval_ref_missing")
  }
  if (admittedPylonRef === null) {
    blockerRefs.add("blocker.pylon.serving_preflight.pylon_admission_missing")
  }
  if (!input.fabricTransportReady) {
    blockerRefs.add("blocker.pylon.serving_preflight.fabric_transport_missing")
  }
  const gatewayRouteReady = input.gatewayRouteReady ?? false
  if ((input.requireGatewayRouteReady ?? false) && !gatewayRouteReady) {
    blockerRefs.add("blocker.pylon.serving_preflight.gateway_route_not_ready")
  }
  if (input.capability.blockerRefs.length > 0) {
    blockerRefs.add("blocker.pylon.serving_preflight.capability_blocked")
  }
  if (!input.capability.realGpuAdapter) {
    blockerRefs.add("blocker.pylon.serving_preflight.real_gpu_capability_missing")
  }
  if (!input.capability.engines.some((engine) => allowedEngineSet.has(engine))) {
    blockerRefs.add("blocker.pylon.serving_preflight.proven_engine_missing")
  }

  const realWarmResidency = input.capability.residency.filter(
    (entry) => entry.residency === "warm" && allowedEngineSet.has(entry.engine),
  )
  if (realWarmResidency.length === 0) {
    blockerRefs.add("blocker.pylon.serving_preflight.warm_residency_missing")
  }

  if (selfBenchmarkReceipt === null) {
    blockerRefs.add("blocker.pylon.serving_preflight.self_benchmark_receipt_missing")
  } else {
    if (selfBenchmarkReceipt.receiptRef !== input.capability.selfBenchmarkReceiptRef) {
      blockerRefs.add("blocker.pylon.serving_preflight.self_benchmark_receipt_mismatch")
    }
    if (selfBenchmarkReceipt.adapter !== "real_gpu") {
      blockerRefs.add("blocker.pylon.serving_preflight.self_benchmark_not_real_gpu")
    }
    if (selfBenchmarkReceipt.blockerRefs.length > 0) {
      blockerRefs.add("blocker.pylon.serving_preflight.self_benchmark_blocked")
    }
    if (!selfBenchmarkReceipt.parity.parityPassed) {
      blockerRefs.add("blocker.pylon.serving_preflight.self_benchmark_parity_failed")
    }
  }

  if (servingReceipt === null) {
    blockerRefs.add("blocker.pylon.serving_preflight.serving_receipt_missing")
  } else {
    if (servingReceipt.blockerRefs.length > 0) {
      blockerRefs.add("blocker.pylon.serving_preflight.serving_receipt_blocked")
    }
    const servesResidentModel = realWarmResidency.some(
      (entry) =>
        entry.modelRef === servingReceipt.modelRef &&
        entry.engine === servingReceipt.engine &&
        entry.quantization === servingReceipt.quantization,
    )
    if (!servesResidentModel) {
      blockerRefs.add("blocker.pylon.serving_preflight.serving_receipt_model_not_resident")
    }
    if (!allowedEngineSet.has(servingReceipt.engine)) {
      blockerRefs.add("blocker.pylon.serving_preflight.serving_receipt_engine_not_allowed")
    }
    if (!servingReceipt.verification.parityPassed) {
      blockerRefs.add("blocker.pylon.serving_preflight.serving_receipt_no_parity")
    }
    if (servingReceipt.verification.canary === null) {
      blockerRefs.add("blocker.pylon.serving_preflight.serving_receipt_canary_missing")
    }
    if (servingReceipt.verification.replay === null) {
      blockerRefs.add("blocker.pylon.serving_preflight.serving_receipt_replay_missing")
    }
    if (!servingReceipt.verification.verified) {
      blockerRefs.add("blocker.pylon.serving_preflight.serving_receipt_unverified")
    }
    if (!servingReceipt.verification.payoutEligible) {
      blockerRefs.add("blocker.pylon.serving_preflight.serving_receipt_not_payout_eligible")
    }
  }

  const evidenceRefs = new Set<string>(publishableServingCapabilityRefs(input.capability))
  if (selfBenchmarkReceipt !== null) {
    evidenceRefs.add(selfBenchmarkReceipt.receiptRef)
  }
  if (servingReceipt !== null) {
    evidenceRefs.add(servingReceipt.receiptRef)
    const replay = servingReceipt.verification.replay
    if (replay !== null) {
      evidenceRefs.add(replay.challengeRef)
    }
    const canary = servingReceipt.verification.canary
    if (canary !== null) {
      evidenceRefs.add(canary.canaryRef)
    }
  }

  const realGpuAdapterReady =
    input.capability.realGpuAdapter &&
    selfBenchmarkReceipt?.adapter === "real_gpu" &&
    selfBenchmarkReceipt.blockerRefs.length === 0 &&
    selfBenchmarkReceipt.parity.parityPassed
  const canArmRealServing = blockerRefs.size === 0
  const paidRoutingEligible =
    canArmRealServing && (input.requireGatewayRouteReady ? gatewayRouteReady : true)

  const preflight: PylonRealServingReadinessPreflight = {
    schema: PYLON_SERVING_PREFLIGHT_SCHEMA,
    observedAt: input.observedAt,
    canArmRealServing,
    paidRoutingEligible,
    ownerApprovalRef,
    admittedPylonRef,
    fabricTransportReady: input.fabricTransportReady,
    gatewayRouteReady,
    realGpuAdapterReady,
    allowedEngines,
    residentModelRefs: realWarmResidency.map((entry) => entry.modelRef),
    evidenceRefs: [...evidenceRefs],
    blockerRefs: [...blockerRefs],
  }
  assertPublicProjectionSafe(preflight)
  return preflight
}

/**
 * Capability refs publishable from serving evidence. The serving capability
 * ref is published ONLY when the node is actually self-benchmarked with a
 * receipt and has no blockers — an unproven serving claim never leaves the
 * device, mirroring the executor no-overclaim rule.
 */
export function publishableServingCapabilityRefs(
  evidence: PylonServingCapabilityEvidence,
): string[] {
  if (!evidence.selfBenchmarked || evidence.blockerRefs.length > 0) return []
  if (evidence.selfBenchmarkReceiptRef === null) return []
  return [PYLON_SERVING_CAPABILITY_REF, evidence.selfBenchmarkReceiptRef]
}

/**
 * Strip an unreceipted serving capability claim from a list of capability refs.
 * Mirrors the executor no-overclaim rule: the serving capability ref may only
 * be published alongside its self-benchmark receipt ref. If the serving ref is
 * present but no matching `receipt.pylon.serving.self_bench.*` ref accompanies
 * it, drop the serving ref so an unproven serving claim never leaves the
 * device.
 */
export function stripUnreceiptedServingCapability(refs: string[]): string[] {
  if (!refs.includes(PYLON_SERVING_CAPABILITY_REF)) return refs
  const hasReceipt = refs.some((ref) => ref.startsWith(PYLON_SERVING_SELF_BENCH_RECEIPT_PREFIX))
  if (hasReceipt) return refs
  return refs.filter((ref) => ref !== PYLON_SERVING_CAPABILITY_REF)
}

/** A warm, parity-proven residency on this node, if any. */
export function warmResidency(
  evidence: PylonServingCapabilityEvidence,
): ServingModelResidency[] {
  return evidence.residency.filter((entry) => entry.residency === "warm")
}

export { COLD_START_ZERO }
