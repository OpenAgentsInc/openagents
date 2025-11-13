import { create } from "zustand";

type Route =
  | { kind: "chat" }
  | { kind: "project"; projectId: string };

type UiState = {
  route: Route;
  setProjectView: (projectId: string | null) => void;
  clearProjectView: () => void;
};

export const useUiStore = create<UiState>((set) => ({
  route: { kind: "chat" },
  setProjectView: (projectId: string | null) =>
    set(() => (projectId ? { route: { kind: "project", projectId } } : { route: { kind: "chat" } })),
  clearProjectView: () => set({ route: { kind: "chat" } }),
}));

