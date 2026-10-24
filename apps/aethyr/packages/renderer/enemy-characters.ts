import type { EnemyActorPose } from "../runtime/enemies.ts";
import { ENEMY_LIMITS } from "../runtime/enemies.ts";
import type { CreatureFrame } from "./creatures.ts";
import {
  CharacterRenderer,
  type CharacterInstance,
  type CharacterRendererOptions,
} from "./character-renderer.ts";
import {
  CharacterAnimator,
  characterClipTime,
  fetchCharacterBytes,
  validateCharacter,
  type CharacterManifest,
  type CharacterMotion,
} from "./character.ts";
import { SCENE_LINEAR_SCALE } from "./radiometry.ts";
export type { EnemyActorPose } from "../runtime/enemies.ts";

declare const enemyCaptureReferenceBrand: unique symbol;
export type EnemyCaptureActorPose = Readonly<
  Omit<EnemyActorPose, "position">
> & {
  readonly position: readonly [number, number, number];
};

/** A capture token owns no live animation history or writable palette views. */
export interface EnemyCaptureReference {
  readonly [enemyCaptureReferenceBrand]: true;
  readonly modelIdentity: string;
  readonly actorIds: readonly number[];
  readonly actors: readonly EnemyCaptureActorPose[];
}
interface SubmittedEnemies {
  count: number;
  instances: CharacterInstance[];
  actors: EnemyActorPose[];
  actorIds: number[];
  generation: number;
}
interface CapturedEnemies {
  instances: CharacterInstance[];
  actorIds: readonly number[];
  actors: readonly EnemyCaptureActorPose[];
  generation: number;
}
function canonicalJSON(value: unknown): string {
  return JSON.stringify(value, (_key, item) =>
    item && typeof item === "object" && !Array.isArray(item)
      ? Object.fromEntries(
          Object.keys(item)
            .sort()
            .map((key) => [key, item[key]]),
        )
      : item,
  );
}
function captureModelDescription(manifest: CharacterManifest): string {
  const {
    schema,
    controller,
    animationProfile,
    geometry,
    joints,
    clips,
    materials,
    draws,
  } = manifest;
  return canonicalJSON({
    schema,
    controller,
    animationProfile,
    geometry,
    joints,
    clips,
    materials,
    draws,
  });
}
async function sha256(bytes: ArrayBuffer): Promise<string> {
  return [...new Uint8Array(await crypto.subtle.digest("SHA-256", bytes))]
    .map((byte) => byte.toString(16).padStart(2, "0"))
    .join("");
}
function copyActor(target: EnemyActorPose, actor: EnemyActorPose): void {
  target.id = actor.id;
  for (let axis = 0; axis < 3; axis++)
    target.position[axis] = actor.position[axis];
  target.yaw = actor.yaw;
  target.alive = actor.alive;
  target.deathSeconds = actor.deathSeconds;
  target.animationTime = actor.animationTime;
  target.animation = actor.animation;
  target.health = actor.health;
  target.maxHealth = actor.maxHealth;
}

export const ENEMY_HEALTH_MAX_HALF_NDC = 0.08;
/** Project an upright billboard's width; zero means its anchor is clipped. */
export function enemyHealthBarScale(
  view: ArrayLike<number>,
  center: readonly number[],
) {
  const clip = (row: number) =>
    view[row] * center[0] +
    view[row + 4] * center[1] +
    view[row + 8] * center[2] +
    view[row + 12];
  const w = clip(3),
    z = clip(2);
  if (
    !Number.isFinite(w) ||
    !Number.isFinite(z) ||
    w <= 0.001 ||
    z <= 0 ||
    z >= w
  )
    return 0;
  const rightScale = Math.hypot(view[0], view[4], view[8]);
  if (!Number.isFinite(rightScale) || rightScale <= 0) return 0;
  return Math.min(1, (ENEMY_HEALTH_MAX_HALF_NDC * w) / (0.37 * rightScale));
}

export const ENEMY_HEALTH_WGSL = /*wgsl*/ `
struct Frame{view:mat4x4<f32>,right:vec4<f32>,up:vec4<f32>}
@group(0)@binding(0)var<uniform>frame:Frame;
@group(0)@binding(1)var<storage,read>bars:array<vec4<f32>>;
struct Out{@builtin(position)clip:vec4<f32>,@location(0)color:vec3<f32>}
@vertex fn vs(@builtin(vertex_index)i:u32,@builtin(instance_index)actor:u32)->Out{
 let corners=array<vec2<f32>,6>(vec2(-1.0,-1.0),vec2(1.0,-1.0),vec2(-1.0,1.0),vec2(-1.0,1.0),vec2(1.0,-1.0),vec2(1.0,1.0));
 let bar=bars[actor];let fill=i>=6u;let corner=corners[i%6u];
 let width=select(.37,.34*bar.w,fill);let height=select(.045,.027,fill);
 let x=corner.x*width+select(0.0,-.34+.34*bar.w,fill);
 let center=frame.view*vec4(bar.xyz,1.0);
 let right=frame.view*vec4(frame.right.xyz,0.0);let up=frame.view*vec4(frame.up.xyz,0.0);
 let halfWidth=.37*length(right.xy)/max(center.w,.001);
 let scale=min(1.0,${ENEMY_HEALTH_MAX_HALF_NDC}/max(halfWidth,1e-8));
 var o:Out;o.clip=center+vec4((right.xy*x+up.xy*corner.y*height)*scale,0.0,0.0);
 if center.w<=.001||center.z<=0.0||center.z>=center.w{o.clip=vec4(2.0,2.0,0.0,1.0);}
 o.color=select(vec3(.025),vec3(.12,3.0,.22),fill);return o;
}
@fragment fn fs(v:Out)->@location(0)vec4<f32>{return vec4(v.color*${SCENE_LINEAR_SCALE},1.0);}
`;

/** One bounded instanced billboard pass; no DOM state follows enemy positions. */
export class EnemyHealthBars {
  readonly bytes: number;
  private frameData = new Float32Array(24);
  private actorData: Float32Array;
  submitted = 0;
  private constructor(
    private device: GPUDevice,
    private pipeline: GPURenderPipeline,
    private frame: GPUBuffer,
    private actors: GPUBuffer,
    private group: GPUBindGroup,
    capacity: number,
  ) {
    this.bytes = 96 + capacity * 16;
    this.actorData = new Float32Array(capacity * 4);
  }
  static async create(
    device: GPUDevice,
    capacity: number = ENEMY_LIMITS.actors,
  ) {
    if (!Number.isInteger(capacity) || capacity < 1 || capacity > 64)
      throw Error("Choose up to 64 world health bars.");
    const module = device.createShaderModule({ code: ENEMY_HEALTH_WGSL });
    const pipeline = await device.createRenderPipelineAsync({
      layout: "auto",
      vertex: { module, entryPoint: "vs" },
      fragment: {
        module,
        entryPoint: "fs",
        targets: [{ format: "rgba16float" }],
      },
      primitive: { topology: "triangle-list", cullMode: "none" },
      depthStencil: {
        format: "depth24plus",
        depthWriteEnabled: false,
        depthCompare: "less-equal",
      },
      multisample: { count: 4 },
    });
    const frame = device.createBuffer({
      size: 96,
      usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
    });
    let actors: GPUBuffer | undefined;
    try {
      actors = device.createBuffer({
        size: capacity * 16,
        usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
      });
      const group = device.createBindGroup({
        layout: pipeline.getBindGroupLayout(0),
        entries: [
          { binding: 0, resource: { buffer: frame } },
          { binding: 1, resource: { buffer: actors } },
        ],
      });
      return new EnemyHealthBars(
        device,
        pipeline,
        frame,
        actors,
        group,
        capacity,
      );
    } catch (error) {
      frame.destroy();
      actors?.destroy();
      throw error;
    }
  }
  render(
    pass: GPURenderPassEncoder,
    frame: CreatureFrame,
    actors: readonly EnemyCaptureActorPose[],
    height: number,
  ) {
    this.submitted = 0;
    if (frame.enabled === false) return;
    for (const actor of actors) {
      if (
        !actor.alive ||
        actor.health === undefined ||
        actor.maxHealth === undefined
      )
        continue;
      const center = [
        actor.position[0],
        actor.position[1] + height,
        actor.position[2],
      ];
      if (enemyHealthBarScale(frame.viewProjection, center) <= 0) continue;
      this.actorData.set(
        [...center, actor.health / actor.maxHealth],
        this.submitted * 4,
      );
      this.submitted++;
    }
    if (!this.submitted) return;
    this.frameData.set(frame.viewProjection, 0);
    for (const [row, offset] of [
      [0, 16],
      [1, 20],
    ]) {
      const x = frame.viewProjection[row],
        y = frame.viewProjection[row + 4],
        z = frame.viewProjection[row + 8];
      const length = Math.hypot(x, y, z);
      if (!Number.isFinite(length) || length < 1e-8)
        throw new Error("Choose a complete camera for enemy health bars.");
      this.frameData.set([x / length, y / length, z / length, 0], offset);
    }
    this.device.queue.writeBuffer(this.frame, 0, this.frameData);
    this.device.queue.writeBuffer(
      this.actors,
      0,
      this.actorData.subarray(0, this.submitted * 4),
    );
    pass.setPipeline(this.pipeline);
    pass.setBindGroup(0, this.group);
    pass.draw(12, this.submitted);
  }
  dispose() {
    this.frame.destroy();
    this.actors.destroy();
    this.submitted = 0;
  }
}

/** One model upload and material set, with a bounded pose palette per knight. */
export class EnemyCharacters {
  private samplers = new Map<
    number,
    {
      animator: CharacterAnimator;
      livingTime: number;
      alive: boolean;
      time?: number;
      palette?: Float32Array;
      motion?: CharacterMotion;
      clock?: number;
      transitionUntil?: number;
    }
  >();
  private available: CharacterAnimator[];
  private deathDuration: number;
  private disposed = false;
  private submittedActors = 0;
  private submittedTriangles = 0;
  private healthBarHeight: number;
  private lastSubmitted?: SubmittedEnemies;
  private spareSubmission?: SubmittedEnemies;
  private topologyGeneration = 0;
  private readonly captures = new WeakMap<
    EnemyCaptureReference,
    CapturedEnemies
  >();
  readonly bytes: number;
  private constructor(
    private renderer: CharacterRenderer,
    readonly manifest: CharacterManifest,
    private readonly modelDescription: string,
    readonly modelIdentity: string,
    private healthBars?: EnemyHealthBars,
  ) {
    this.bytes = renderer.bytes + (healthBars?.bytes ?? 0);
    const height = manifest.metadata?.healthBarHeight;
    this.healthBarHeight =
      typeof height === "number" &&
      Number.isFinite(height) &&
      height >= 0.1 &&
      height <= 4
        ? height
        : 1.9;
    // Rig admission and sampler allocation happen during loading, outside a frame.
    this.available = Array.from(
      { length: ENEMY_LIMITS.actors },
      () => new CharacterAnimator(manifest),
    );
    this.deathDuration = manifest.clips.find(
      (clip) => clip.name === "death",
    )!.duration;
  }
  static async create(
    device: GPUDevice,
    manifest: CharacterManifest,
    load = fetchCharacterBytes,
    options: CharacterRendererOptions = {},
  ) {
    validateCharacter(manifest);
    if (!manifest.clips.some((clip) => clip.name === "death"))
      throw new Error(
        "Prepare the knight's death animation before opening the town.",
      );
    if (
      manifest.animationProfile !== "quick-mmorpg-zombie-v1" &&
      !manifest.clips.some((clip) => clip.name === "run" && clip.distance > 0)
    )
      throw new Error(
        "Prepare the knight's running stride before opening the town.",
      );
    const renderer = await CharacterRenderer.create(
      device,
      manifest,
      load,
      ENEMY_LIMITS.actors,
      options,
    );
    try {
      const description = captureModelDescription(manifest),
        identity = await sha256(new TextEncoder().encode(description).buffer);
      const healthBars =
        manifest.animationProfile === "quick-mmorpg-zombie-v1"
          ? await EnemyHealthBars.create(device)
          : undefined;
      return new EnemyCharacters(
        renderer,
        manifest,
        description,
        identity,
        healthBars,
      );
    } catch (error) {
      renderer.dispose();
      throw error;
    }
  }
  get stats() {
    return {
      submittedActors: this.submittedActors,
      submittedTriangles: this.submittedTriangles,
      submittedHealthBars: this.healthBars?.submitted ?? 0,
      submittedHealthBarTriangles: (this.healthBars?.submitted ?? 0) * 4,
    };
  }
  render(
    pass: GPURenderPassEncoder,
    frame: CreatureFrame,
    actors: readonly EnemyActorPose[],
    recordLive = true,
  ) {
    this.submittedActors = 0;
    this.submittedTriangles = 0;
    if (this.healthBars) this.healthBars.submitted = 0;
    if (this.disposed || frame.enabled === false) return;
    if (!Array.isArray(actors) || actors.length > ENEMY_LIMITS.actors)
      throw new Error("The town supports up to 16 knights.");
    const ids = new Set<number>();
    for (const actor of actors) {
      if (
        !actor ||
        !Number.isInteger(actor.id) ||
        actor.id < 0 ||
        actor.id > 0xffff_ffff ||
        ids.has(actor.id) ||
        !Array.isArray(actor.position) ||
        actor.position.length !== 3 ||
        actor.position.some(
          (v: number) => !Number.isFinite(v) || Math.abs(v) > 20_000,
        ) ||
        !Number.isFinite(actor.yaw) ||
        typeof actor.alive !== "boolean" ||
        !Number.isFinite(actor.deathSeconds) ||
        actor.deathSeconds < 0 ||
        actor.deathSeconds > 1e12 ||
        !Number.isFinite(actor.animationTime) ||
        actor.animationTime < 0 ||
        actor.animationTime > 1e12 ||
        (actor.animation !== undefined &&
          !["idle", "walk", "run", "attack", "death"].includes(
            actor.animation,
          )) ||
        (actor.animation !== undefined &&
          !this.manifest.clips.some((clip) => clip.name === actor.animation)) ||
        ((actor.health !== undefined || actor.maxHealth !== undefined) &&
          !(
            typeof actor.health === "number" &&
            Number.isFinite(actor.health) &&
            typeof actor.maxHealth === "number" &&
            Number.isFinite(actor.maxHealth) &&
            actor.maxHealth > 0 &&
            actor.maxHealth <= 1e6 &&
            actor.health >= 0 &&
            actor.health <= actor.maxHealth &&
            actor.alive === actor.health > 0
          ))
      )
        throw new Error("The knight needs a complete world pose.");
      ids.add(actor.id);
    }
    // Despawned IDs cannot leave animation histories growing across play sessions.
    for (const [id, sampler] of this.samplers) {
      if (ids.has(id)) continue;
      sampler.animator.reset();
      this.available.push(sampler.animator);
      this.samplers.delete(id);
    }
    const instances: CharacterInstance[] = [];
    for (const actor of actors) {
      let sampler = this.samplers.get(actor.id);
      if (!sampler) {
        sampler = {
          animator: this.available.pop()!,
          livingTime: actor.animationTime,
          alive: actor.alive,
        };
        this.samplers.set(actor.id, sampler);
      }
      if (sampler.alive && !actor.alive && actor.animation === undefined) {
        // Begin the blend at the simulation's death time, even after skipped frames.
        sampler.animator.sample("death", 0, sampler.livingTime);
        sampler.time = undefined;
      } else if (!sampler.alive && actor.alive) {
        sampler.animator.reset();
        sampler.time = undefined;
      }
      sampler.alive = actor.alive;
      if (actor.alive) sampler.livingTime = actor.animationTime;
      const clock = actor.alive
        ? actor.animationTime
        : sampler.livingTime + actor.deathSeconds;
      const motion: CharacterMotion = actor.alive
        ? (actor.animation ?? (this.manifest.animationProfile ? "idle" : "run"))
        : "death";
      const sourceClock =
        actor.animation !== undefined ||
        this.manifest.animationProfile === "quick-mmorpg-zombie-v1";
      const time =
        actor.alive && sourceClock
          ? actor.animationTime
          : actor.alive
            ? characterClipTime(
                this.manifest,
                "run",
                0,
                actor.animationTime * ENEMY_LIMITS.gaitSpeed,
              )
            : Math.min(actor.deathSeconds, this.deathDuration);
      const sampleClock = sourceClock ? frame.time : clock;
      if (sampler.motion !== motion)
        sampler.transitionUntil = sampleClock + 0.12;
      if (
        sampler.time !== time ||
        sampler.motion !== motion ||
        !sampler.palette ||
        sampleClock <= (sampler.transitionUntil ?? -Infinity) ||
        (sampler.clock ?? Infinity) < (sampler.transitionUntil ?? -Infinity)
      ) {
        sampler.palette = sampler.animator.sample(motion, time, sampleClock);
        sampler.time = time;
        sampler.motion = motion;
      }
      sampler.clock = sampleClock;
      instances.push({
        position: actor.position,
        yaw: actor.yaw,
        palette: sampler.palette,
      });
    }
    const submitted = recordLive
      ? this.stageSubmission(instances, actors)
      : undefined;
    this.renderer.renderInstances(pass, frame, instances);
    this.healthBars?.render(pass, frame, actors, this.healthBarHeight);
    if (submitted) {
      this.spareSubmission = this.lastSubmitted;
      this.lastSubmitted = submitted;
      this.topologyGeneration = submitted.generation;
    }
    this.submittedActors = actors.length;
    this.submittedTriangles =
      (actors.length * this.manifest.geometry.indexCount) / 3;
  }
  private stageSubmission(
    instances: readonly CharacterInstance[],
    actors: readonly EnemyActorPose[],
  ): SubmittedEnemies {
    const next = this.spareSubmission ?? {
      count: 0,
      instances: [],
      actors: [],
      actorIds: [],
      generation: 0,
    };
    next.count = actors.length;
    next.actorIds.length = actors.length;
    for (let index = 0; index < actors.length; index++) {
      const actor = actors[index],
        source = instances[index];
      const instance = (next.instances[index] ??= {
        position: [0, 0, 0],
        yaw: 0,
        palette: new Float32Array(source.palette.length),
      });
      for (let axis = 0; axis < 3; axis++)
        (instance.position as number[])[axis] = source.position[axis];
      instance.yaw = source.yaw;
      instance.palette.set(source.palette);
      const pose = (next.actors[index] ??= {
        id: 0,
        position: [0, 0, 0],
        yaw: 0,
        alive: true,
        deathSeconds: 0,
        animationTime: 0,
      });
      copyActor(pose, actor);
      next.actorIds[index] = actor.id;
    }
    const previous = this.lastSubmitted;
    next.generation =
      this.topologyGeneration +
      Number(
        !previous ||
          previous.count !== next.count ||
          next.actorIds.some((id, index) => previous.actorIds[index] !== id),
      );
    return next;
  }
  private captureSnapshot(reference: EnemyCaptureReference): CapturedEnemies {
    if (this.disposed) throw new Error("The knight renderer has closed.");
    const snapshot = this.captures.get(reference),
      current = this.lastSubmitted;
    if (
      !snapshot ||
      !current ||
      snapshot.generation !== this.topologyGeneration ||
      snapshot.actorIds.length > ENEMY_LIMITS.actors ||
      snapshot.actorIds.length !== current.count ||
      snapshot.actorIds.some((id, index) => current.actorIds[index] !== id) ||
      captureModelDescription(this.manifest) !== this.modelDescription
    )
      throw new Error("Choose a current knight capture from this world.");
    return snapshot;
  }
  pinCaptureReference(): EnemyCaptureReference {
    if (this.disposed) throw new Error("The knight renderer has closed.");
    const submitted = this.lastSubmitted;
    if (
      !submitted ||
      captureModelDescription(this.manifest) !== this.modelDescription
    )
      throw new Error("Render the knights before taking a capture.");
    return this.storeCaptureReference(
      submitted.instances.slice(0, submitted.count),
      submitted.actors.slice(0, submitted.count),
      submitted.generation,
    );
  }
  /** Only the trusted host supplies poses from the source-native enemy setup. */
  pinNativeStartReference(
    actors: readonly EnemyActorPose[],
  ): EnemyCaptureReference {
    if (this.disposed) throw new Error("The knight renderer has closed.");
    const submitted = this.lastSubmitted;
    if (
      !submitted ||
      !Array.isArray(actors) ||
      actors.length > ENEMY_LIMITS.actors ||
      actors.length !== submitted.count ||
      captureModelDescription(this.manifest) !== this.modelDescription
    )
      throw new Error(
        "Render this world's knights before choosing their starting pose.",
      );
    for (let index = 0; index < actors.length; index++) {
      const actor = actors[index];
      if (
        !actor ||
        actor.id !== submitted.actorIds[index] ||
        actor.alive !== true ||
        actor.deathSeconds !== 0 ||
        actor.animationTime !== 0 ||
        !Array.isArray(actor.position) ||
        actor.position.length !== 3 ||
        Array.from(actor.position).some(
          (value) =>
            typeof value !== "number" ||
            !Number.isFinite(value) ||
            Math.abs(value) > 20_000,
        ) ||
        !Number.isFinite(actor.yaw)
      )
        throw new Error(
          "Choose the knights' unchanged starting poses from this world.",
        );
    }
    const instances = actors.map((actor) => ({
      position: actor.position,
      yaw: actor.yaw,
      palette: new CharacterAnimator(this.manifest).sample(
        actor.animation ?? (this.manifest.animationProfile ? "idle" : "run"),
        0,
        0,
      ),
    }));
    return this.storeCaptureReference(instances, actors, submitted.generation);
  }
  private storeCaptureReference(
    sourceInstances: readonly CharacterInstance[],
    sourceActors: readonly EnemyActorPose[],
    generation: number,
  ): EnemyCaptureReference {
    const instances = sourceInstances.map((instance) => ({
      position: [...instance.position],
      yaw: instance.yaw,
      palette: instance.palette.slice(),
    }));
    const actors = Object.freeze(
      sourceActors.map((actor) =>
        Object.freeze({
          id: actor.id,
          yaw: actor.yaw,
          alive: actor.alive,
          deathSeconds: actor.deathSeconds,
          animationTime: actor.animationTime,
          animation: actor.animation,
          health: actor.health,
          maxHealth: actor.maxHealth,
          position: Object.freeze([...actor.position]) as readonly [
            number,
            number,
            number,
          ],
        }),
      ),
    );
    const actorIds = Object.freeze(sourceActors.map((actor) => actor.id));
    const reference = Object.freeze({
      modelIdentity: this.modelIdentity,
      actorIds,
      actors,
    }) as unknown as EnemyCaptureReference;
    this.captures.set(reference, {
      instances,
      actors,
      actorIds,
      generation,
    });
    return reference;
  }
  async captureReferenceIdentity(reference: EnemyCaptureReference): Promise<{
    modelIdentity: string;
    poseSha256: string;
  }> {
    const snapshot = this.captureSnapshot(reference);
    const header = new TextEncoder().encode(
      canonicalJSON({
        schema: "aethyrforge.enemy.capture.v1",
        modelIdentity: this.modelIdentity,
        actors: snapshot.actors,
        instances: snapshot.instances.map(({ position, yaw, palette }) => ({
          position,
          yaw,
          paletteFloats: palette.length,
        })),
      }),
    );
    const bytes = new Uint8Array(
      header.byteLength +
        1 +
        snapshot.instances.reduce(
          (sum, instance) => sum + instance.palette.byteLength,
          0,
        ),
    );
    bytes.set(header);
    let offset = header.byteLength + 1;
    for (const instance of snapshot.instances) {
      bytes.set(
        new Uint8Array(
          instance.palette.buffer,
          instance.palette.byteOffset,
          instance.palette.byteLength,
        ),
        offset,
      );
      offset += instance.palette.byteLength;
    }
    const poseSha256 = await sha256(bytes.buffer);
    this.captureSnapshot(reference);
    return { modelIdentity: this.modelIdentity, poseSha256 };
  }
  renderCapture(
    pass: GPURenderPassEncoder,
    frame: CreatureFrame,
    reference: EnemyCaptureReference,
  ): void {
    const snapshot = this.captureSnapshot(reference);
    this.submittedActors = 0;
    this.submittedTriangles = 0;
    this.renderer.renderInstances(pass, frame, snapshot.instances);
    this.healthBars?.render(pass, frame, snapshot.actors, this.healthBarHeight);
    if (frame.enabled === false) return;
    this.submittedActors = snapshot.actorIds.length;
    this.submittedTriangles =
      (snapshot.actorIds.length * this.manifest.geometry.indexCount) / 3;
  }
  dispose() {
    if (this.disposed) return;
    this.disposed = true;
    this.renderer.dispose();
    this.healthBars?.dispose();
    this.samplers.clear();
    this.available.length = 0;
    this.lastSubmitted = this.spareSubmission = undefined;
    this.topologyGeneration++;
    this.submittedActors = 0;
    this.submittedTriangles = 0;
  }
}
