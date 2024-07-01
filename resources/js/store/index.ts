import { createMessageStore, useMessageStore } from "./messageStore";
import { createCodebaseStore, useCodebaseStore } from "./codebaseStore";
import { create } from "zustand";

const useStore = create(() => ({
  messageStore: createMessageStore(),
  codebaseStore: createCodebaseStore(),
}));

export { useMessageStore, useCodebaseStore };
export default useStore;
