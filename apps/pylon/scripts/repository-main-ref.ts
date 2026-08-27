const GIT_SHA = /^[0-9a-f]{40}$/u;

export type GitCommand = (...args: ReadonlyArray<string>) => Promise<string>;

/** Resolve the repository's main branch without assuming a remote name. */
export const resolveRepositoryMainRef = async (git: GitCommand): Promise<string> => {
  const candidates = [
    "refs/remotes/openagents/main",
    "refs/remotes/origin/main",
    "refs/heads/main",
  ] as const;

  for (const candidate of candidates) {
    try {
      const commit = await git("rev-parse", "--verify", `${candidate}^{commit}`);
      if (GIT_SHA.test(commit)) return candidate;
    } catch {
      // Try the next admitted main ref. A clone can name the forge remote
      // `openagents` or `origin`, while a local worktree can carry only `main`.
    }
  }

  throw new Error(
    "repository main is unavailable; expected openagents/main, origin/main, or local main",
  );
};
