import { create } from "zustand";

export type ModelKind = "ollama" | "codex";

type ModelState = {
  selected: ModelKind;
  setSelected: (m: ModelKind) => void;
};

export const useModelStore = create<ModelState>((set) => ({
  selected: "ollama",
  setSelected: (m) => set({ selected: m }),
}));

