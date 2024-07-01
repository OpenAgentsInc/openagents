import { create } from "zustand";

interface Codebase {
  name: string;
  branch: string;
  isSelected: boolean;
}

interface CodebaseState {
  codebases: Codebase[];
  toggleCodebase: (name: string) => void;
}

export const useCodebaseStore = create<CodebaseState>((set) => ({
  codebases: [
    { name: "openagentsinc/openagents", branch: "v2", isSelected: true },
  ],
  toggleCodebase: (name: string) =>
    set((state) => ({
      codebases: state.codebases.map((codebase) =>
        codebase.name === name
          ? { ...codebase, isSelected: !codebase.isSelected }
          : codebase
      ),
    })),
}));
