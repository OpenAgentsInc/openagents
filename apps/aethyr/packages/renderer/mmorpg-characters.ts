import {
  MMORPG_CLASSES,
  validateMmorpgActors,
  type MmorpgActorPose,
  type MmorpgClass,
} from "../assets/src/mmorpg.ts";
import {
  CharacterRenderer,
  type CharacterFrame,
  type CharacterInstance,
  type CharacterRendererOptions,
} from "./character-renderer.ts";
import {
  CharacterAnimator,
  fetchCharacterBytes,
  validateCharacter,
  type CharacterManifest,
} from "./character.ts";
import { EnemyHealthBars } from "./enemy-characters.ts";
import { MmorpgAnimationClock } from "../runtime/mmorpg-animation.ts";
import {
  inverse,
  multiply,
  modelMatrix,
  transformPoint,
  type V3,
} from "./math.ts";
export type { MmorpgActorPose, MmorpgClass } from "../assets/src/mmorpg.ts";
export type MmorpgAttachments = ReadonlyMap<
  number,
  Readonly<Record<"Head" | "Hips" | "RightHandIndex1" | "LeftHandIndex1", V3>>
>;
const ANCHORS = ["Head", "Hips", "RightHandIndex1", "LeftHandIndex1"] as const;
/** Simulation yaw faces -Z at zero; the original imported models face +Z. */
export const mmorpgModelYaw = (yaw: number) => Math.PI - yaw;
/** One model and material upload per class, shared by every visible actor. */
export class MmorpgCharacters {
  private samplers = new Map<
    number,
    {
      kind: MmorpgClass;
      animator: CharacterAnimator;
      clock: MmorpgAnimationClock;
    }
  >();
  private groups = new Map<MmorpgClass, CharacterInstance[]>();
  private anchorPositions = new Map<
    number,
    Record<(typeof ANCHORS)[number], V3>
  >();
  private sourceBind = new Map<
    MmorpgClass,
    { names: string[]; matrices: Float32Array[] }
  >();
  private preparedActors?: readonly MmorpgActorPose[];
  private preparedTime = NaN;
  private submitted = {
    actors: 0,
    triangles: 0,
    healthBars: 0,
    healthBarTriangles: 0,
    classes: 0,
  };
  private disposed = false;
  readonly bytes: number;
  private constructor(
    private pools: Map<MmorpgClass, CharacterRenderer>,
    readonly manifests: Readonly<Record<MmorpgClass, CharacterManifest>>,
    private bars: EnemyHealthBars,
  ) {
    this.bytes =
      [...pools.values()].reduce((n, p) => n + p.bytes, 0) + bars.bytes;
    for (const kind of MMORPG_CLASSES)
      this.sourceBind.set(kind, {
        names:
          (manifests[kind].metadata?.sourceJointNames as
            string[] | undefined) ?? [],
        matrices: manifests[kind].joints.map((j) => inverse(j.inverseBind)),
      });
  }
  static async create(
    device: GPUDevice,
    manifests: Readonly<Record<MmorpgClass, CharacterManifest>>,
    load = fetchCharacterBytes,
    options: CharacterRendererOptions = {},
  ) {
    for (const kind of MMORPG_CLASSES) {
      validateCharacter(manifests[kind]);
      if (manifests[kind].mmorpgClass !== kind)
        throw Error("Prepare the matching world character classes.");
    }
    const pools = new Map<MmorpgClass, CharacterRenderer>();
    let bars: EnemyHealthBars | undefined;
    try {
      for (const kind of MMORPG_CLASSES)
        pools.set(
          kind,
          await CharacterRenderer.create(
            device,
            manifests[kind],
            load,
            64,
            options,
          ),
        );
      bars = await EnemyHealthBars.create(device, 64);
      return new MmorpgCharacters(pools, manifests, bars);
    } catch (error) {
      for (const pool of pools.values()) pool.dispose();
      bars?.dispose();
      throw error;
    }
  }
  get stats() {
    return {
      ...this.submitted,
      modelLoaded: !this.disposed,
      geometryBytes: this.bytes,
    };
  }
  get attachments(): MmorpgAttachments {
    return this.anchorPositions;
  }
  prepare(frame: CharacterFrame, actors: readonly MmorpgActorPose[]) {
    if (this.preparedActors === actors && this.preparedTime === frame.time)
      return;
    validateMmorpgActors(actors);
    for (const actor of actors)
      if (
        !this.manifests[actor.class].clips.some((c) => c.name === actor.state)
      )
        throw Error("Choose an authored animation for this character.");
    const live = new Set(actors.map((a) => a.id));
    for (const id of this.samplers.keys())
      if (!live.has(id)) this.samplers.delete(id);
    this.groups.clear();
    this.anchorPositions.clear();
    const elapsed = Number.isFinite(this.preparedTime)
      ? Math.max(0, frame.time - this.preparedTime)
      : 0;
    for (const actor of actors) {
      let sampler = this.samplers.get(actor.id);
      if (!sampler || sampler.kind !== actor.class) {
        sampler = {
          kind: actor.class,
          animator: new CharacterAnimator(this.manifests[actor.class]),
          clock: new MmorpgAnimationClock(this.manifests[actor.class]),
        };
        this.samplers.set(actor.id, sampler);
      }
      const pose = sampler.clock.sample(actor, elapsed, frame.time);
      const palette = sampler.animator.sample(
        pose.animation,
        pose.time,
        frame.time,
        pose.blend,
      );
      const instances = this.groups.get(actor.class) ?? [];
      instances.push({
        position: actor.position,
        yaw: mmorpgModelYaw(actor.yaw),
        palette,
        weapon: actor.weapon,
      });
      this.groups.set(actor.class, instances);
      const source = this.sourceBind.get(actor.class)!,
        model = modelMatrix(actor.position, mmorpgModelYaw(actor.yaw)),
        anchors = {} as Record<(typeof ANCHORS)[number], V3>;
      for (const name of ANCHORS) {
        const joint = source.names.indexOf(name);
        if (joint < 0) {
          anchors[name] = [...actor.position];
          continue;
        }
        const bone = multiply(
          palette.subarray(joint * 32, joint * 32 + 16),
          source.matrices[joint],
        );
        anchors[name] = transformPoint(model, [bone[12], bone[13], bone[14]]);
      }
      this.anchorPositions.set(actor.id, anchors);
    }
    this.preparedActors = actors;
    this.preparedTime = frame.time;
  }
  render(
    pass: GPURenderPassEncoder,
    frame: CharacterFrame,
    actors: readonly MmorpgActorPose[],
    localActorId?: number,
  ) {
    this.submitted = {
      actors: 0,
      triangles: 0,
      healthBars: 0,
      healthBarTriangles: 0,
      classes: 0,
    };
    if (this.disposed || frame.enabled === false) return;
    this.prepare(frame, actors);
    for (const actor of actors) {
      this.submitted.actors++;
      this.submitted.triangles += this.manifests[actor.class].draws.reduce(
        (n, d) =>
          n + (!d.part || d.part === actor.weapon ? d.indexCount / 3 : 0),
        0,
      );
    }
    for (const [kind, instances] of this.groups) {
      this.pools.get(kind)!.renderInstances(pass, frame, instances);
      this.submitted.classes++;
    }
    const health = actors
      .filter((a) => a.id !== localActorId)
      .map((a) => ({
        id: a.id,
        position: [
          a.position[0],
          a.position[1] +
            Number(this.manifests[a.class].metadata?.nameOffset ?? 1.5),
          a.position[2],
        ] as V3,
        yaw: a.yaw,
        alive: a.alive,
        deathSeconds: a.state === "death" ? a.actionTime : 0,
        animationTime: a.animationTime,
        health: a.health,
        maxHealth: a.maxHealth,
      }));
    this.bars.render(pass, frame, health, 0);
    this.submitted.healthBars = this.bars.submitted;
    this.submitted.healthBarTriangles = this.bars.submitted * 4;
  }
  renderShadow(
    pass: GPURenderPassEncoder,
    matrix: ArrayLike<number>,
    key: string,
  ) {
    if (this.disposed) return;
    for (const [kind, instances] of this.groups)
      this.pools.get(kind)!.renderShadow(pass, matrix, instances, key);
  }
  dispose() {
    if (this.disposed) return;
    this.disposed = true;
    for (const pool of this.pools.values()) pool.dispose();
    this.bars.dispose();
    this.samplers.clear();
    this.anchorPositions.clear();
  }
}
