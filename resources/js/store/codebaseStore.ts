import { create, StateCreator } from "zustand";

interface Codebase {
  name: string;
  branch: string;
  isSelected: boolean;
}

interface CodebaseState {
  codebases: Codebase[];
  toggleCodebase: (name: string) => void;
  addCodebase: (name: string, branch: string) => void;
  removeCodebase: (name: string) => void;
}

const createCodebaseSlice: StateCreator<CodebaseState> = (set) => ({
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
  addCodebase: (name: string, branch: string) =>
    set((state) => ({
      codebases: [...state.codebases, { name, branch, isSelected: true }],
    })),
  removeCodebase: (name: string) =>
    set((state) => ({
      codebases: state.codebases.filter((codebase) => codebase.name !== name),
    })),
});

export const createCodebaseStore = () =>
  create<CodebaseState>(createCodebaseSlice);

export const useCodebaseStore = createCodebaseStore();
