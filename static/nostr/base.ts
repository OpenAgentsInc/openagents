import NDK, {
  NDKEvent,
  NDKSubscription,
  NDKNip07Signer,
} from "@nostr-dev-kit/ndk";
import { NostrChatConfig, ChatState, ChannelMetadata } from "./types";
import { ChatStorage } from "./storage";

export class NostrChatBase {
  protected config: NostrChatConfig;
  protected state: ChatState;
  protected storage: ChatStorage;
  protected templates: Map<string, HTMLTemplateElement>;
  protected signer: NDKNip07Signer | null = null;
  protected api: any;

  constructor() {
    this.config = {
      defaultRelays: [
        "wss://nostr-pub.wellorder.net",
        "wss://nostr.mom",
        "wss://relay.nostr.band",
      ],
      messageTemplate: "#message-template",
      autoScroll: true,
      moderationEnabled: true,
      pollInterval: 5000,
      messageLimit: 50,
    };
    this.storage = new ChatStorage();
    this.templates = new Map();
    this.state = {
      messages: new Map(),
      hiddenMessages: new Set(),
      mutedUsers: new Set(),
      moderationActions: [],
      channels: new Map(),
    };
  }

  // Expose state to child classes
  getSigner() {
    return this.signer;
  }
  getState() {
    return this.state;
  }
  getConfig() {
    return this.config;
  }
  getTemplates() {
    return this.templates;
  }
  getStorage() {
    return this.storage;
  }

  // Utility methods
  replaceTemplateVariables(element: HTMLElement, data: Record<string, any>) {
    const walker = document.createTreeWalker(
      element,
      NodeFilter.SHOW_TEXT | NodeFilter.SHOW_ELEMENT,
      null,
    );

    let node;
    while ((node = walker.nextNode())) {
      if (node.nodeType === Node.TEXT_NODE) {
        node.textContent = node.textContent?.replace(
          /\{\{(\w+)\}\}/g,
          (_, key) => data[key] || "",
        );
      } else if (node instanceof Element) {
        Array.from(node.attributes).forEach((attr) => {
          attr.value = attr.value.replace(
            /\{\{(\w+)\}\}/g,
            (_, key) => data[key] || "",
          );
        });
      }
    }
  }

  dispatchEvent(name: string, detail: any) {
    document.dispatchEvent(new CustomEvent(name, { detail }));
  }

  handleError(message: string, error: any) {
    console.error(message, error);
    this.dispatchEvent("nostr-chat:error", { message, error });
  }
}
