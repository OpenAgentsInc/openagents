import { createMessageStore } from "./messageStore";
import { StateCreator, create } from "zustand";

// Define the shape of our entire store
interface StoreState {
  messageStore: ReturnType<typeof createMessageStore>;
  // Add other store slices here as needed
}

// Create the main store
export const useStore = create<StoreState>((set, get) => ({
  messageStore: createMessageStore(),
  // Initialize other store slices here
}));

// Export convenience hooks for each slice
export const useMessageStore = () => useStore((state) => state.messageStore);
