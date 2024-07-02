import { create, StateCreator } from "zustand";
import { v4 as uuidv4 } from "uuid";

interface Codebase {
  id: string;
  name: string;
  branch: string;
  isSelected: boolean;
}

interface CodebaseState {
  codebases: Codebase[];
  toggleCodebase: (id: string) => void;
  addCodebase: (name: string, branch: string) => void;
  removeCodebase: (id: string) => void;
}

const createCodebaseSlice: StateCreator<CodebaseState> = (set) => ({
  codebases: [
    {
      id: uuidv4(),
      name: "openagentsinc/openagents",
      branch: "main",
      isSelected: true,
    },
  ],
  toggleCodebase: (id: string) =>
    set((state) => ({
      codebases: state.codebases.map((codebase) =>
        codebase.id === id
          ? { ...codebase, isSelected: !codebase.isSelected }
          : codebase,
      ),
    })),
  addCodebase: (name: string, branch: string) =>
    set((state) => ({
      codebases: [
        ...state.codebases,
        { id: uuidv4(), name, branch, isSelected: true },
      ],
    })),
  removeCodebase: (id: string) =>
    set((state) => ({
      codebases: state.codebases.filter((codebase) => codebase.id !== id),
    })),
});

export const createCodebaseStore = () =>
  create<CodebaseState>(createCodebaseSlice);

export const useCodebaseStore = createCodebaseStore();
