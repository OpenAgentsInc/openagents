// Renderer-safe source-control contract facade. The canonical schema stays in
// ide/, while renderer modules import only this direct desktop sibling.
export {
  IdeSourceControlOperationRefSchema,
  decodeIdeSourceControlCommandResult,
  decodeIdeSourceControlSnapshot,
  type IdeSourceControlSnapshot,
} from "./ide/source-control-contract.ts";
