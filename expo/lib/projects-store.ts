import AsyncStorage from '@react-native-async-storage/async-storage';

export type ProjectId = string;

export type ProjectRepo = {
  provider?: 'github' | 'gitlab' | 'other';
  remote?: string;  // "owner/name"
  url?: string;     // e.g. https://github.com/owner/name
  branch?: string;
};

export type ProjectTodo = { text: string; completed: boolean };

export type Project = {
  id: ProjectId;
  name: string;
  voiceAliases: string[];
  workingDir: string;
  repo?: ProjectRepo;
  agentFile?: string;
  instructions?: string;

  runningAgents?: number;
  attentionCount?: number;
  todos?: ProjectTodo[];

  lastActivity?: number;
  createdAt: number;
  updatedAt: number;

  approvals?: 'never' | 'on-request' | 'on-failure';
  model?: string;
  sandbox?: 'danger-full-access' | 'workspace-write' | 'read-only';
};

const KEY = '@openagents/projects-v1';
const ACTIVE_KEY = '@openagents/projects-active-v1';

let mem: Record<ProjectId, Project> = {};
let activeId: ProjectId | null = null;

export async function hydrateProjects(): Promise<void> {
  try {
    const raw = await AsyncStorage.getItem(KEY);
    mem = raw ? JSON.parse(raw) : {};
    activeId = (await AsyncStorage.getItem(ACTIVE_KEY)) as ProjectId | null;
  } catch {
    mem = {};
    activeId = null;
  }
}

async function persist() {
  try { await AsyncStorage.setItem(KEY, JSON.stringify(mem)); } catch {}
}

async function persistActive() {
  try {
    if (activeId) await AsyncStorage.setItem(ACTIVE_KEY, activeId);
    else await AsyncStorage.removeItem(ACTIVE_KEY);
  } catch {}
}

export function listProjects(): Project[] {
  return Object.values(mem).sort((a, b) => a.name.localeCompare(b.name));
}

export function getProject(id: ProjectId): Project | undefined {
  return mem[id];
}

export function getActiveProject(): Project | undefined {
  return activeId ? mem[activeId] : undefined;
}

export async function setActiveProject(id: ProjectId | null): Promise<void> {
  activeId = id;
  await persistActive();
}

export async function upsertProject(p: Project): Promise<void> {
  mem[p.id] = { ...p, updatedAt: Date.now(), createdAt: p.createdAt ?? Date.now() };
  await persist();
}

export async function removeProject(id: ProjectId): Promise<void> {
  delete mem[id];
  if (activeId === id) { activeId = null; await persistActive(); }
  await persist();
}

export async function mergeProjectTodos(projectId: ProjectId, todos: ProjectTodo[]): Promise<void> {
  const p = mem[projectId];
  if (!p) return;
  p.todos = todos;
  p.attentionCount = todos.filter(t => !t.completed).length;
  p.updatedAt = Date.now();
  await persist();
}

