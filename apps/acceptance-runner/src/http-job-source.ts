// HTTP job source: leases acceptance jobs from the gateway over an authenticated pull
// endpoint (EPIC #6017).
//
// The out-of-Worker runner CANNOT be a Cloudflare Queue consumer (a consumer is a
// Worker; chromium never runs in a Worker), so it PULLS its work. This source polls an
// authenticated `GET <leaseUrl>` that returns the next pending acceptance job (or 204 /
// `{ job: null }` when the queue is empty), runs it, and reports the outcome back with
// `POST <ackUrl>`. Both calls carry the runner bearer token. The lease/ack endpoints are
// the Worker-side counterpart (`acceptance-job-lease-routes.ts`), INERT by default.
//
// FAIL-CLOSED: a missing token or URL means no source (the daemon does nothing). A
// non-2xx is a typed transport fault the daemon treats as a transient idle (back off,
// retry) — it never fabricates a job or a verdict.

import { AcceptanceJobMessage } from './harness-bridge'
import {
  type JobSource,
  type LeasedJob,
  JobLeaseTransportError,
} from './job-source'

export type HttpJobSourceConfig = Readonly<{
  // The authenticated lease endpoint (GET). Returns the next job or an empty signal.
  leaseUrl: string
  // The authenticated ack endpoint (POST). Reports delivered / retryable per leaseId.
  ackUrl: string
  // The runner bearer token (the SAME ACCEPTANCE_VERDICT_CALLBACK_TOKEN the verdict
  // callback uses, so one secret authenticates the whole runner<->gateway channel).
  bearerToken: string
  // Injectable for tests.
  fetchFn?: typeof fetch
}>

// Decode the lease response body into a typed leased job, or null. The Worker returns
// `{ leaseId, job }` for a leased job, or `{ job: null }` / 204 for the idle case. The
// `job` is decoded through the SAME `AcceptanceJobMessage` schema the dispatch produces.
const decodeLeasedJob = (value: unknown): LeasedJob | null => {
  if (value === null || typeof value !== 'object') return null
  const record = value as Record<string, unknown>
  if (record.job === null || record.job === undefined) return null
  const leaseId = record.leaseId
  if (typeof leaseId !== 'string' || leaseId.trim() === '') return null
  // Throws on a malformed job — a malformed lease is a transport fault, not a silent
  // skip; the daemon backs off rather than running garbage.
  const message = AcceptanceJobMessage.make(
    AcceptanceJobMessageFromUnknown(record.job),
  )
  return { leaseId, message }
}

// AcceptanceJobMessage.make wants the decoded fields; the wire carries them as plain
// JSON. We decode through the class' schema to validate the shape at the boundary.
const AcceptanceJobMessageFromUnknown = (
  value: unknown,
): AcceptanceJobMessage => {
  // The class is an Effect Schema class; decoding an unknown validates every field
  // (schemaVersion literal, spec params, etc.). A bad payload throws here.
  return AcceptanceJobMessage.make(value as AcceptanceJobMessage)
}

export const makeHttpJobSource = (config: HttpJobSourceConfig): JobSource => {
  const fetchFn = config.fetchFn ?? fetch
  const authHeaders = {
    authorization: `Bearer ${config.bearerToken}`,
    'content-type': 'application/json',
  }
  return {
    ack: async ({ leaseId, delivered }) => {
      const response = await fetchFn(config.ackUrl, {
        body: JSON.stringify({ delivered, leaseId }),
        headers: authHeaders,
        method: 'POST',
      })
      if (!response.ok) {
        throw new JobLeaseTransportError('ack', response.status)
      }
    },
    label: `http:${config.leaseUrl}`,
    lease: async () => {
      const response = await fetchFn(config.leaseUrl, {
        headers: authHeaders,
        method: 'GET',
      })
      // 204 = no pending job (the idle case).
      if (response.status === 204) return null
      if (!response.ok) {
        throw new JobLeaseTransportError('lease', response.status)
      }
      const body = (await response.json().catch(() => null)) as unknown
      return decodeLeasedJob(body)
    },
  }
}
