import { create } from "zustand";

export type ModelKind = "codex" | "claude-code";

type ModelState = {
  selected: ModelKind;
  setSelected: (m: ModelKind) => void;
};

export const useModelStore = create<ModelState>((set) => ({
  selected: "codex",
  setSelected: (m) => set({ selected: m }),
}));
