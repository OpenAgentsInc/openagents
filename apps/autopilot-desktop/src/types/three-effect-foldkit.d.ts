import type { Attribute, Html } from "foldkit/html"
import type {
  TrainingRunLocalPoseUpdate,
  TrainingRunNodeSelection,
  TrainingRunPresenceZone,
  TrainingRunVisualizationOptions,
} from "./three-effect-core.js"

export const trainingRunView: <Message>(
  attributes?: ReadonlyArray<Attribute<Message>>,
  visualization?: TrainingRunVisualizationOptions,
  onNodeSelected?: (node: TrainingRunNodeSelection) => Message,
  onPresenceZoneChanged?: (zone: TrainingRunPresenceZone | null) => Message,
  onLocalPoseChanged?: (pose: TrainingRunLocalPoseUpdate) => Message,
) => Html
