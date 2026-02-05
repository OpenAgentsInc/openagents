type ProjectDetailsProps = {
  projectId?: string | null;
};

/**
 * Minimal "Project" badge for overlay parity. Renders nothing when projectId is not set.
 */
export function ProjectDetails({ projectId }: ProjectDetailsProps) {
  if (projectId == null || projectId === '') {
    return null;
  }

  return (
    <div className="pointer-events-auto absolute left-4 top-4 rounded-md border border-border bg-card px-3 py-1.5 shadow-sm">
      <span className="text-xs font-medium text-muted-foreground">Project</span>
      <span className="ml-2 text-xs text-card-foreground" title={projectId}>
        {projectId}
      </span>
    </div>
  );
}
