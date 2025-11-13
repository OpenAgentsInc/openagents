import { create } from "zustand";

type WorkingDirState = {
  // Default working directory for new threads
  defaultCwd: string;

  // Per-thread working directory overrides
  perThreadCwd: Map<string, string>;

  // Warning state (e.g., directory no longer exists)
  warning: { threadId: string; message: string } | null;

  // Set the default working directory (persisted to localStorage)
  setDefaultCwd: (path: string) => void;

  // Set working directory for a specific thread
  setThreadCwd: (threadId: string, path: string) => void;

  // Get working directory for a specific thread (falls back to default)
  getThreadCwd: (threadId: string | undefined) => string;

  // Clear warning
  clearWarning: () => void;

  // Set warning
  setWarning: (threadId: string, message: string) => void;

  // Initialize from localStorage
  initialize: () => void;
};

// Get default working directory from localStorage or use process.cwd()
const getInitialDefaultCwd = (): string => {
  if (typeof window !== "undefined") {
    const saved = localStorage.getItem("openagents_default_cwd");
    if (saved) {
      return saved;
    }
  }
  // Fallback - will be updated by App.tsx when Tauri reports actual cwd
  return "";
};

export const useWorkingDirStore = create<WorkingDirState>((set, get) => ({
  defaultCwd: getInitialDefaultCwd(),
  perThreadCwd: new Map(),
  warning: null,

  setDefaultCwd: (path: string) => {
    if (typeof window !== "undefined") {
      localStorage.setItem("openagents_default_cwd", path);
    }
    set({ defaultCwd: path });
  },

  setThreadCwd: (threadId: string, path: string) => {
    const newMap = new Map(get().perThreadCwd);
    newMap.set(threadId, path);
    set({ perThreadCwd: newMap });
  },

  getThreadCwd: (threadId: string | undefined) => {
    if (!threadId) {
      return get().defaultCwd;
    }
    const perThread = get().perThreadCwd.get(threadId);
    return perThread || get().defaultCwd;
  },

  clearWarning: () => set({ warning: null }),

  setWarning: (threadId: string, message: string) =>
    set({ warning: { threadId, message } }),

  initialize: () => {
    const defaultCwd = getInitialDefaultCwd();
    set({ defaultCwd });
  },
}));
