import { direction } from "./math.ts";
import type { SpellSwarm, SpellSwarmOptions } from "./spell-swarm.ts";

export interface SpellSwarmHostContext {
  readonly device: GPUDevice;
  camera():
    | {
        readonly eye: readonly [number, number, number];
        readonly yaw: number;
        readonly pitch: number;
      }
    | undefined;
  closed(): boolean;
  account(delta: number): void;
}
export type SpellSwarmHostOptions = Pick<
  SpellSwarmOptions,
  "orientationPreparation"
>;
export type SpellSwarmCount = 0 | 10000 | 100000 | 200000;
interface SubmittedGeometry {
  instances: number;
  triangles: number;
  drawCalls: number;
}

/** Editor-only ownership; static players do not import this module. */
export class SpellSwarmHost {
  private current?: SpellSwarm;
  private revision = 0;
  private time: number | undefined;
  private disposed = false;

  constructor(
    private context: SpellSwarmHostContext,
    private options: SpellSwarmHostOptions = {},
    private create: typeof SpellSwarm.create = async (...args) =>
      (await import("./spell-swarm.ts")).SpellSwarm.create(...args),
  ) {}

  info() {
    return this.current?.inspect() ?? null;
  }
  async probe() {
    return this.current ? this.current.probe() : null;
  }
  private closed() {
    return this.disposed || this.context.closed();
  }

  async setCount(count: SpellSwarmCount) {
    if (![0, 10000, 100000, 200000].includes(count))
      throw new Error(
        "Choose 10,000, 100,000, or 200,000 spell creatures, or turn the swarm off.",
      );
    if (this.closed())
      throw new Error("Reopen the world before casting a swarm.");
    const revision = ++this.revision;
    if (count === 0) {
      const previous = this.current;
      this.current = undefined;
      this.time = undefined;
      this.context.account(-(previous?.bytes ?? 0));
      previous?.dispose();
      return;
    }
    const camera = this.context.camera();
    const forward = camera ? direction(camera.yaw, camera.pitch) : [0, 0, -1];
    const eye = camera?.eye ?? [6, 4, 20];
    const center: [number, number, number] = [
      eye[0] + forward[0] * 18,
      Math.max(12, eye[1] + forward[1] * 18 + 7),
      eye[2] + forward[2] * 18,
    ];
    const next = await this.create(this.context.device, {
      count,
      center,
      ...this.options,
    });
    let activated = false;
    try {
      if (this.closed() || revision !== this.revision) return;
      await this.context.device.queue.onSubmittedWorkDone();
      if (this.closed() || revision !== this.revision) return;
      const previous = this.current;
      this.current = next;
      this.time = undefined;
      this.context.account(next.bytes - (previous?.bytes ?? 0));
      activated = true;
      previous?.dispose();
    } finally {
      if (!activated) next.dispose();
    }
  }

  prepareRender(encoder: GPUCommandEncoder, time: number, capture: boolean) {
    const current = this.current;
    if (!current) return;
    if (!capture) {
      const dt =
        this.time === undefined
          ? 1 / 60
          : Math.min(0.05, Math.max(0, time - this.time));
      if (dt > 0) current.encodeSimulation(encoder, dt, time);
      this.time = time;
    }
    current.prepareRender(encoder);
  }

  render(
    pass: GPURenderPassEncoder,
    view: Float32Array,
    eye: readonly number[],
    time: number,
    geometry: SubmittedGeometry,
  ) {
    const current = this.current;
    if (!current?.render(pass, view, eye, time)) return false;
    geometry.instances += current.count;
    geometry.triangles += current.count * 310;
    geometry.drawCalls++;
    return true;
  }

  afterSubmit() {
    this.current?.afterSubmit();
  }

  dispose() {
    if (this.disposed) return;
    this.disposed = true;
    this.revision++;
    const previous = this.current;
    this.current = undefined;
    this.time = undefined;
    this.context.account(-(previous?.bytes ?? 0));
    previous?.dispose();
  }
}

export function createSpellSwarmHost(
  context: SpellSwarmHostContext,
  options: SpellSwarmHostOptions = {},
): SpellSwarmHost {
  return new SpellSwarmHost(context, options);
}
