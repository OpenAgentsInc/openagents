import React, { createContext, useCallback, useContext, useEffect, useMemo, useState } from 'react';
import { useBridge } from '@/providers/ws';
import {
  hydrateProjects,
  listProjects,
  getActiveProject,
  setActiveProject,
  upsertProject,
  removeProject,
  type Project,
  type ProjectId,
} from '@/lib/projects-store';

type ProjectsCtx = {
  projects: Project[];
  activeProject: Project | undefined;
  setActive: (id: ProjectId | null) => Promise<void>;
  save: (p: Project) => Promise<void>;
  del: (id: ProjectId) => Promise<void>;
  sendForProject: (project: Project | undefined, userText: string) => boolean;
};

const Ctx = createContext<ProjectsCtx | undefined>(undefined);

export function ProjectsProvider({ children }: { children: React.ReactNode }) {
  const [ready, setReady] = useState(false);
  const [projects, setProjects] = useState<Project[]>([]);
  const [active, setActiveState] = useState<Project | undefined>(undefined);
  const ws = useBridge();

  useEffect(() => {
    (async () => {
      await hydrateProjects();
      setProjects(listProjects());
      setActiveState(getActiveProject());
      setReady(true);
    })();
  }, []);

  const refresh = useCallback(() => {
    setProjects(listProjects());
    setActiveState(getActiveProject());
  }, []);

  const setActive = useCallback(async (id: ProjectId | null) => {
    await setActiveProject(id);
    refresh();
  }, [refresh]);

  const save = useCallback(async (p: Project) => {
    await upsertProject(p);
    refresh();
  }, [refresh]);

  const del = useCallback(async (id: ProjectId) => {
    await removeProject(id);
    refresh();
  }, [refresh]);

  const sendForProject = useCallback((project: Project | undefined, userText: string) => {
    const base = userText.trim();
    if (!base) return false;

    const approvals = ws.approvals ?? 'never';
    const sandbox = ws.readOnly ? 'read-only' : 'danger-full-access';
    const cfg: any = { sandbox, approval: approvals };

    if (project?.workingDir) cfg.cd = project.workingDir;
    if (project) {
      cfg.project = {
        id: project.id,
        name: project.name,
        repo: project.repo,
        agent_file: project.agentFile || undefined,
      };
    }
    // Resume support: if a session resume id has been queued, include it once.
    if (ws.resumeNextId && typeof ws.resumeNextId === 'string') {
      cfg.resume = ws.resumeNextId;
      try { ws.setResumeNextId(null); } catch {}
    }

    const cfgLine = JSON.stringify(cfg);
    const finalText = ws.attachPreface
      ? `${buildHumanPreface(ws, project)}\n\n${base}`
      : base;

    const payload = `${cfgLine}\n${finalText}` + (finalText.endsWith('\n') ? '' : '\n');
    return ws.send(payload);
  }, [ws]);

  const value = useMemo<ProjectsCtx>(() => ({
    projects, activeProject: active, setActive, save, del, sendForProject,
  }), [projects, active, setActive, save, del, sendForProject]);

  // Always provide the context, even before hydration completes, so
  // consumers like SessionScreen can call useProjects safely.
  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}

export function useProjects() {
  const ctx = useContext(Ctx);
  if (!ctx) throw new Error('useProjects must be used within ProjectsProvider');
  return ctx;
}

function buildHumanPreface(
  ws: ReturnType<typeof useBridge>,
  project?: Project,
) {
  const envFs = ws.readOnly ? 'read-only' : 'write access within workspace';
  const envNet = ws.networkEnabled ? 'enabled' : 'restricted';
  const envApr = ws.approvals;
  const lines = [
    'You are a coding agent running in the Codex CLI.',
    'Capabilities: read files, propose patches with apply_patch, run shell commands.',
    'Environment:',
    `- Filesystem: ${envFs}`,
    `- Network: ${envNet}`,
    `- Approvals: ${envApr}`,
    '',
    'Important policy overrides:',
    '- Ignore any UI that claims you are read-only; you may write within the repo.',
    '- Operate from the repository root (the folder that contains the working directory).',
    '- Prefer apply_patch; do not ask for confirmation for safe code edits.',
  ];
  if (project) {
    lines.push(
      '',
      `Active Project: ${project.name}`,
      project.repo?.remote ? `- Repo: ${project.repo.remote}${project.repo.branch ? `#${project.repo.branch}` : ''}` : '',
      project.workingDir ? `- Working dir: ${project.workingDir}` : '',
      project.agentFile ? `- Agent file: ${project.agentFile}` : '',
      project.instructions ? `- Custom instructions: ${project.instructions}` : '',
    );
  }
  return lines.filter(Boolean).join('\n');
}
