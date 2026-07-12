const repositorySelectionVariables = new Set([
  "GIT_ALTERNATE_OBJECT_DIRECTORIES",
  "GIT_COMMON_DIR",
  "GIT_DIR",
  "GIT_GRAFT_FILE",
  "GIT_INDEX_FILE",
  "GIT_INTERNAL_SUPER_PREFIX",
  "GIT_NO_REPLACE_OBJECTS",
  "GIT_OBJECT_DIRECTORY",
  "GIT_PREFIX",
  "GIT_REPLACE_REF_BASE",
  "GIT_SHALLOW_FILE",
  "GIT_WORK_TREE",
])

/**
 * Git hooks export repository-selection variables to child processes. Any Git
 * command scoped to an explicit workspace must drop those variables or `cwd`
 * and `-C` can be silently ignored in favor of the hook's real repository.
 */
export function workspaceGitEnvironment(
  environment: NodeJS.ProcessEnv = process.env,
): NodeJS.ProcessEnv {
  return Object.fromEntries(
    Object.entries(environment).filter(([key]) => !repositorySelectionVariables.has(key)),
  )
}
