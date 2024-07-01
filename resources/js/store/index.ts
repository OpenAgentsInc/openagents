import { createMessageStore, useMessageStore } from "./messageStore";
import { useCodebaseStore } from "./codebaseStore";
import { create } from "zustand";

const useStore = create(() => ({
  messageStore: createMessageStore(),
  codebaseStore: useCodebaseStore(),
}));

export { useMessageStore, useCodebaseStore };
export default useStore;
