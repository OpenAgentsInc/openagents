/**
 * Kernel-optimization market dispatch encoder.
 *
 * Advances `compute.agentic_kernel_optimization_at_scale.v1`'s
 * market-dispatch blocker by turning the paper kernel-optimization WORK
 * DEFINITION
 * (docs/tassadar/2026-06-19-agentic-kernel-optimization-work-definition-and-parity-protocol.md)
 * into a concrete, dispatchable job on the already-green verified-work rail
 * (`labor.forum_work_requests.v1`).
 *
 * A kernel-optimization job names a target model + device, a named-baseline
 * tok/s record, and binds the output-parity verdict
 * (`KERNEL_OPTIMIZATION_PARITY_CLASS_ID`, verified via the exact-trace-replay
 * engine in `packages/tassadar-executor`) as the verification command. This
 * encoder produces the exact `CreateForumWorkRequestBody` the forum
 * work-request route already accepts, so a kernel job can be posted through the
 * same dispatch + settlement rail as any other verified-work request.
 *
 * It moves no money and makes no serving claim; it only constructs the
 * dispatch payload. Acceptance still requires a `Verified` parity verdict AND a
 * throughput improvement over the named baseline
 * (see `verifyKernelOptimizationParity`).
 */
import {
  KERNEL_OPTIMIZATION_PARITY_CLASS_ID,
  TASSADAR_EXECUTOR_CAPABILITY_REF,
} from '@openagentsinc/tassadar-executor'

import {
  type CreateForumWorkRequestBody,
  decodeCreateForumWorkRequestBody,
} from './forum-work-request-route-contract'

export const KERNEL_OPTIMIZATION_DISPATCH_CLASS_ID =
  'kernel_optimization_work_dispatch.v1'

/** Capability ref every kernel-optimization job requires of a provider. */
export const KERNEL_OPTIMIZATION_CAPABILITY_REF =
  'capability.kernel_optimization.author_optimized_kernel'

/** Max length the forum work-request route allows for a single public ref. */
const MAX_REF_LENGTH = 220

export class KernelOptimizationDispatchError extends Error {
  constructor(message: string) {
    super(message)
    this.name = 'KernelOptimizationDispatchError'
  }
}

/**
 * A kernel-optimization unit of accepted work, in the shape of the paper work
 * definition. Every field is public-safe; no prompts, credentials, or raw
 * kernel source are carried here (the deliverable is referenced, not inlined).
 */
export type KernelOptimizationJobSpec = Readonly<{
  /** Named open model, e.g. "qwen-3.5-0.5b". */
  targetModel: string
  /** Named device class, e.g. "cuda", "metal", "webgpu". */
  device: string
  /** Declared hardware the baseline tok/s was measured on. */
  hardwareRef: string
  /** Kernel/op to optimize, e.g. "rmsnorm", "attention.flash". */
  kernelRef: string
  /** Named-baseline throughput (tok/s) on the declared target; finite, > 0. */
  baselineTokensPerSecond: number
  /** Public ref to the baseline tok/s record (a number on declared hardware). */
  baselineRecordRef: string
  /** Independent validator device that replays for the parity verdict. */
  validatorDeviceRef: string
  /** Budget escrowed for the job, in whole sats; positive integer. */
  budgetSats: number
  /** Public deadline ref for the job. */
  deadlineRef: string
}>

const requireNonEmptyRef = (value: string, field: string): string => {
  const trimmed = value.trim()
  if (trimmed.length === 0) {
    throw new KernelOptimizationDispatchError(`${field} must be non-empty.`)
  }
  if (trimmed.length > MAX_REF_LENGTH) {
    throw new KernelOptimizationDispatchError(
      `${field} must be at most ${MAX_REF_LENGTH} characters.`,
    )
  }
  return trimmed
}

const slugify = (value: string): string =>
  value
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')

/**
 * Build a verified-work dispatch payload for a kernel-optimization job.
 *
 * The returned body is the exact `CreateForumWorkRequestBody` the forum
 * work-request route accepts; this function round-trips it through
 * `decodeCreateForumWorkRequestBody` so any value it returns is guaranteed
 * dispatch-valid.
 *
 * The `verificationCommandRef` binds the parity verdict: it names the parity
 * class, the baseline tok/s floor, the named-baseline record, and the
 * independent validator device, so acceptance is mechanically checkable
 * (faster AND still correct), not operator judgment.
 */
export const buildKernelOptimizationWorkRequest = (
  spec: KernelOptimizationJobSpec,
): CreateForumWorkRequestBody => {
  if (
    !Number.isFinite(spec.baselineTokensPerSecond) ||
    spec.baselineTokensPerSecond <= 0
  ) {
    throw new KernelOptimizationDispatchError(
      `baselineTokensPerSecond must be finite and > 0 (got ${spec.baselineTokensPerSecond}).`,
    )
  }
  if (
    !Number.isInteger(spec.budgetSats) ||
    spec.budgetSats <= 0 ||
    spec.budgetSats > 21_000_000_000_000
  ) {
    throw new KernelOptimizationDispatchError(
      'budgetSats must be a positive integer sat amount.',
    )
  }

  const targetModel = requireNonEmptyRef(spec.targetModel, 'targetModel')
  const device = requireNonEmptyRef(spec.device, 'device')
  const hardwareRef = requireNonEmptyRef(spec.hardwareRef, 'hardwareRef')
  const kernelRef = requireNonEmptyRef(spec.kernelRef, 'kernelRef')
  const baselineRecordRef = requireNonEmptyRef(
    spec.baselineRecordRef,
    'baselineRecordRef',
  )
  const validatorDeviceRef = requireNonEmptyRef(
    spec.validatorDeviceRef,
    'validatorDeviceRef',
  )

  const title = requireNonEmptyRef(
    `Kernel optimization: ${kernelRef} for ${targetModel} on ${device}`.slice(
      0,
      160,
    ),
    'title',
  )

  const objectiveRef = requireNonEmptyRef(
    `kernel-optimization:beat ${spec.baselineTokensPerSecond} tok/s on ${targetModel}/${device}/${hardwareRef} for ${kernelRef}`,
    'objectiveRef',
  )

  const verificationCommandRef = requireNonEmptyRef(
    `tassadar-parity:${KERNEL_OPTIMIZATION_PARITY_CLASS_ID}|baseline=${baselineRecordRef}|validator=${validatorDeviceRef}|min_tok_s=${spec.baselineTokensPerSecond}`,
    'verificationCommandRef',
  )

  const slugBase = slugify(`kernel-opt-${kernelRef}-${targetModel}-${device}`)
  const requestedSlug = requireNonEmptyRef(
    slugBase.slice(0, 80).replace(/-+$/g, '') || 'kernel-opt-job',
    'requestedSlug',
  )

  const body = {
    budgetSats: spec.budgetSats,
    deadlineRef: requireNonEmptyRef(spec.deadlineRef, 'deadlineRef'),
    objectiveRef,
    requestedSlug,
    requiredCapabilityRefs: [
      KERNEL_OPTIMIZATION_CAPABILITY_REF,
      TASSADAR_EXECUTOR_CAPABILITY_REF,
      `capability.kernel_optimization.device.${slugify(device)}`,
    ],
    title,
    verificationCommandRef,
  }

  return decodeCreateForumWorkRequestBody(body)
}
