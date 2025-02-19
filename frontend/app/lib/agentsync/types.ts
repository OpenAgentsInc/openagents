export interface SyncState {
  isOnline: boolean;
  lastSyncId: number;
  pendingChanges: number;
}

export interface SyncOptions {
  scope: string;
  models?: string[];
  subscribe?: string[];
}

export interface StartChatResponse {
  id: string;
  initialMessage: string;
}

export interface AgentSyncHook {
  state: SyncState;
  sendMessage: (content: string, repos?: string[]) => Promise<StartChatResponse>;
}