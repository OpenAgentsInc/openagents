/** Limits local execution after the parent checks its trusted authoring service. */
export type Project = { id: string; branch: string; revision: number };
export type HostGrant = {
  token: string;
  action_sha256: string;
  session: string;
  release: string;
  project: Project;
  draft: Project;
  capability: string;
  expires_at: number;
};
export class AuthoringPermit {
  constructor(private readonly digest: (action: unknown) => string) {}
  #current: HostGrant | undefined;
  readonly #used = new Set<string>();
  authorize(
    grant: HostGrant,
    project: Project,
    release: string,
    at = Date.now(),
  ) {
    if (
      grant.release !== release ||
      grant.expires_at * 1000 <= at ||
      !["propose", "draft", "compile", "undo", "redo"].includes(
        grant.capability,
      ) ||
      grant.project.id !== project.id ||
      grant.project.branch !== project.branch ||
      grant.project.revision !== project.revision ||
      this.#used.has(grant.token)
    )
      throw new Error("Sign in and confirm this change again.");
    this.#current = grant;
  }
  revoke() {
    this.#current = undefined;
  }
  async consume(action: unknown, project: Project, at = Date.now()) {
    const grant = this.#current;
    this.#current = undefined;
    const digest = this.digest(action);
    if (
      !grant ||
      grant.expires_at * 1000 <= at ||
      grant.action_sha256 !== digest ||
      grant.capability !==
        ([
          "restore_project",
          "restore_material_project",
          "apply_material",
        ].includes((action as { type: string }).type)
          ? "propose"
          : (action as { type: string }).type) ||
      grant.project.id !== project.id ||
      grant.project.branch !== project.branch ||
      grant.project.revision !== project.revision
    )
      throw new Error("Sign in and confirm this change again.");
    this.#used.add(grant.token);
    if (this.#used.size > 64)
      this.#used.delete(this.#used.values().next().value!);
    return grant;
  }
}
