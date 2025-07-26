import { create } from 'zustand';
import { devtools } from 'zustand/middleware';

interface Message {
  id: string;
  message_type: string;
  content: string;
  timestamp: string;
  tool_info?: {
    tool_name: string;
    tool_use_id: string;
    input: Record<string, any>;
    output?: string;
  };
}

interface Session {
  id: string;
  projectPath: string;
  messages: Message[];
  inputMessage: string;
  isLoading: boolean;
  isInitializing?: boolean;
}

interface AppState {
  // Session state
  sessions: Session[];
  activeChatSession: string | null;
  newProjectPath: string;
  
  // Mobile sync state
  processedMobileSessions: Set<string>;
  isProcessingAnyMobileSession: boolean;
  
  // App state
  isAppInitialized: boolean;
  claudeStatus: string;
  
  // Actions
  setSessions: (sessions: Session[]) => void;
  addSession: (session: Session) => void;
  updateSession: (sessionId: string, updates: Partial<Session>) => void;
  removeSession: (sessionId: string) => void;
  setNewProjectPath: (path: string) => void;
  markMobileSessionProcessed: (sessionId: string) => void;
  setProcessingState: (isProcessing: boolean) => void;
  setAppInitialized: (isInitialized: boolean) => void;
  setClaudeStatus: (status: string) => void;
  setActiveChatSession: (sessionId: string | null) => void;
}

export const useAppStore = create<AppState>()(
  devtools((set) => ({
    // State
    sessions: [],
    activeChatSession: null,
    newProjectPath: "",
    processedMobileSessions: new Set(),
    isProcessingAnyMobileSession: false,
    isAppInitialized: false,
    claudeStatus: "Not initialized",
    
    // Actions
    setSessions: (sessions) => set({ sessions }),
    
    addSession: (session) => set((state) => ({ 
      sessions: [...state.sessions, session] 
    })),
    
    updateSession: (sessionId, updates) => set((state) => ({
      sessions: state.sessions.map(s => 
        s.id === sessionId ? { ...s, ...updates } : s
      )
    })),
    
    removeSession: (sessionId) => set((state) => ({
      sessions: state.sessions.filter(s => s.id !== sessionId)
    })),
    
    setNewProjectPath: (path) => set({ newProjectPath: path }),
    
    markMobileSessionProcessed: (sessionId) => set((state) => ({
      processedMobileSessions: new Set([...state.processedMobileSessions, sessionId])
    })),
    
    setProcessingState: (isProcessing) => set({ isProcessingAnyMobileSession: isProcessing }),
    
    setAppInitialized: (isInitialized) => set({ isAppInitialized: isInitialized }),
    
    setClaudeStatus: (status) => set({ claudeStatus: status }),
    
    setActiveChatSession: (sessionId) => set({ activeChatSession: sessionId }),
  }))
);