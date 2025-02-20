import { create } from "zustand";
import { devtools } from "zustand/middleware";

export interface Message {
  id: string;
  role: string;
  content: string;
  metadata?: {
    repos?: string[];
  };
}

interface MessagesState {
  messages: Record<string, Message[]>;
  addMessage: (chatId: string, message: Message) => void;
  setMessages: (chatId: string, messages: Message[]) => void;
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
          "addMessage"
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
          "setMessages"
        ),
    }),
    {
      name: "MessagesStore",
    }
  )
);