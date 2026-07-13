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

export const selectExactManagedFleetProviderAccount = async <Account>(
  accounts: readonly Account[],
  expectedAccountRefHash: string,
  accountRefHash: (account: Account) => Promise<string>,
): Promise<Readonly<{ account: Account; accountRefHash: string }> | undefined> => {
  const candidates = await Promise.all(
    accounts.map(async account => ({
      account,
      accountRefHash: await accountRefHash(account),
    })),
  )
  const matches = candidates.filter(
    candidate => candidate.accountRefHash === expectedAccountRefHash,
  )
  return matches.length === 1 ? matches[0] : undefined
}
