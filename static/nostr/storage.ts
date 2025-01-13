import { ModerationAction, ChatState } from "./types";

const STORAGE_KEYS = {
  HIDDEN_MESSAGES: "nostr-chat:hidden-messages",
  MUTED_USERS: "nostr-chat:muted-users",
  MODERATION_ACTIONS: "nostr-chat:moderation-actions",
  CHANNEL_METADATA: "nostr-chat:channel-metadata",
};

export class ChatStorage {
  private state: ChatState;

  constructor() {
    this.state = this.loadState();
  }

  private loadState(): ChatState {
    return {
      messages: new Map(),
      hiddenMessages: new Set(this.getHiddenMessages()),
      mutedUsers: new Set(this.getMutedUsers()),
      moderationActions: this.getModerationActions(),
    };
  }

  // Moderation Storage
  private getHiddenMessages(): string[] {
    const stored = localStorage.getItem(STORAGE_KEYS.HIDDEN_MESSAGES);
    return stored ? JSON.parse(stored) : [];
  }

  private getMutedUsers(): string[] {
    const stored = localStorage.getItem(STORAGE_KEYS.MUTED_USERS);
    return stored ? JSON.parse(stored) : [];
  }

  private getModerationActions(): ModerationAction[] {
    const stored = localStorage.getItem(STORAGE_KEYS.MODERATION_ACTIONS);
    return stored ? JSON.parse(stored) : [];
  }

  // Public Methods
  hideMessage(messageId: string, reason?: string) {
    this.state.hiddenMessages.add(messageId);
    const action: ModerationAction = {
      type: "hide",
      target: messageId,
      reason,
      timestamp: Date.now(),
    };
    this.state.moderationActions.push(action);
    this.persist();
  }

  muteUser(pubkey: string, reason?: string) {
    this.state.mutedUsers.add(pubkey);
    const action: ModerationAction = {
      type: "mute",
      target: pubkey,
      reason,
      timestamp: Date.now(),
    };
    this.state.moderationActions.push(action);
    this.persist();
  }

  isMessageHidden(messageId: string): boolean {
    return this.state.hiddenMessages.has(messageId);
  }

  isUserMuted(pubkey: string): boolean {
    return this.state.mutedUsers.has(pubkey);
  }

  getModerationState() {
    return {
      hiddenMessages: Array.from(this.state.hiddenMessages),
      mutedUsers: Array.from(this.state.mutedUsers),
      actions: this.state.moderationActions,
    };
  }

  private persist() {
    localStorage.setItem(
      STORAGE_KEYS.HIDDEN_MESSAGES,
      JSON.stringify(Array.from(this.state.hiddenMessages)),
    );
    localStorage.setItem(
      STORAGE_KEYS.MUTED_USERS,
      JSON.stringify(Array.from(this.state.mutedUsers)),
    );
    localStorage.setItem(
      STORAGE_KEYS.MODERATION_ACTIONS,
      JSON.stringify(this.state.moderationActions),
    );
  }

  // Channel Metadata Cache
  cacheChannelMetadata(channelId: string, metadata: any) {
    const cache = this.getChannelMetadataCache();
    cache[channelId] = {
      ...metadata,
      timestamp: Date.now(),
    };
    localStorage.setItem(STORAGE_KEYS.CHANNEL_METADATA, JSON.stringify(cache));
  }

  getChannelMetadata(channelId: string) {
    const cache = this.getChannelMetadataCache();
    return cache[channelId];
  }

  private getChannelMetadataCache(): Record<string, any> {
    const stored = localStorage.getItem(STORAGE_KEYS.CHANNEL_METADATA);
    return stored ? JSON.parse(stored) : {};
  }
}
