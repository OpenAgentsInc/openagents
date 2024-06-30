import { createMessageStore, useMessageStore } from "./messageStore";
import { create } from "zustand";

const useStore = create(() => ({
  messageStore: createMessageStore(),
}));

export { useMessageStore };
export default useStore;
