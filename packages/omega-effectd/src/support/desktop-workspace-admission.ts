/**
 * One durable identity admission for a user-selected repository checkout.
 *
 * The raw root is deliberately absent. It remains only in the private binding
 * file, while every product/runtime surface shares these opaque refs.
 */
export type DesktopWorkspaceAdmission = Readonly<{
  grantRef: string
  projectRef: string
  repositoryRef: string
  worktreeRef: string
  workContextRef: string
  sessionRef: string
}>
