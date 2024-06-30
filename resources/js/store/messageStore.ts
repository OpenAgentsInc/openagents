import { create, StateCreator } from "zustand";
import { v4 as uuidv4 } from "uuid";

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

const createMessageSlice: StateCreator<MessageState> = (set) => ({
  messages: [],
  addMessage: (content, isUser) =>
    set((state) => ({
      messages: [
        ...state.messages,
        {
          id: uuidv4(),
          content,
          isUser,
          isComplete: true,
        },
      ],
    })),
  updateLastMessage: (content) =>
    set((state) => {
      const lastMessage = state.messages[state.messages.length - 1];
      if (!lastMessage || lastMessage.isUser || lastMessage.isComplete) {
        return {
          messages: [
            ...state.messages,
            {
              id: uuidv4(),
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
});

export const createMessageStore = () =>
  create<MessageState>(createMessageSlice);

export const useMessageStore = createMessageStore();
