// The single import boundary between the standalone runner service and the canonical
// acceptance-runner harness that lives in the openagents.com worker source (EPIC #6017).
//
// WHY A BRIDGE. The real headless executor (`runner.ts`), the pluggable host seam
// (`harness.ts`), the verdict/spec types, and the wire `AcceptanceJobMessage` schema all
// live under `apps/openagents.com/workers/api/src/inference/` — that is their canonical
// home and a concurrent lane owns them. The standalone service MUST NOT fork or
// re-implement any of that logic; it only ORCHESTRATES it on a node with chromium. This
// module is the ONE place the service reaches into the worker source, so the rest of the
// service imports `./harness-bridge` and never deep-imports the worker tree. If the
// harness moves to a published package later, only this file changes.
//
// Relative path from apps/acceptance-runner/src to the worker inference source.

export {
  type RunnerTransport,
  type RunAcceptanceJobResult,
  type VerdictCallbackPayload,
  AcceptanceJobMessage,
  runAcceptanceJob,
  makeFetchVerdictPoster,
  specFromJobSpec,
  VerdictCallbackDeliveryError,
} from '../../openagents.com/workers/api/src/inference/acceptance-runner/harness'

export { runAcceptanceSuite } from '../../openagents.com/workers/api/src/inference/acceptance-runner/runner'
export type { AcceptanceVerdict } from '../../openagents.com/workers/api/src/inference/acceptance-runner/verdict'
export { crossyRoadAcceptanceSpec } from '../../openagents.com/workers/api/src/inference/acceptance-spec'

// The verdict-callback ingest + verification store, re-exported for the in-process
// end-to-end proof (run the real headless suite -> POST the verdict through the REAL
// route -> assert the receipt backfills verified:true). Prod uses the D1 store; the
// proof uses the in-memory reference store.
export {
  handleAcceptanceVerdictCallback,
} from '../../openagents.com/workers/api/src/inference/acceptance-verdict-callback-routes'
export {
  makeInMemoryKhalaVerificationStore,
} from '../../openagents.com/workers/api/src/inference/acceptance-dispatch'
