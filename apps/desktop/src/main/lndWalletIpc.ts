export const LND_WALLET_CHANNELS = {
  snapshot: "openagents:lnd-wallet:snapshot",
  initialize: "openagents:lnd-wallet:initialize",
  unlock: "openagents:lnd-wallet:unlock",
  lock: "openagents:lnd-wallet:lock",
  acknowledgeSeedBackup: "openagents:lnd-wallet:ack-seed-backup",
  prepareRestore: "openagents:lnd-wallet:prepare-restore",
  restore: "openagents:lnd-wallet:restore",
} as const;
