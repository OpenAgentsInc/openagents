import { create } from "zustand";

interface Message {
  id: string;
  content: string;
  isUser: boolean;
}

interface MessageState {
  messages: Message[];
  addMessage: (content: string, isUser: boolean) => void;
}

export const createMessageStore = () =>
  create<MessageState>((set) => ({
    messages: [],
    addMessage: (content, isUser) =>
      set((state) => ({
        messages: [
          ...state.messages,
          {
            id: Date.now().toString(),
            content,
            isUser,
          },
        ],
      })),
  }));
