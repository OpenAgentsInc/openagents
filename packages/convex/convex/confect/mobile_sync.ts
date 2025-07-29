// Re-export mobile sync functions from confect
export {
  createClaudeSession,
  updateSessionStatus,
  getPendingMobileSessions,
  getSessions,
  getSession,
  getSessionMessages,
  addClaudeMessage,
  batchAddMessages,
  requestDesktopSession,
  updateSyncStatusConfect as updateSyncStatus,
  getSyncStatusConfect as getSyncStatus,
  syncSessionFromHook,
  markMobileSessionProcessed,
} from "../../confect/mobile_sync";