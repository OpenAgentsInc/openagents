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
  currentPlan: string;
  addMessage: (content: string, isUser: boolean, isComplete?: boolean) => void;
  updateLastMessage: (content: string) => void;
  setLastMessageComplete: () => void;
  updateCurrentPlan: (planContent: string) => void;
  appendToPlan: (planContent: string) => void;
  addGreptileResult: (content: string) => void;
}

const createMessageSlice: StateCreator<MessageState> = (set) => ({
  messages: [],
  currentPlan: "",
  addMessage: (content, isUser, isComplete = false) =>
    set((state) => ({
      messages: [
        ...state.messages,
        {
          id: uuidv4(),
          content: content.trim(),
          isUser,
          isComplete,
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
              content: content.trim(),
              isUser: false,
              isComplete: false,
            },
          ],
        };
      }
      return {
        messages: [
          ...state.messages.slice(0, -1),
          { ...lastMessage, content: (lastMessage.content + content).trim() },
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
  updateCurrentPlan: (planContent) =>
    set(() => ({
      currentPlan: planContent,
    })),
  appendToPlan: (planContent) =>
    set((state) => ({
      currentPlan: state.currentPlan + planContent,
    })),
  addGreptileResult: (content) =>
    set((state) => ({
      messages: [
        ...state.messages,
        {
          id: uuidv4(),
          content: `${content}`,
          isUser: false,
          isComplete: true,
          isGreptileResult: true,
        },
      ],
    })),
});

export const createMessageStore = () =>
  create<MessageState>(createMessageSlice);

export const useMessageStore = createMessageStore();
