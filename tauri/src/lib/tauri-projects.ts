/**
 * Type-safe wrappers for project-related WebSocket control messages
 */

import type { Project } from "./project-store";

/**
 * Create a new project
 */
export function createProject(
  ws: { send: (msg: object) => void },
  project: Omit<Project, "created_at" | "updated_at">
): void {
  ws.send({
    control: "tvx.create_project",
    project: {
      id: project.id,
      name: project.name,
      path: project.path,
      description: project.description || null,
      icon: project.icon || null,
      color: project.color || null,
      starred: project.starred,
    },
  });
}

/**
 * Update an existing project
 */
export function updateProject(
  ws: { send: (msg: object) => void },
  projectId: string,
  updates: Partial<Omit<Project, "id" | "created_at" | "updated_at">>
): void {
  ws.send({
    control: "tvx.update_project",
    projectId,
    updates,
  });
}

/**
 * Delete (archive) a project
 */
export function deleteProject(
  ws: { send: (msg: object) => void },
  projectId: string
): void {
  ws.send({
    control: "tvx.delete_project",
    projectId,
  });
}

/**
 * Query projects list
 */
export function listProjects(ws: { send: (msg: object) => void }): void {
  ws.send({
    control: "tvx.query",
    name: "projects.list",
    args: {},
  });
}

/**
 * Subscribe to projects stream
 */
export function subscribeProjects(ws: { send: (msg: object) => void }): void {
  ws.send({
    control: "tvx.subscribe",
    stream: "projects",
  });
}
