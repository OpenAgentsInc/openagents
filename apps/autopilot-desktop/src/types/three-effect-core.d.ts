import type { Effect } from "effect"
import type * as Three from "three"

export type TrainingRunVector = readonly [number, number, number]

export type TrainingRunNodeRole =
  | "lifecycle"
  | "run"
  | "proof"
  | "receipt"
  | "rung"

export type TrainingRunNodeStatus =
  | "planned"
  | "queued"
  | "sync"
  | "active"
  | "sealed"
  | "verified"
  | "blocked"

export type TrainingRunNodeDefinition = Readonly<{
  id: string
  label: string
  detail: string
  role: TrainingRunNodeRole
  status: TrainingRunNodeStatus
  position: TrainingRunVector
  connectedTo?: readonly string[]
}>

export type TrainingRunNodeSelection = Pick<
  TrainingRunNodeDefinition,
  "detail" | "id" | "label" | "role" | "status"
>

export type TrainingRunOperatorSignalState =
  | "error"
  | "idle"
  | "info"
  | "success"

export type TrainingRunOperatorSignalDefinition = Readonly<{
  id: string
  label: string
  state: TrainingRunOperatorSignalState
  detail: string
}>

export type TrainingRunPromiseSignalState =
  | "degraded"
  | "green"
  | "planned"
  | "red"
  | "unknown"
  | "withdrawn"
  | "yellow"

export type TrainingRunPromiseSignalDefinition = Readonly<{
  blockerCount: number
  evidenceRefCount: number
  id: string
  label: string
  state: TrainingRunPromiseSignalState
}>

export type TrainingRunEntityDefinition = Readonly<{
  detail?: string
  id: string
  label?: string
  position?: TrainingRunVector
  status: string
}>

export type VerseIconKind =
  | "agent"
  | "chat"
  | "focus"
  | "inspect"
  | "proof"
  | "pylon"
  | "receipt"
  | "run"
  | "settlement"
  | "training"
  | "zap"

export type VerseIconPrimitive =
  | "bolt"
  | "brackets"
  | "bubble"
  | "chevron"
  | "core"
  | "diamond"
  | "eye"
  | "hex"
  | "node"
  | "orbit"
  | "ring"
  | "spark"
  | "stack"
  | "trace"
  | "triangle"

export type VerseIconRecipe = Readonly<{
  background: "grid" | "halo" | "radial" | "scanline" | "void"
  fallback: boolean
  id: string
  kind: VerseIconKind | "unknown"
  palette:
    | "agent"
    | "chat"
    | "focus"
    | "gold"
    | "pylon"
    | "proof"
    | "run"
    | "settlement"
    | "training"
    | "zap"
  primitives: ReadonlyArray<VerseIconPrimitive>
  seed: number
}>

export declare const verseIconRecipeForId: (id: string) => VerseIconRecipe

export type TrainingRunMotionKind =
  | "presence"
  | "assignment"
  | "trace_submitted"
  | "replay_verified"
  | "replay_rejected"
  | "settlement_recorded"
  | "real_bitcoin_moved"
  | "corpus_accepted"
  | "counter_changed"
  | (string & {})

export type TrainingRunMotionEvidence = Readonly<{
  expiresAt?: string
  generatedAt?: string
  motionId?: string
  motionKind?: TrainingRunMotionKind
  simulated?: boolean
  sourceRefs?: readonly string[]
}>

export type TrainingRunBeamDefinition = Readonly<
  TrainingRunMotionEvidence & { fromId: string; toId: string }
>

export type TrainingRunBurstDefinition = Readonly<
  TrainingRunMotionEvidence & { atId: string }
>

export type TrainingRunRemoteAvatarDefinition = Readonly<{
  actorRef?: string
  animation?: "idle" | "run" | "walk"
  avatarKind?: string
  color?: string
  id: string
  label: string
  labelVisibility?: "auto" | "hidden" | "visible"
  modelUrl?: string
  position: TrainingRunVector
  stale?: boolean
  updatedAtMs?: number
  yaw?: number
}>

export type TrainingRunWorldItemKind = "bulletin_board"

export type TrainingRunWorldItemDefinition = Readonly<{
  detail: string
  id: string
  interactionRadius?: number
  kind: TrainingRunWorldItemKind
  label: string
  lines?: readonly string[]
  position: TrainingRunVector
  sourceRefs?: readonly string[]
  status?: TrainingRunNodeStatus
  title?: string
  yaw?: number
}>

export type TrainingRunWorldItemSelection = Readonly<{
  detail: string
  id: string
  kind: TrainingRunWorldItemKind
  label: string
  sourceRefs: readonly string[]
  status: TrainingRunNodeStatus
}>

export type TrainingRunPresenceZone = "tassadar_area"

export type TrainingRunLocalPoseSnapshot = Readonly<{
  action?: string
  capturedAtMs?: number
  controller: "third_person_character" | "wasd_mouselook"
  position: TrainingRunVector
  yaw?: number
}>

export type TrainingRunLocalPoseUpdate = Readonly<{
  action: string
  capturedAtMs: number
  controller: "third_person_character" | "wasd_mouselook"
  position: TrainingRunVector
  yaw: number
}>

export type WasdAction =
  | "backward"
  | "fall"
  | "forward"
  | "left"
  | "right"
  | "rise"
  | "sprint"

export type WasdKeyboardBindingMap = Readonly<
  Partial<Record<WasdAction, readonly string[]>>
>

export type TrainingRunKeyboardTargetingAction = "next" | "previous"

export type TrainingRunKeyboardTargetingBinding = Readonly<{
  altKey?: boolean
  code?: string
  ctrlKey?: boolean
  key?: string
  metaKey?: boolean
  shiftKey?: boolean
}>

export type TrainingRunKeyboardTargeting = Readonly<{
  bindings?: Readonly<
    Partial<
      Record<
        TrainingRunKeyboardTargetingAction,
        readonly TrainingRunKeyboardTargetingBinding[]
      >
    >
  >
  enabled?: boolean
  maxTargets?: number
}>

export type WasdMouseLookControllerOptions = Readonly<{
  acceleration?: number
  bounds?: unknown
  damping?: number
  debug?: boolean | ((snapshot: unknown) => void)
  enabled?: boolean
  eyeHeight?: number
  groundHeightAt?: (x: number, z: number) => number
  initialPosition?: TrainingRunVector
  inputTarget?: HTMLElement | Window
  keyboardBindings?: WasdKeyboardBindingMap
  lockSelector?: string
  movementSpeed?: number
  onLockChange?: (locked: boolean) => void
  pitchMax?: number
  pitchMin?: number
  pointerSensitivity?: number
  sprintMultiplier?: number
}>

export type ThreePlayerControllerOptions = Readonly<{
  camera?: Readonly<Record<string, unknown>>
  character?: Readonly<Record<string, unknown>>
  dragSensitivity?: number
  enabled?: boolean
  gravity?: number
  groundHeightAt?: (x: number, z: number) => number
  initialPosition?: TrainingRunVector
  inputTarget?: HTMLElement | Window
  jumpHeight?: number
  keyboardBindings?: WasdKeyboardBindingMap
  onActionChange?: (action: string) => void
  onCameraControl?: (event: Readonly<Record<string, unknown>>) => void
}>

export type TrainingRunVisualizationOptions = Readonly<{
  backgroundColor?: number
  beams?: readonly TrainingRunBeamDefinition[]
  bursts?: readonly TrainingRunBurstDefinition[]
  cameraMode?: "orthographic_map" | "perspective_walk"
  controller?: "none" | "third_person_character" | "wasd_mouselook"
  contributors?: readonly unknown[]
  entities?: readonly TrainingRunEntityDefinition[]
  keyboardTargeting?: TrainingRunKeyboardTargeting
  lossCurve?: readonly unknown[]
  maxAllowedStaleSteps?: number
  motionPolicy?: Readonly<{
    ambient?: "static" | "animated"
    bursts?: "once" | "loop"
    evidence?: "optional" | "required"
    structuralEdges?: "static" | "animated"
  }>
  nodes?: readonly TrainingRunNodeDefinition[]
  onLocalPoseChange?: (pose: TrainingRunLocalPoseUpdate) => void
  onNodeClick?: (node: TrainingRunNodeSelection) => void
  onPresenceZoneChange?: (zone: TrainingRunPresenceZone | null) => void
  onWorldItemProximityChange?: (
    item: TrainingRunWorldItemSelection | null,
  ) => void
  operatorSignals?: readonly TrainingRunOperatorSignalDefinition[]
  pixelRatio?: number
  promiseSignals?: readonly TrainingRunPromiseSignalDefinition[]
  pulseSpeed?: number
  remoteAvatarInterpolation?: unknown
  remoteAvatars?: readonly TrainingRunRemoteAvatarDefinition[]
  sceneChrome?: Readonly<{
    contributorOrbit?: "visible" | "hidden"
    lossPanel?: "visible" | "hidden" | "auto"
    staleRing?: "visible" | "hidden"
    statusChart?: "visible" | "hidden"
  }>
  stageNodeGlyph?: "orb" | "compact_gate"
  thirdPersonController?: ThreePlayerControllerOptions
  walkController?: WasdMouseLookControllerOptions
  worldItems?: readonly TrainingRunWorldItemDefinition[]
  worldLabelDensity?: "full" | "compact" | "pylons"
}>

export type TrainingRunVisualizationSnapshot = Readonly<Record<string, unknown>>

export type TrainingRunVisualizationHandle = Readonly<{
  canvas: HTMLCanvasElement
  captureLocalPose: () => TrainingRunLocalPoseSnapshot | undefined
  dispose: Effect.Effect<void>
  element: HTMLElement
  resize: Effect.Effect<void>
  selectNextTarget: (direction?: 1 | -1) => TrainingRunNodeSelection | undefined
  updateVisualization: (options: TrainingRunVisualizationOptions) => boolean
  updateRemoteAvatars: (
    avatars: readonly TrainingRunRemoteAvatarDefinition[],
  ) => void
}>

export const defaultTrainingRunNodes: readonly TrainingRunNodeDefinition[]

export const trainingRunVisualizationOptionsFromSnapshot: (
  snapshot?: TrainingRunVisualizationSnapshot,
) => TrainingRunVisualizationOptions

export const trainingRunVisualizationOptionsWithLocalPose: (
  options: TrainingRunVisualizationOptions,
  pose: TrainingRunLocalPoseSnapshot,
) => TrainingRunVisualizationOptions

export const trainingRunVisualizationRetainedStructuralSignature: (
  options?: TrainingRunVisualizationOptions,
) => string

export const canRetainTrainingRunVisualization: (
  current: TrainingRunVisualizationOptions,
  next: TrainingRunVisualizationOptions,
) => boolean

export const mountTrainingRunVisualization: (
  element: HTMLElement,
  options?: TrainingRunVisualizationOptions,
) => Effect.Effect<TrainingRunVisualizationHandle, unknown>

export type HudStatus =
  | "primary"
  | "secondary"
  | "success"
  | "info"
  | "warning"
  | "error"
  | "neutral"
  | "line"
  | "background"

export const hudStatusColor: (status: HudStatus) => number

export type HudDisposable = Readonly<{ dispose: () => void }>

export type HudMaterialObject = Three.Object3D &
  Readonly<{
    material: {
      color: { set(value: number): void }
      opacity: number
      transparent: boolean
    }
  }>

export type HudObjectHandle = HudDisposable &
  Readonly<{ object3D: HudMaterialObject }>

export type HudGroupHandle = HudDisposable &
  Readonly<{ group: Three.Group }>

export const createHudDotGrid: (
  options?: Readonly<Record<string, unknown>>,
) => HudDisposable & Readonly<{ points: Three.Points }>

export const createHudLabel: (
  options: Readonly<Record<string, unknown>>,
) => HudObjectHandle & Readonly<{ setText: (text: string) => void }>

export const createHudMeter: (
  options?: Readonly<Record<string, unknown>>,
) => HudGroupHandle & Readonly<{ setValue: (value: number) => void }>

export const createHudStatusLight: (
  options?: Readonly<Record<string, unknown>>,
) => HudGroupHandle & Readonly<{
  setStatus: (status: HudStatus) => void
  update: (elapsed: number) => void
}>

export type MokshaAssetUrls = Readonly<Record<string, string>>

export const defaultMokshaAssetUrls: MokshaAssetUrls

export type ProofReplayVector =
  | Three.Vector3
  | readonly [number, number, number]
  | Readonly<{ x: number; y: number; z: number }>

export type ProofReplayCameraPose = Readonly<{
  cameraRef?: string
  fov?: number
  mode?: string
  position: ProofReplayVector
  second?: number
  sourceRefs?: readonly string[]
  target: ProofReplayVector
}>

export type ProofReplayStageDefinition = Readonly<{
  id: string
  kind: string
  label: string
  position: ProofReplayVector
  sourceRefs?: readonly string[]
}>

export type ProofReplayActorDefinition = Readonly<{
  id: string
  label: string
  position: ProofReplayVector
  role: string
  sourceRefs?: readonly string[]
  state?: string
}>

export type ProofReplayEventDefinition = Readonly<{
  actorIds?: readonly string[]
  amountSats?: number
  id: string
  kind: string
  label: string
  rail?: string
  second: number
  sourceRefs?: readonly string[]
  targetIds?: readonly string[]
}>

export type ProofReplayFlowDefinition = Readonly<{
  fromId: string
  id: string
  kind?: string
  sourceRefs?: readonly string[]
  toId: string
}>

export type ProofReplayVisualizationFrame = Readonly<{
  activeEvents?: readonly ProofReplayEventDefinition[]
  actors?: readonly ProofReplayActorDefinition[]
  camera: ProofReplayCameraPose
  second: number
}>

export type ProofReplayVisualizationOptions = Readonly<{
  actors?: readonly ProofReplayActorDefinition[]
  backgroundColor?: number
  camera?: ProofReplayCameraPose
  durationSecond?: number
  events?: readonly ProofReplayEventDefinition[]
  flows?: readonly ProofReplayFlowDefinition[]
  labels?: boolean
  pixelRatio?: number
  stages?: readonly ProofReplayStageDefinition[]
  title?: string
}>

export type ProofReplayVisualizationHandle = Readonly<{
  canvas: HTMLCanvasElement
  dispose: Effect.Effect<void>
  element: HTMLElement
  renderNow: () => void
  resize: Effect.Effect<void>
  setFrame: (frame: ProofReplayVisualizationFrame) => void
  webglAvailable: boolean
}>

export const mountProofReplayVisualization: (
  element: HTMLElement,
  options?: ProofReplayVisualizationOptions,
) => Effect.Effect<ProofReplayVisualizationHandle, unknown>
