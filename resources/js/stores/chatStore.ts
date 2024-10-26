import { create } from "zustand"
import { persist } from "zustand/middleware"

interface ChatState {
  selectedTools: string[];
  setSelectedTools: (tools: string[]) => void;
  selectedModel: { name: string; value: string };
  setSelectedModel: (model: { name: string; value: string }) => void;
  selectedCodebases: string[];
  setSelectedCodebases: (codebases: string[]) => void;
}

export const useChatStore = create(
  persist<ChatState>(
    (set) => ({
      selectedTools: [],
      setSelectedTools: (tools) => set({ selectedTools: tools }),
      selectedModel: { name: 'Claude 3.5 Sonnet', value: 'claude-3.5-sonnet' },
      setSelectedModel: (model) => set({ selectedModel: model }),
      selectedCodebases: [],
      setSelectedCodebases: (codebases) => set({ selectedCodebases: codebases }),
    }),
    {
      name: 'chat-storage-2',
    }
  )
);
