import { create } from "zustand";

interface HotbarState {
  pressedSlots: number[];
}

interface HotbarStore extends HotbarState {
  setPressedSlot: (slot: number, pressed: boolean) => void;
}

export const useHotbarStore = create<HotbarStore>((set) => ({
  pressedSlots: [],
  
  setPressedSlot: (slot: number, pressed: boolean) => {
    set((state) => ({
      pressedSlots: pressed
        ? [...state.pressedSlots, slot]
        : state.pressedSlots.filter((s) => s !== slot),
    }));
  },
}));