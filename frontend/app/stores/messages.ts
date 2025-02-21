import { create } from "zustand";
import { devtools } from "zustand/middleware";

export interface Message {
  id: string;
  role: string;
  content: string;
  reasoning?: string; // Add reasoning field
  metadata?: {
    repos?: string[];
  };
}

interface MessagesState {
  messages: Record<string, Message[]>;
  addMessage: (chatId: string, message: Message) => void;
  setMessages: (chatId: string, messages: Message[]) => void;
  removeMessages: (chatId: string) => void;
}

export const useMessagesStore = create<MessagesState>()(
  devtools(
    (set) => ({
      messages: {},
      addMessage: (chatId, message) =>
        set(
          (state) => ({
            messages: {
              ...state.messages,
              [chatId]: [...(state.messages[chatId] || []), message],
            },
          }),
          false,
          "addMessage",
        ),
      setMessages: (chatId, messages) =>
        set(
          (state) => ({
            messages: {
              ...state.messages,
              [chatId]: messages,
            },
          }),
          false,
          "setMessages",
        ),
      removeMessages: (chatId) =>
        set(
          (state) => {
            const { [chatId]: _, ...rest } = state.messages;
            return { messages: rest };
          },
          false,
          "removeMessages",
        ),
    }),
    {
      name: "MessagesStore",
    },
  ),
);
