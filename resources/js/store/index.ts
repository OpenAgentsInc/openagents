import { createMessageStore } from "./messageStore";
import { create } from "zustand";

const useStore = create(() => ({
  messageStore: createMessageStore(),
}));

export const useMessageStore = createMessageStore();
