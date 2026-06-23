// The JOB-SOURCE seam for the out-of-Worker acceptance runner (EPIC #6017).
//
// THE PROBLEM. The gateway (a Cloudflare Worker) must hand acceptance jobs to a node
// with chromium — but a Cloudflare Queue CONSUMER is itself a Worker, and chromium can
// NEVER run in a Worker. So the out-of-Worker runner cannot be a Queue consumer; it has
// to PULL its work over an authenticated HTTP endpoint (a job lease) OR be handed a
// single job locally for a one-shot run. This module is the pluggable seam for "where
// does the next job come from", mirroring the `RunnerTransport` seam in the harness
// (`acceptance-runner/harness.ts`) for "where does the artifact come from / where does
// the verdict go".
//
// FAIL-CLOSED + INERT: the HTTP job source is configured out of band (the runner host's
// local secret). With no lease URL / no bearer token the daemon has no job source and
// does nothing — it never invents work. A poll that returns "no job" is the normal idle
// state, not an error.

import type { AcceptanceJobMessage } from './harness-bridge'

// One leased job plus the lease handle the source needs to ack/extend it. `leaseId` is
// opaque to the runner — it is echoed back to the source so the Worker can mark the job
// done / retryable. `message` is the typed `AcceptanceJobMessage` the harness runs.
export type LeasedJob = Readonly<{
  leaseId: string
  message: AcceptanceJobMessage
}>

// The pluggable job source. `lease()` returns the next pending job or `null` when the
// queue is empty (the idle case — NOT an error). `ack()` reports the terminal outcome of
// a leased job back to the source so it can be removed (delivered) or re-queued
// (retryable). A source that has no ack concept (a one-shot file source) implements
// `ack` as a no-op.
export type JobSource = Readonly<{
  lease: () => Promise<LeasedJob | null>
  ack: (
    input: Readonly<{ leaseId: string; delivered: boolean }>,
  ) => Promise<void>
  // A human label for logs (e.g. "http:https://openagents.com/...", "file:job.json").
  readonly label: string
}>

// A typed lease-transport fault so a non-2xx lease/ack response is legible (status as a
// field, not parsed from a string) — the same `*Error extends Error` discipline the
// harness uses for delivery faults. The daemon treats a lease fault as a transient idle
// (back off and retry), never as a reason to fabricate a verdict.
export class JobLeaseTransportError extends Error {
  readonly status: number
  constructor(operation: string, status: number) {
    super(`job_lease_${operation}_failed: ${status}`)
    this.name = 'JobLeaseTransportError'
    this.status = status
  }
}
