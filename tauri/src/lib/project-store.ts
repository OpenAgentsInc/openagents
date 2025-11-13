import { create } from "zustand";

export type Project = {
  id: string;
  name: string;
  path: string;
  description?: string | null;
  icon?: string | null;
  color?: string | null;
  starred: number; // 0 or 1 (SQLite boolean)
  archived: number; // 0 or 1 (SQLite boolean)
  created_at: number;
  updated_at: number;
};

type ProjectState = {
  // List of all projects
  projects: Project[];

  // Currently active project ID (null if none)
  activeProjectId: string | null;

  // Set all projects (from initial load)
  setProjects: (projects: Project[]) => void;

  // Set active project
  setActiveProject: (projectId: string | null) => void;

  // Get project by ID
  getProject: (projectId: string) => Project | undefined;

  // Get active project
  getActiveProject: () => Project | undefined;

  // Add or update a project (from WebSocket update)
  upsertProject: (project: Project) => void;

  // Remove a project from the list (when archived)
  removeProject: (projectId: string) => void;

  // Get starred projects
  getStarredProjects: () => Project[];

  // Get projects sorted by update time
  getSortedProjects: () => Project[];
};

export const useProjectStore = create<ProjectState>((set, get) => ({
  projects: [],
  activeProjectId: null,

  setProjects: (projects: Project[]) => {
    set({ projects });
  },

  setActiveProject: (projectId: string | null) => {
    set({ activeProjectId: projectId });
  },

  getProject: (projectId: string) => {
    return get().projects.find((p) => p.id === projectId);
  },

  getActiveProject: () => {
    const { activeProjectId, projects } = get();
    if (!activeProjectId) return undefined;
    return projects.find((p) => p.id === activeProjectId);
  },

  upsertProject: (project: Project) => {
    set((state) => {
      const existingIndex = state.projects.findIndex((p) => p.id === project.id);
      if (existingIndex >= 0) {
        // Update existing project
        const newProjects = [...state.projects];
        newProjects[existingIndex] = project;
        return { projects: newProjects };
      } else {
        // Add new project
        return { projects: [...state.projects, project] };
      }
    });
  },

  removeProject: (projectId: string) => {
    set((state) => ({
      projects: state.projects.filter((p) => p.id !== projectId),
      // Clear active project if it was deleted
      activeProjectId: state.activeProjectId === projectId ? null : state.activeProjectId,
    }));
  },

  getStarredProjects: () => {
    return get().projects.filter((p) => p.starred === 1 && p.archived === 0);
  },

  getSortedProjects: () => {
    return [...get().projects]
      .filter((p) => p.archived === 0)
      .sort((a, b) => {
        // Starred projects first
        if (a.starred !== b.starred) {
          return b.starred - a.starred;
        }
        // Then by updated_at
        return b.updated_at - a.updated_at;
      });
  },
}));
