import {
  type DesktopCodingCatalog,
  type DesktopCodingCatalogSnapshot,
  type DesktopWorkspaceAdmission,
} from "./desktop-coding-catalog.ts"
import {
  openWorkspaceService,
  type DesktopWorkspaceService,
} from "./workspace-service.ts"

export type AdmittedDesktopWorkspace = Readonly<{
  admission: DesktopWorkspaceAdmission
  catalog: DesktopCodingCatalogSnapshot
  workspace: DesktopWorkspaceService
}>

/**
 * Installs the picker-selected workspace under the exact grant admitted into
 * the durable coding catalog. Callers never derive authority from the root;
 * the root only resolves the host-private filesystem service.
 */
export const openAdmittedDesktopWorkspace = (
  catalog: DesktopCodingCatalog,
  selectedRoot: string,
  open: (root: string, grantRef: string) => DesktopWorkspaceService =
    (root, grantRef) => openWorkspaceService(root, { grantRef }),
): AdmittedDesktopWorkspace => {
  const admitted = catalog.admitWorkspace(selectedRoot)
  const workspace = open(selectedRoot, admitted.admission.grantRef)
  if (workspace.grantRef !== admitted.admission.grantRef) {
    workspace.dispose()
    throw new Error("workspace service did not retain the admitted grant")
  }
  return {
    admission: admitted.admission,
    catalog: admitted.snapshot,
    workspace,
  }
}
