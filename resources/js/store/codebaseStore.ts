import { create, StateCreator } from "zustand";

interface Codebase {
  id: string;
  name: string;
  branch: string;
  isSelected: boolean;
  status?: string;
}

interface CodebaseState {
  codebases: Codebase[];
  toggleCodebase: (id: string) => void;
  addCodebase: (name: string, branch: string, id: string) => void;
  removeCodebase: (id: string) => void;
  updateCodebaseStatus: (id: string, status: any) => void;
}

const createCodebaseSlice: StateCreator<CodebaseState> = (set) => ({
  codebases: [
    {
      id: "github:v2:openagentsinc/openagents",
      name: "openagentsinc/openagents",
      branch: "v2",
      isSelected: true,
      status: "Indexed",
    },
  ],
  toggleCodebase: (id) =>
    set((state) => ({
      codebases: state.codebases.map((codebase) =>
        codebase.id === id
          ? { ...codebase, isSelected: !codebase.isSelected }
          : codebase,
      ),
    })),
  addCodebase: (name, branch, id) =>
    set((state) => ({
      codebases: [
        ...state.codebases,
        { id, name, branch, isSelected: true, status: "Indexing" },
      ],
    })),
  removeCodebase: (id) =>
    set((state) => ({
      codebases: state.codebases.filter((codebase) => codebase.id !== id),
    })),
  updateCodebaseStatus: (id, status) =>
    set((state) => ({
      codebases: state.codebases.map((codebase) =>
        codebase.id === id ? { ...codebase, status: status.status } : codebase,
      ),
    })),
});

export const createCodebaseStore = () =>
  create<CodebaseState>(createCodebaseSlice);

export const useCodebaseStore = createCodebaseStore();
