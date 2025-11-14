/**
 * Example component demonstrating Convex integration for projects
 * This shows how to use Convex queries and mutations in the Tauri app
 */

import { useQuery, useMutation } from "convex/react";
import { api } from "../../../convex/_generated/api";
import type { Id } from "../../../convex/_generated/dataModel";

export function ConvexProjectsList() {
  // Query projects from Convex
  const projects = useQuery(api.projects.listProjects, { includeArchived: false });
  const starredProjects = useQuery(api.projects.listStarredProjects);

  // Mutations for project management
  const createProject = useMutation(api.projects.createProject);
  const updateProject = useMutation(api.projects.updateProject);
  const deleteProject = useMutation(api.projects.deleteProject);
  const toggleStarred = useMutation(api.projects.toggleStarred);

  const handleCreateProject = async () => {
    await createProject({
      name: "New Project",
      path: "/path/to/project",
      description: "Example project created via Convex",
    });
  };

  const handleToggleStar = async (projectId: Id<"projects">) => {
    await toggleStarred({ projectId });
  };

  const handleDeleteProject = async (projectId: Id<"projects">) => {
    if (confirm("Are you sure you want to delete this project?")) {
      await deleteProject({ projectId });
    }
  };

  if (projects === undefined) {
    return <div>Loading projects...</div>;
  }

  return (
    <div className="p-4">
      <h2 className="text-xl font-bold mb-4">Convex Projects (Example)</h2>

      <button
        onClick={handleCreateProject}
        className="mb-4 px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700"
      >
        Create Test Project
      </button>

      <div className="space-y-4">
        <div>
          <h3 className="text-lg font-semibold mb-2">All Projects ({projects.length})</h3>
          {projects.length === 0 ? (
            <p className="text-gray-400">No projects yet. Click "Create Test Project" above.</p>
          ) : (
            <ul className="space-y-2">
              {projects.map((project) => (
                <li key={project._id} className="flex items-center justify-between p-3 bg-zinc-800 rounded">
                  <div>
                    <div className="font-medium">{project.name}</div>
                    <div className="text-sm text-gray-400">{project.path}</div>
                    {project.description && (
                      <div className="text-sm text-gray-500">{project.description}</div>
                    )}
                  </div>
                  <div className="flex gap-2">
                    <button
                      onClick={() => handleToggleStar(project._id)}
                      className={`px-3 py-1 rounded ${
                        project.starred ? "bg-yellow-600" : "bg-gray-600"
                      } hover:opacity-80`}
                    >
                      {project.starred ? "★" : "☆"}
                    </button>
                    <button
                      onClick={() => handleDeleteProject(project._id)}
                      className="px-3 py-1 bg-red-600 rounded hover:bg-red-700"
                    >
                      Delete
                    </button>
                  </div>
                </li>
              ))}
            </ul>
          )}
        </div>

        {starredProjects && starredProjects.length > 0 && (
          <div>
            <h3 className="text-lg font-semibold mb-2">
              Starred Projects ({starredProjects.length})
            </h3>
            <ul className="space-y-2">
              {starredProjects.map((project) => (
                <li key={project._id} className="p-3 bg-zinc-800 rounded">
                  <div className="font-medium">★ {project.name}</div>
                  <div className="text-sm text-gray-400">{project.path}</div>
                </li>
              ))}
            </ul>
          </div>
        )}
      </div>
    </div>
  );
}
