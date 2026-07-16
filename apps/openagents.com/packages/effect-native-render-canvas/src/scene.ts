import { Schema } from "effect"

/**
 * Typed scene-descriptor catalog for `@effect-native/render-canvas`.
 *
 * This catalog is intentionally LOCAL to this package. It is a small, closed,
 * bounded 3D/data-viz descriptor set — separate from the main UI component
 * catalog in `@effect-native/core`. An invalid scene cannot be constructed:
 * every numeric field is finite, sizes are positive, opacity is a unit
 * interval, colors are hex strings, and node kinds/geometry/material refs are
 * closed unions.
 */

export const SceneCatalogVersion = "effect-native/scene/v1" as const
export type SceneCatalogVersion = typeof SceneCatalogVersion
export const SceneCatalogVersionSchema = Schema.Literal(SceneCatalogVersion)

export const sceneNodeTags = ["Group", "Mesh", "Line", "Points", "Label"] as const
export type SceneNodeTag = (typeof sceneNodeTags)[number]

// ---------------------------------------------------------------------------
// Bounded scalar schemas
// ---------------------------------------------------------------------------

export const FiniteNumberSchema = Schema.Number.check(Schema.isFinite({ title: "FiniteNumber" }))

export const PositiveNumberSchema = Schema.Number.check(
  Schema.isFinite({ title: "FiniteNumber" }),
  Schema.isGreaterThan(0, { title: "PositiveNumber" })
)

export const UnitIntervalSchema = Schema.Number.check(
  Schema.isFinite({ title: "FiniteNumber" }),
  Schema.isGreaterThanOrEqualTo(0, { title: "MinUnit" }),
  Schema.isLessThanOrEqualTo(1, { title: "MaxUnit" })
)

export const FovSchema = Schema.Number.check(
  Schema.isFinite({ title: "FiniteNumber" }),
  Schema.isGreaterThan(0, { title: "MinFov" }),
  Schema.isLessThan(180, { title: "MaxFov" })
)

export const SegmentCountSchema = Schema.Int.check(
  Schema.isGreaterThanOrEqualTo(3, { title: "MinSegments" }),
  Schema.isLessThanOrEqualTo(256, { title: "MaxSegments" })
)

export const HexColorSchema = Schema.String.check(
  Schema.isPattern(/^#(?:[0-9a-fA-F]{3}|[0-9a-fA-F]{6})$/, { title: "HexColor" })
)

export const NodeKeySchema = Schema.NonEmptyString

export type Vec3 = readonly [number, number, number]
export const Vec3Schema: Schema.Codec<Vec3, Vec3> = Schema.Tuple([
  FiniteNumberSchema,
  FiniteNumberSchema,
  FiniteNumberSchema
]) as unknown as Schema.Codec<Vec3, Vec3>

export const LabelAnchorSchema = Schema.Literals(["start", "center", "end"] as const)
export type LabelAnchor = Schema.Schema.Type<typeof LabelAnchorSchema>

// ---------------------------------------------------------------------------
// Geometry and material refs (closed unions, bounded props)
// ---------------------------------------------------------------------------

export interface BoxGeometry {
  readonly _tag: "Box"
  readonly width: number
  readonly height: number
  readonly depth: number
}
export interface SphereGeometry {
  readonly _tag: "Sphere"
  readonly radius: number
  readonly segments?: number
}
export interface PlaneGeometry {
  readonly _tag: "Plane"
  readonly width: number
  readonly height: number
}
export type GeometryRef = BoxGeometry | SphereGeometry | PlaneGeometry

export const BoxGeometrySchema: Schema.Codec<BoxGeometry, BoxGeometry> = Schema.TaggedStruct("Box", {
  width: PositiveNumberSchema,
  height: PositiveNumberSchema,
  depth: PositiveNumberSchema
})
export const SphereGeometrySchema: Schema.Codec<SphereGeometry, SphereGeometry> = Schema.TaggedStruct("Sphere", {
  radius: PositiveNumberSchema,
  segments: SegmentCountSchema.pipe(Schema.optionalKey)
})
export const PlaneGeometrySchema: Schema.Codec<PlaneGeometry, PlaneGeometry> = Schema.TaggedStruct("Plane", {
  width: PositiveNumberSchema,
  height: PositiveNumberSchema
})
export const GeometryRefSchema: Schema.Codec<GeometryRef, GeometryRef> = Schema.Union([
  BoxGeometrySchema,
  SphereGeometrySchema,
  PlaneGeometrySchema
])

export interface BasicMaterial {
  readonly _tag: "Basic"
  readonly color: string
  readonly opacity?: number
  readonly wireframe?: boolean
}
export interface StandardMaterial {
  readonly _tag: "Standard"
  readonly color: string
  readonly opacity?: number
  readonly metalness?: number
  readonly roughness?: number
  readonly emissive?: string
}
export type MaterialRef = BasicMaterial | StandardMaterial

export const BasicMaterialSchema: Schema.Codec<BasicMaterial, BasicMaterial> = Schema.TaggedStruct("Basic", {
  color: HexColorSchema,
  opacity: UnitIntervalSchema.pipe(Schema.optionalKey),
  wireframe: Schema.Boolean.pipe(Schema.optionalKey)
})
export const StandardMaterialSchema: Schema.Codec<StandardMaterial, StandardMaterial> = Schema.TaggedStruct(
  "Standard",
  {
    color: HexColorSchema,
    opacity: UnitIntervalSchema.pipe(Schema.optionalKey),
    metalness: UnitIntervalSchema.pipe(Schema.optionalKey),
    roughness: UnitIntervalSchema.pipe(Schema.optionalKey),
    emissive: HexColorSchema.pipe(Schema.optionalKey)
  }
)
export const MaterialRefSchema: Schema.Codec<MaterialRef, MaterialRef> = Schema.Union([
  BasicMaterialSchema,
  StandardMaterialSchema
])

// ---------------------------------------------------------------------------
// Camera (closed union)
// ---------------------------------------------------------------------------

export interface PerspectiveCamera {
  readonly _tag: "Perspective"
  readonly position: Vec3
  readonly target: Vec3
  readonly fov: number
  readonly near: number
  readonly far: number
}
export interface OrthographicCamera {
  readonly _tag: "Orthographic"
  readonly position: Vec3
  readonly target: Vec3
  readonly frustum: number
  readonly near: number
  readonly far: number
}
export type Camera = PerspectiveCamera | OrthographicCamera

export const PerspectiveCameraSchema: Schema.Codec<PerspectiveCamera, PerspectiveCamera> = Schema.TaggedStruct(
  "Perspective",
  {
    position: Vec3Schema,
    target: Vec3Schema,
    fov: FovSchema,
    near: PositiveNumberSchema,
    far: PositiveNumberSchema
  }
)
export const OrthographicCameraSchema: Schema.Codec<OrthographicCamera, OrthographicCamera> = Schema.TaggedStruct(
  "Orthographic",
  {
    position: Vec3Schema,
    target: Vec3Schema,
    frustum: PositiveNumberSchema,
    near: PositiveNumberSchema,
    far: PositiveNumberSchema
  }
)
export const CameraSchema: Schema.Codec<Camera, Camera> = Schema.Union([
  PerspectiveCameraSchema,
  OrthographicCameraSchema
])

// ---------------------------------------------------------------------------
// Scene nodes
// ---------------------------------------------------------------------------

export interface GroupNode {
  readonly _tag: "Group"
  readonly key: string
  readonly position?: Vec3
  readonly rotation?: Vec3
  readonly scale?: Vec3
  readonly visible?: boolean
  readonly children: ReadonlyArray<SceneNode>
}
export interface MeshNode {
  readonly _tag: "Mesh"
  readonly key: string
  readonly geometry: GeometryRef
  readonly material: MaterialRef
  readonly position?: Vec3
  readonly rotation?: Vec3
  readonly scale?: Vec3
  readonly visible?: boolean
}
export interface LineNode {
  readonly _tag: "Line"
  readonly key: string
  readonly points: ReadonlyArray<Vec3>
  readonly color: string
  readonly opacity?: number
  readonly width?: number
  readonly position?: Vec3
  readonly visible?: boolean
}
export interface PointsNode {
  readonly _tag: "Points"
  readonly key: string
  readonly positions: ReadonlyArray<Vec3>
  readonly size: number
  readonly color: string
  readonly opacity?: number
  readonly visible?: boolean
}
export interface LabelNode {
  readonly _tag: "Label"
  readonly key: string
  readonly text: string
  readonly color: string
  readonly fontSize: number
  readonly anchor?: LabelAnchor
  readonly position?: Vec3
  readonly visible?: boolean
}

export type SceneNode = GroupNode | MeshNode | LineNode | PointsNode | LabelNode

/** A scene node with any child nodes stripped — the unit the backend creates. */
export type SceneNodeLeaf = Omit<GroupNode, "children"> | MeshNode | LineNode | PointsNode | LabelNode

const CommonNodeFields = {
  key: NodeKeySchema,
  position: Vec3Schema.pipe(Schema.optionalKey),
  rotation: Vec3Schema.pipe(Schema.optionalKey),
  scale: Vec3Schema.pipe(Schema.optionalKey),
  visible: Schema.Boolean.pipe(Schema.optionalKey)
} as const

const SceneNodeSelf = Schema.suspend((): Schema.Codec<SceneNode, SceneNode> => SceneNodeSchema)

const PointArraySchema = Schema.Array(Vec3Schema).check(Schema.isMinLength(2, { title: "LineNeedsTwoPoints" }))
const PositionArraySchema = Schema.Array(Vec3Schema).check(Schema.isMinLength(1, { title: "PointsNeedsOnePosition" }))

export const GroupNodeSchema: Schema.Codec<GroupNode, GroupNode> = Schema.TaggedStruct("Group", {
  ...CommonNodeFields,
  children: Schema.Array(SceneNodeSelf)
})
export const MeshNodeSchema: Schema.Codec<MeshNode, MeshNode> = Schema.TaggedStruct("Mesh", {
  ...CommonNodeFields,
  geometry: GeometryRefSchema,
  material: MaterialRefSchema
})
export const LineNodeSchema: Schema.Codec<LineNode, LineNode> = Schema.TaggedStruct("Line", {
  key: NodeKeySchema,
  points: PointArraySchema,
  color: HexColorSchema,
  opacity: UnitIntervalSchema.pipe(Schema.optionalKey),
  width: PositiveNumberSchema.pipe(Schema.optionalKey),
  position: Vec3Schema.pipe(Schema.optionalKey),
  visible: Schema.Boolean.pipe(Schema.optionalKey)
})
export const PointsNodeSchema: Schema.Codec<PointsNode, PointsNode> = Schema.TaggedStruct("Points", {
  key: NodeKeySchema,
  positions: PositionArraySchema,
  size: PositiveNumberSchema,
  color: HexColorSchema,
  opacity: UnitIntervalSchema.pipe(Schema.optionalKey),
  visible: Schema.Boolean.pipe(Schema.optionalKey)
})
export const LabelNodeSchema: Schema.Codec<LabelNode, LabelNode> = Schema.TaggedStruct("Label", {
  key: NodeKeySchema,
  text: Schema.String,
  color: HexColorSchema,
  fontSize: PositiveNumberSchema,
  anchor: LabelAnchorSchema.pipe(Schema.optionalKey),
  position: Vec3Schema.pipe(Schema.optionalKey),
  visible: Schema.Boolean.pipe(Schema.optionalKey)
})

export const SceneNodeSchema: Schema.Codec<SceneNode, SceneNode> = Schema.suspend(() =>
  Schema.Union([GroupNodeSchema, MeshNodeSchema, LineNodeSchema, PointsNodeSchema, LabelNodeSchema])
)

export interface CanvasScene {
  readonly _tag: "Scene"
  readonly catalogVersion: SceneCatalogVersion
  readonly camera: Camera
  readonly background?: string
  readonly children: ReadonlyArray<SceneNode>
}

export const CanvasSceneSchema: Schema.Codec<CanvasScene, CanvasScene> = Schema.TaggedStruct("Scene", {
  catalogVersion: SceneCatalogVersionSchema,
  camera: CameraSchema,
  background: HexColorSchema.pipe(Schema.optionalKey),
  children: Schema.Array(SceneNodeSchema)
})

/** Decoder app authors / renderers use for persisted or externally authored scenes. */
export const CompatibleSceneSchema: Schema.Codec<CanvasScene, CanvasScene> = CanvasSceneSchema

export const decodeScene = Schema.decodeUnknownSync(CanvasSceneSchema)
export const encodeScene = Schema.encodeSync(CanvasSceneSchema)

// ---------------------------------------------------------------------------
// Constructors (validate on construction; an invalid scene throws here)
// ---------------------------------------------------------------------------

export const perspectiveCamera = (props: Omit<PerspectiveCamera, "_tag">): PerspectiveCamera =>
  PerspectiveCameraSchema.make({ _tag: "Perspective", ...props })
export const orthographicCamera = (props: Omit<OrthographicCamera, "_tag">): OrthographicCamera =>
  OrthographicCameraSchema.make({ _tag: "Orthographic", ...props })

export const box = (props: Omit<BoxGeometry, "_tag">): BoxGeometry => BoxGeometrySchema.make({ _tag: "Box", ...props })
export const sphere = (props: Omit<SphereGeometry, "_tag">): SphereGeometry =>
  SphereGeometrySchema.make({ _tag: "Sphere", ...props })
export const plane = (props: Omit<PlaneGeometry, "_tag">): PlaneGeometry =>
  PlaneGeometrySchema.make({ _tag: "Plane", ...props })

export const basicMaterial = (props: Omit<BasicMaterial, "_tag">): BasicMaterial =>
  BasicMaterialSchema.make({ _tag: "Basic", ...props })
export const standardMaterial = (props: Omit<StandardMaterial, "_tag">): StandardMaterial =>
  StandardMaterialSchema.make({ _tag: "Standard", ...props })

export const group = (
  props: Omit<GroupNode, "_tag" | "children">,
  children: ReadonlyArray<SceneNode> = []
): GroupNode => GroupNodeSchema.make({ _tag: "Group", ...props, children })
export const mesh = (props: Omit<MeshNode, "_tag">): MeshNode => MeshNodeSchema.make({ _tag: "Mesh", ...props })
export const line = (props: Omit<LineNode, "_tag">): LineNode => LineNodeSchema.make({ _tag: "Line", ...props })
export const points = (props: Omit<PointsNode, "_tag">): PointsNode =>
  PointsNodeSchema.make({ _tag: "Points", ...props })
export const label = (props: Omit<LabelNode, "_tag">): LabelNode => LabelNodeSchema.make({ _tag: "Label", ...props })

export const scene = (
  props: Omit<CanvasScene, "_tag" | "catalogVersion" | "children">,
  children: ReadonlyArray<SceneNode> = []
): CanvasScene =>
  CanvasSceneSchema.make({
    _tag: "Scene",
    catalogVersion: SceneCatalogVersion,
    ...props,
    children
  })

/** Strip children from a node, yielding the leaf descriptor the backend receives. */
export const toLeaf = (node: SceneNode): SceneNodeLeaf => {
  if (node._tag === "Group") {
    const { children: _children, ...rest } = node
    return rest
  }
  return node
}

export const childrenOf = (node: SceneNode): ReadonlyArray<SceneNode> => (node._tag === "Group" ? node.children : [])
