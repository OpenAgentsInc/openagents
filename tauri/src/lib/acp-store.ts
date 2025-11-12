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
    console.log(`[acp-store] listen start`, { topic, sessionId });
    let silenceTimer: ReturnType<typeof setTimeout> | undefined;

    const onNotif = (payload?: SessionNotification) => {
      console.log("[acp-store] incoming", payload);
      if (!payload || !(payload as any).update) return;
      const sn = payload as any;
      // Try to filter by session if present; otherwise accept
      if (sn.sessionId && sn.sessionId !== sessionId) return;
      const update: any = sn.update;
      const kind: string | undefined = update.sessionUpdate;

      if (kind === "agent_message_chunk") {
        console.log(`[acp-store] msg chunk`, update);
        set((s) => ({ isStreaming: true, liveText: appendContentText(s.liveText, update.content as ContentBlock) }));
      } else if (kind === "agent_thought_chunk") {
        console.log(`[acp-store] thought chunk`, update);
        set((s) => ({ isStreaming: true, thoughtText: appendContentText(s.thoughtText, update.content as ContentBlock) }));
      } else {
        // other updates: ignore in phase 2
        return;
      }

      // Debounce end-of-stream indicator
      if (silenceTimer) clearTimeout(silenceTimer);
      silenceTimer = setTimeout(() => {
        set({ isStreaming: false });
        console.log(`[acp-store] stream idle`);
      }, 800);
    };

    const un1 = await listen<SessionNotification>(topic, (evt: Event<SessionNotification>) => onNotif(evt.payload));
    const un2 = await listen<SessionNotification>("acp:update", (evt: Event<SessionNotification>) => onNotif(evt.payload));
    const un = () => { try { un1(); } catch {} try { un2(); } catch {} };
    

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
