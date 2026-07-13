import { createHash } from 'node:crypto'

export type ManagedFleetDispatchAuthorityInput = Readonly<{
  acceptedRunLease: boolean
  runStatus: string
  runRef: string
  workUnitRef: string
  unitClaimRef: string
}>

/**
 * Keep the two FC-4 claims deliberately separate. The accepted Sarah intake
 * lease authorizes the Pylon for the whole run; the Pylon work claim names one
 * exact unit inside that run and must never be compared to the lease claim.
 */
export const authorizesManagedFleetUnitDispatch = (
  input: ManagedFleetDispatchAuthorityInput,
): boolean =>
  input.acceptedRunLease &&
  (input.runStatus === 'claimed_by_pylon' || input.runStatus === 'running') &&
  input.unitClaimRef.startsWith(
    `${input.runRef}.claim.${input.workUnitRef}.`,
  )

/**
 * The FleetRun claim registry owns worker identity. Managed dispatch may
 * resolve a private provider account behind the broker, but that resolution
 * must not replace the public worker identity already bound to the claim.
 */
export const managedFleetClaimAccountRefHash = (
  workerAccountRef: string,
): string =>
  `account.pylon.codex.${createHash('sha256')
    .update(`codex:${workerAccountRef}`)
    .digest('hex')
    .slice(0, 24)}`
