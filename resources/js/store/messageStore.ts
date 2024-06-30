import { create } from "zustand";

interface Message {
  id: string;
  content: string;
  isUser: boolean;
  isComplete: boolean;
}

interface MessageState {
  messages: Message[];
  addMessage: (content: string, isUser: boolean) => void;
  updateLastMessage: (content: string) => void;
  setLastMessageComplete: () => void;
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
            isComplete: isUser, // User messages are always complete
          },
        ],
      })),
    updateLastMessage: (content) =>
      set((state) => {
        const lastMessage = state.messages[state.messages.length - 1];
        if (!lastMessage || lastMessage.isUser || lastMessage.isComplete) {
          // If there's no last message, or it's a user message, or it's already complete,
          // add a new message instead of updating
          return {
            messages: [
              ...state.messages,
              {
                id: Date.now().toString(),
                content,
                isUser: false,
                isComplete: false,
              },
            ],
          };
        }
        return {
          messages: [
            ...state.messages.slice(0, -1),
            { ...lastMessage, content: lastMessage.content + content },
          ],
        };
      }),
    setLastMessageComplete: () =>
      set((state) => {
        const lastMessage = state.messages[state.messages.length - 1];
        if (!lastMessage || lastMessage.isUser || lastMessage.isComplete)
          return state;
        return {
          messages: [
            ...state.messages.slice(0, -1),
            { ...lastMessage, isComplete: true },
          ],
        };
      }),
  }));
