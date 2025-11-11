import { create } from "zustand";
import { listen, type Event } from "@tauri-apps/api/event";
import type { SessionNotification, ContentBlock } from "@agentclientprotocol/sdk";

type UnlistenFn = () => void;

type AcpState = {
  activeSessionId?: string;
  isStreaming: boolean;
  liveText: string;
  thoughtText: string;
  unlisten?: UnlistenFn;
  setActiveSession: (sessionId: string) => void;
  startListening: (sessionId: string) => Promise<void>;
  stopListening: () => void;
  reset: () => void;
};

function appendContentText(prev: string, block: ContentBlock): string {
  if (block.type === "text") return prev + block.text;
  // Ignore non-text for minimal Phase 2 scope
  return prev;
}

export const useAcpStore = create<AcpState>((set, get) => ({
  activeSessionId: undefined,
  isStreaming: false,
  liveText: "",
  thoughtText: "",
  unlisten: undefined,
  setActiveSession(sessionId) {
    set({ activeSessionId: sessionId });
  },
  async startListening(sessionId) {
    // Stop previous listener if any
    const prevUnlisten = get().unlisten;
    if (prevUnlisten) {
      try { prevUnlisten(); } catch {}
    }

    // Subscribe to per-session channel
    const topic = `session:${sessionId}`;
    const un = await listen<SessionNotification>(topic, (evt: Event<SessionNotification>) => {
      const payload = evt.payload;
      if (!payload || !payload.update) return;
      const update: any = payload.update as any;
      const kind: string | undefined = update.sessionUpdate;

      if (kind === "agent_message_chunk") {
        set((s) => ({ isStreaming: true, liveText: appendContentText(s.liveText, update.content as ContentBlock) }));
        return;
      }
      if (kind === "agent_thought_chunk") {
        set((s) => ({ isStreaming: true, thoughtText: appendContentText(s.thoughtText, update.content as ContentBlock) }));
        return;
      }
      // TODO: handle other update kinds in later phases
    });

    set({ activeSessionId: sessionId, unlisten: un, isStreaming: false, liveText: "", thoughtText: "" });
  },
  stopListening() {
    const prevUnlisten = get().unlisten;
    if (prevUnlisten) {
      try { prevUnlisten(); } catch {}
    }
    set({ unlisten: undefined, isStreaming: false });
  },
  reset() {
    set({ isStreaming: false, liveText: "", thoughtText: "" });
  },
}));

