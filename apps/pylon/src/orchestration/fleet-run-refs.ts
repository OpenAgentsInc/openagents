export const fleetRunTaskIdForClaim = (runRef: string, claimRef: string): string =>
  `${runRef}.task.${claimRef.replace(/[^a-zA-Z0-9_.-]/g, "_")}`
