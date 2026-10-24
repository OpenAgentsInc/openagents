import { BATTLE_COMMON_WGSL } from "./battle-crowd-shader.ts";
import { BATTLE_COMBAT_TYPES } from "./battle-combat-shader.ts";
import { BATTLE_BODY_RADII } from "./battle-grid.ts";

export const BATTLE_PARTICIPANT_CONTACT_LIMIT = 64;
export const BATTLE_PARTICIPANT_CONTACT_BYTES = 32 + 64 * 32;

export interface BattleParticipantContacts {
  readonly serial: number;
  /** Monotonic milliseconds when the GPU query was encoded, before queue/readback delay. */
  readonly sampledAt: number;
  readonly epoch: number;
  readonly tick: number;
  readonly position: readonly [number, number, number];
  readonly overflow: number;
  readonly contacts: readonly {
    readonly id: number;
    readonly position: readonly [number, number, number];
    readonly radius: number;
    readonly velocity: readonly [number, number];
  }[];
}

/** One bounded query reads current living poses through the most recent spatial grid. */
export const BATTLE_PARTICIPANT_CONTACTS_WGSL =
  BATTLE_COMMON_WGSL +
  BATTLE_COMBAT_TYPES +
  /* wgsl */ `
struct Cell {count:atomic<u32>,x:atomic<i32>,z:atomic<i32>,members:array<u32,64>}
struct Contact {pose:vec4f,velocity:vec2f,id:u32,distance:f32}
struct Contacts {info:vec4u,origin:vec4f,entries:array<Contact,64>}
@group(0)@binding(0)var<storage,read>actors:array<Actor>;
@group(0)@binding(1)var<storage,read_write>cells:array<Cell>;
@group(0)@binding(2)var<storage,read_write>combat:Combat;
@group(0)@binding(3)var<storage,read>counters:array<u32>;
@group(0)@binding(4)var<storage,read_write>contacts:Contacts;
@group(0)@binding(5)var<uniform>frame:Frame;
@compute @workgroup_size(1)fn participantContacts(){
 contacts.info=vec4u(atomicLoad(&combat.data[11]),counters[11],0u,0u);
 contacts.origin=vec4(frame.participantPose.xyz,1.0);
 if frame.participant.y==0u||atomicLoad(&combat.data[1])==0u{return;}
 let center=battleGridPosition(frame.participantPose.xz,frame.battle.xy);var found=0u;var gridOverflow=0u;
 for(var z=-1;z<=1;z++){for(var x=-1;x<=1;x++){
  let q=center+vec2i(x,z);if any(q<vec2i(0))||any(q>=vec2i(battleGridDimension)){continue;}
  let c=battleGridIndex(q);let population=atomicLoad(&cells[c].count);gridOverflow+=population-min(population,64u);
  for(var n=0u;n<min(population,64u);n++){
   let id=cells[c].members[n];if id>=frame.counts.x{continue;}let a=actors[id];if a.motion.z<=0.0{continue;}
   let delta=a.pose.xz-frame.participantPose.xz;let distance=dot(delta,delta);if distance>16.0{continue;}
   let count=min(found,64u);found++;
   var slot=0u;for(;slot<count;slot++){let other=contacts.entries[slot];if distance<other.distance||(distance==other.distance&&id<other.id){break;}}
   if slot<64u{
    let end=min(count,63u);for(var j=end;j>slot;j--){contacts.entries[j]=contacts.entries[j-1u];}
    contacts.entries[slot]=Contact(vec4(a.pose.xyz,bodyRadius(a.info.x)),a.motion.xy,id,distance);
   }
  }
 }}
 contacts.info.z=min(found,64u);contacts.info.w=found-min(found,64u)+gridOverflow;
}
`;

export function decodeBattleParticipantContacts(
  raw: ArrayBuffer,
  population: number,
): Omit<BattleParticipantContacts, "serial" | "sampledAt"> {
  if (raw.byteLength !== BATTLE_PARTICIPANT_CONTACT_BYTES)
    throw new Error("Incomplete battle contact sample.");
  const words = new Uint32Array(raw),
    values = new Float32Array(raw);
  if (words[2] > 64) throw new Error("Too many battle contacts.");
  const finite = (n: number, bound: number) =>
    Number.isFinite(n) && Math.abs(n) <= bound;
  const position = Object.freeze([
    values[4],
    values[5],
    values[6],
  ]) as readonly [number, number, number];
  if (!position.every((n) => finite(n, 1e6)))
    throw new Error("Invalid battle contact origin.");
  const seen = new Set<number>();
  const contacts = Array.from({ length: words[2] }, (_, i) => {
    const offset = 8 + i * 8,
      id = words[offset + 6],
      radius = values[offset + 3];
    const position = Object.freeze([
      values[offset],
      values[offset + 1],
      values[offset + 2],
    ]) as readonly [number, number, number];
    const velocity = Object.freeze([
      values[offset + 4],
      values[offset + 5],
    ]) as readonly [number, number];
    if (
      id >= population ||
      seen.has(id) ||
      !position.every((n) => finite(n, 1e6)) ||
      !velocity.every((n) => finite(n, 64)) ||
      !BATTLE_BODY_RADII.some((n) => Math.abs(n - radius) < 1e-5)
    )
      throw new Error("Invalid battle body contact.");
    seen.add(id);
    return Object.freeze({ id, position, radius, velocity });
  });
  return Object.freeze({
    epoch: words[0],
    tick: words[1],
    position,
    overflow: words[3],
    contacts: Object.freeze(contacts),
  });
}

interface Slot {
  buffer: GPUBuffer;
  busy: boolean;
  pending: boolean;
  generation: number;
  population: number;
  epoch: number;
  sequence: number;
  sampledAt: number;
}

/** Three small staging buffers keep readback outside the render loop. */
export class BattleParticipantContactSampler {
  readonly bytes = BATTLE_PARTICIPANT_CONTACT_BYTES * 4;
  private latest: BattleParticipantContacts | null = null;
  private generation = 0;
  private serial = 0;
  private querySequence = 0;
  private acceptedSequence = 0;
  private disposed = false;
  private active = false;
  private constructor(
    private output: GPUBuffer,
    private slots: Slot[],
    private pipeline: GPUComputePipeline,
    private groups: GPUBindGroup[],
  ) {}
  static async create(
    device: GPUDevice,
    states: readonly GPUBuffer[],
    cells: GPUBuffer,
    combat: GPUBuffer,
    counters: GPUBuffer,
    uniform: GPUBuffer,
  ) {
    const buffers: GPUBuffer[] = [];
    try {
      const output = device.createBuffer({
        label: "Battle crowd: nearby participant bodies",
        size: BATTLE_PARTICIPANT_CONTACT_BYTES,
        usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_SRC,
      });
      buffers.push(output);
      const slots = Array.from({ length: 3 }, () => {
        const buffer = device.createBuffer({
          label: "Battle crowd: asynchronous body contact readback",
          size: BATTLE_PARTICIPANT_CONTACT_BYTES,
          usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
        });
        buffers.push(buffer);
        return {
          buffer,
          busy: false,
          pending: false,
          generation: 0,
          population: 0,
          epoch: 0,
          sequence: 0,
          sampledAt: 0,
        };
      });
      const layout = device.createBindGroupLayout({
        entries: [
          "read-only-storage",
          "storage",
          "storage",
          "read-only-storage",
          "storage",
          "uniform",
        ].map((type, binding) => ({
          binding,
          visibility: GPUShaderStage.COMPUTE,
          buffer: { type: type as GPUBufferBindingType },
        })),
      });
      const pipeline = await device.createComputePipelineAsync({
        label: "Battle crowd: participant body contacts",
        layout: device.createPipelineLayout({ bindGroupLayouts: [layout] }),
        compute: {
          module: device.createShaderModule({
            code: BATTLE_PARTICIPANT_CONTACTS_WGSL,
          }),
          entryPoint: "participantContacts",
        },
      });
      const groups = states.map((state) =>
        device.createBindGroup({
          layout,
          entries: [state, cells, combat, counters, output, uniform].map(
            (buffer, binding) => ({ binding, resource: { buffer } }),
          ),
        }),
      );
      return new BattleParticipantContactSampler(
        output,
        slots,
        pipeline,
        groups,
      );
    } catch (error) {
      buffers.forEach((buffer) => buffer.destroy());
      throw error;
    }
  }
  invalidate() {
    this.generation++;
    this.latest = null;
  }
  read(): BattleParticipantContacts | null {
    return this.disposed ? null : this.latest;
  }
  prepare(
    encoder: GPUCommandEncoder,
    front: number,
    active: boolean,
    population: number,
    epoch: number,
  ) {
    if (this.disposed) return;
    if (!active) {
      if (this.active) this.invalidate();
      this.active = false;
      return;
    }
    this.active = true;
    const slot = this.slots.find((slot) => !slot.busy && !slot.pending);
    if (!slot) return;
    const pass = encoder.beginComputePass({
      label: "Battle crowd: nearby participant body query",
    });
    pass.setPipeline(this.pipeline);
    pass.setBindGroup(0, this.groups[front]);
    pass.dispatchWorkgroups(1);
    pass.end();
    slot.pending = true;
    slot.generation = this.generation;
    slot.population = population;
    slot.epoch = epoch;
    slot.sequence = ++this.querySequence;
    slot.sampledAt = performance.now();
    encoder.copyBufferToBuffer(
      this.output,
      0,
      slot.buffer,
      0,
      BATTLE_PARTICIPANT_CONTACT_BYTES,
    );
  }
  afterSubmit() {
    if (this.disposed) return;
    for (const slot of this.slots)
      if (slot.pending) {
        slot.pending = false;
        slot.busy = true;
        void slot.buffer
          .mapAsync(GPUMapMode.READ)
          .then(() => {
            try {
              if (!this.disposed && slot.generation === this.generation) {
                const sample = decodeBattleParticipantContacts(
                  slot.buffer.getMappedRange().slice(0),
                  slot.population,
                );
                if (
                  sample.epoch === slot.epoch &&
                  slot.sequence > this.acceptedSequence &&
                  (!this.latest || sample.tick >= this.latest.tick)
                ) {
                  this.acceptedSequence = slot.sequence;
                  this.latest = Object.freeze({
                    ...sample,
                    serial: ++this.serial,
                    sampledAt: slot.sampledAt,
                  });
                }
              }
            } finally {
              slot.buffer.unmap();
            }
          })
          .catch(() => {
            if (
              !this.disposed &&
              slot.generation === this.generation &&
              slot.sequence > this.acceptedSequence
            )
              this.latest = null;
          })
          .finally(() => {
            slot.busy = false;
          });
      }
  }
  dispose() {
    if (this.disposed) return;
    this.disposed = true;
    this.invalidate();
    this.output.destroy();
    this.slots.forEach((slot) => slot.buffer.destroy());
  }
}
