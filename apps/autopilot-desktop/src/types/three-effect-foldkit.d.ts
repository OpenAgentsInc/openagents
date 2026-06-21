import type { Attribute, Html } from "foldkit/html"
import type {
  TrainingRunLocalPoseUpdate,
  TrainingRunNodeSelection,
  TrainingRunPresenceZone,
  TrainingRunVisualizationOptions,
  TrainingRunWorldItemSelection,
} from "./three-effect-core.js"

export const trainingRunView: <Message>(
  attributes?: ReadonlyArray<Attribute<Message>>,
  visualization?: TrainingRunVisualizationOptions,
  onNodeSelected?: (node: TrainingRunNodeSelection) => Message,
  onPresenceZoneChanged?: (zone: TrainingRunPresenceZone | null) => Message,
  onLocalPoseChanged?: (pose: TrainingRunLocalPoseUpdate) => Message,
  onWorldItemProximityChanged?: (
    item: TrainingRunWorldItemSelection | null,
  ) => Message,
) => Html
