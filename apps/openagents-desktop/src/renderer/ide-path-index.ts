/**
 * Named renderer integration boundary for the IDE path index. Renderer
 * modules import this sibling facade; the underlying Schema/Effect authority
 * stays in `src/ide` and Pierre still receives only its bounded projection.
 */
export {
  IdeAttachmentGenerationSchema,
  IdeAttachmentRefSchema,
  IdePathIndexGenerationSchema,
  IdeProjectRefSchema,
  IdeRootRefSchema,
  IdeWorktreeRefSchema,
} from "../ide/project-contract.ts"

export {
  IdeExplorerCommandSchema,
  IdePathIndexIdentitySchema,
  IdePathOperationRefSchema,
  IdePathScanRefSchema,
  type IdeExplorerCommand,
  type IdePathIndexIdentity,
  type IdePathIndexInteractionUpdate,
  type IdePathIndexOperationUpdate,
  type IdePathIndexReconcileRequest,
  type IdePathIndexScanRequest,
  type IdePathIndexSnapshot,
  type IdePierreTreeProjection,
} from "../ide/path-index-contract.ts"

export {
  IdePathIndexService,
  emptyIdePathIndexSnapshot,
  makeIdePathIndexLayer,
  type IdePathIndexSource,
} from "../ide/path-index-service.ts"
