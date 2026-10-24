import type { FractureChunk } from "./source-fracture.ts";
import type { V3 } from "./math.ts";

const RECORD_BYTES = 64;
const JOB_BYTES = 64;
const WORKGROUP = 64;
const MAX_JOBS = 32768;
const ARENA_LIMIT = 160 * 1024 * 1024;
const PAGE_BYTES = 32 * 1024 * 1024;
export const DEBRIS_PACKING_WGSL = `
struct Job { ranges:vec4<u32>, x:vec4<f32>, y:vec4<f32>, z:vec4<f32> }
@group(0) @binding(0) var<storage,read> source:array<u32>;
@group(0) @binding(1) var<storage,read> jobs:array<Job>;
@group(0) @binding(2) var<storage,read_write> vertices:array<u32>;
@group(0) @binding(3) var<storage,read_write> colors:array<u32>;
@group(0) @binding(4) var<storage,read_write> uv1:array<u32>;
@compute @workgroup_size(64) fn pack(@builtin(workgroup_id) group:vec3<u32>, @builtin(local_invocation_index) lane:u32) {
 let job=jobs[group.x]; if(lane>=job.ranges.z){return;}
 let src=(job.ranges.x+lane)*16u;let dst=job.ranges.y+lane;
 for(var a=0u;a<12u;a++){vertices[dst*12u+a]=source[src+a];}
 colors[dst]=source[src+12u];uv1[dst*2u]=source[src+13u];uv1[dst*2u+1u]=source[src+14u];
 if(job.ranges.w!=0u){
  for(var a=0u;a<3u;a++){
   var row=job.x;if(a==1u){row=job.y;}if(a==2u){row=job.z;}
   let x=bitcast<f32>(source[src]);let y=bitcast<f32>(source[src+1u]);let z=bitcast<f32>(source[src+2u]);
   vertices[dst*12u+a]=bitcast<u32>(((row.x*x+row.y*y)+row.z*z)+row.w);
   for(var field=3u;field<=8u;field+=5u){
    let nx=bitcast<f32>(source[src+field]);let ny=bitcast<f32>(source[src+field+1u]);let nz=bitcast<f32>(source[src+field+2u]);
    vertices[dst*12u+field+a]=bitcast<u32>((row.x*nx+row.y*ny)+row.z*nz);
   }
  }
 }
}`;
export interface PackingPart {
  chunk: FractureChunk;
  instance: number;
  world: boolean;
  transform?: Float32Array;
}
export interface PackingTarget {
  vertices: GPUBuffer;
  colors: GPUBuffer;
  uv1: GPUBuffer;
}
export interface PackingDraw {
  material: number;
  firstIndex: number;
  indexCount: number;
  instance: number;
}
export interface PackingBounds {
  min: V3;
  max: V3;
}
interface Page {
  buffer: GPUBuffer;
  free: { start: number; count: number }[];
  live: number;
}
interface Entry {
  chunk: FractureChunk;
  world: boolean;
  page: Page;
  start: number;
  count: number;
  bounds: PackingBounds;
  ready: boolean;
}
interface Run {
  entry: Entry;
  first: number;
  count: number;
  destination: number;
  transform?: Float32Array;
}
interface Plan {
  target: PackingTarget;
  runs: Run[];
  jobs: number;
}

/** CPU reference for the GPU's explicitly f32 rubble arithmetic; copied attributes retain their bits. */
export function transformPackingRecord(
  record: Float32Array,
  matrix: Float32Array,
): Float32Array {
  const out = record.slice(),
    f = Math.fround;
  for (const attribute of [0, 3, 8])
    for (let axis = 0; axis < 3; axis++) {
      let value = f(
        f(
          f(matrix[axis] * record[attribute]) +
            f(matrix[axis + 4] * record[attribute + 1]),
        ) + f(matrix[axis + 8] * record[attribute + 2]),
      );
      if (attribute === 0) value = f(value + matrix[axis + 12]);
      out[attribute + axis] = value;
    }
  return out;
}

/** Prepared geometry is resident; an impact uploads bounded draw jobs instead of all vertices. */
export class DebrisGpuPacking {
  private pages: Page[] = [];
  private entries = new Map<FractureChunk, Map<boolean, Entry>>();
  private desired = new Set<Entry>();
  private pending = new Map<GPUBuffer, Plan>();
  private encoded = new Map<GPUBuffer, Plan>();
  private jobs: GPUBuffer;
  private scratch = new ArrayBuffer(MAX_JOBS * JOB_BYTES);
  private closed = false;
  private arenaBytes = 0;
  private counters = {
    preparedBytes: 0,
    descriptorBytes: 0,
    encodes: 0,
    dispatches: 0,
    copiedVertices: 0,
    transformedVertices: 0,
    cpuFallbacks: 0,
  };
  private constructor(
    private device: GPUDevice,
    private pipeline: GPUComputePipeline,
    private account: (delta: number) => void,
  ) {
    this.jobs = device.createBuffer({
      label: "Debris packing jobs",
      size: this.scratch.byteLength,
      usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
    });
    account(this.jobs.size);
  }
  static async create(
    device: GPUDevice,
    account: (delta: number) => void = () => {},
  ): Promise<DebrisGpuPacking> {
    const module = device.createShaderModule({ code: DEBRIS_PACKING_WGSL });
    const info = await module.getCompilationInfo?.();
    const errors = info?.messages.filter((message) => message.type === "error");
    if (errors?.length)
      throw new Error("Prepared debris graphics could not start.", {
        cause: errors.map((message) => ({
          line: message.lineNum,
          column: message.linePos,
          message: message.message,
        })),
      });
    const pipeline = await device.createComputePipelineAsync({
      label: "Prepared debris scatter",
      layout: "auto",
      compute: { module, entryPoint: "pack" },
    });
    return new DebrisGpuPacking(device, pipeline, account);
  }
  private open() {
    if (this.closed) throw new Error("The debris packing arena has closed.");
  }
  private allocate(count: number): { page: Page; start: number } {
    for (const page of this.pages) {
      const block = page.free.find((range) => range.count >= count);
      if (block) {
        const start = block.start;
        block.start += count;
        block.count -= count;
        page.live++;
        return { page, start };
      }
    }
    const limit = Math.min(
      PAGE_BYTES,
      this.device.limits.maxStorageBufferBindingSize,
      this.device.limits.maxBufferSize,
    );
    const size = Math.floor(limit / RECORD_BYTES) * RECORD_BYTES;
    if (count * RECORD_BYTES > size || this.arenaBytes + size > ARENA_LIMIT)
      throw new Error(
        "Prepared debris exceeds its 160 MiB GPU geometry arena.",
      );
    const page = {
      buffer: this.device.createBuffer({
        label: "Prepared debris geometry",
        size,
        usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
      }),
      free: [{ start: count, count: size / RECORD_BYTES - count }],
      live: 1,
    };
    this.pages.push(page);
    this.arenaBytes += size;
    this.account(size);
    return { page, start: 0 };
  }
  private release(entry: Entry) {
    const variants = this.entries.get(entry.chunk);
    if (variants?.get(entry.world) === entry) variants.delete(entry.world);
    if (!variants?.size) this.entries.delete(entry.chunk);
    const page = entry.page;
    page.live--;
    page.free.push({ start: entry.start, count: entry.count });
    page.free.sort((a, b) => a.start - b.start);
    for (let i = 1; i < page.free.length;) {
      const before = page.free[i - 1],
        next = page.free[i];
      if (before.start + before.count === next.start) {
        before.count += next.count;
        page.free.splice(i, 1);
      } else i++;
    }
    if (!page.live) {
      page.buffer.destroy();
      this.pages.splice(this.pages.indexOf(page), 1);
      this.arenaBytes -= page.buffer.size;
      this.account(-page.buffer.size);
    }
  }
  private trim() {
    const held = new Set<Entry>(this.desired);
    for (const plan of this.pending.values())
      for (const run of plan.runs) held.add(run.entry);
    for (const variants of [...this.entries.values()])
      for (const entry of [...variants.values()])
        if (!held.has(entry)) this.release(entry);
  }
  /** Every new chunk yields between at most 1,024 vertex records. Failed preparation releases only its candidates. */
  *prepare(
    chunks: readonly { chunk: FractureChunk; world: boolean }[],
    cancelled: () => boolean,
  ): Generator<void, boolean> {
    this.open();
    const next = new Set<Entry>(),
      created: Entry[] = [];
    let accepted = false;
    try {
      for (const { chunk, world } of chunks) {
        if (cancelled()) return false;
        if (!chunk.vertices.length) continue;
        let entry = this.entries.get(chunk)?.get(world);
        if (!entry?.ready) {
          const count = chunk.vertices.length / 12;
          if (
            !Number.isSafeInteger(count) ||
            count < 0 ||
            !chunk.center.every(Number.isFinite) ||
            chunk.colors.length !== count * 4 ||
            chunk.uv1.length !== count * 2
          )
            throw new Error("Prepared debris needs complete vertex records.");
          const allocation = this.allocate(count);
          entry = {
            chunk,
            world,
            ...allocation,
            count,
            bounds: {
              min: [Infinity, Infinity, Infinity],
              max: [-Infinity, -Infinity, -Infinity],
            },
            ready: false,
          };
          let variants = this.entries.get(chunk);
          if (!variants) {
            variants = new Map();
            this.entries.set(chunk, variants);
          }
          variants.set(world, entry);
          created.push(entry);
          const temp = new ArrayBuffer(Math.min(1024, count) * RECORD_BYTES),
            words = new Uint32Array(temp),
            floats = new Float32Array(temp),
            colors = new DataView(
              chunk.colors.buffer,
              chunk.colors.byteOffset,
              chunk.colors.byteLength,
            );
          for (let first = 0; first < count; first += 1024) {
            if (cancelled()) return false;
            const length = Math.min(1024, count - first);
            for (let vertex = 0; vertex < length; vertex++) {
              const src = (first + vertex) * 12,
                dst = vertex * 16;
              for (let a = 0; a < 12; a++)
                floats[dst + a] = chunk.vertices[src + a];
              for (let axis = 0; axis < 3; axis++) {
                if (world) floats[dst + axis] += chunk.center[axis];
                const value = floats[dst + axis];
                entry.bounds.min[axis] = Math.min(
                  entry.bounds.min[axis],
                  value,
                );
                entry.bounds.max[axis] = Math.max(
                  entry.bounds.max[axis],
                  value,
                );
              }
              words[dst + 12] = colors.getUint32((first + vertex) * 4, true);
              floats[dst + 13] = chunk.uv1[(first + vertex) * 2];
              floats[dst + 14] = chunk.uv1[(first + vertex) * 2 + 1];
            }
            this.device.queue.writeBuffer(
              entry.page.buffer,
              (entry.start + first) * RECORD_BYTES,
              temp,
              0,
              length * RECORD_BYTES,
            );
            this.counters.preparedBytes += length * RECORD_BYTES;
            yield;
          }
          entry.ready = true;
        }
        next.add(entry);
      }
      if (cancelled()) return false;
      this.desired = next;
      accepted = true;
      this.trim();
      return true;
    } finally {
      if (!accepted) for (const entry of created) this.release(entry);
    }
  }
  /** Returns undefined only for an unsupported job count; the established CPU path remains available. */
  stage(
    parts: readonly PackingPart[],
    target: PackingTarget,
    budget: number,
  ): { draws: PackingDraw[]; bounds?: PackingBounds } | undefined {
    this.open();
    const grouped = new Map<
      number,
      {
        part: PackingPart;
        draw: FractureChunk["draws"][number];
        entry: Entry;
      }[]
    >();
    let count = 0,
      jobs = 0;
    for (const part of parts) {
      if (!part.chunk.vertices.length) continue;
      const entry = this.entries.get(part.chunk)?.get(part.world);
      if (!entry?.ready)
        throw new Error(
          "Prepare all debris geometry before changing its collision.",
        );
      for (const draw of part.chunk.draws) {
        if (
          !Number.isSafeInteger(draw.firstIndex) ||
          draw.firstIndex < 0 ||
          !Number.isSafeInteger(draw.indexCount) ||
          draw.indexCount < 0 ||
          draw.firstIndex + draw.indexCount > entry.count
        )
          throw new Error(
            "Debris draw ranges must stay within their prepared records.",
          );
        count += draw.indexCount;
        jobs += Math.ceil(draw.indexCount / WORKGROUP);
        const list = grouped.get(draw.material) ?? [];
        list.push({ part, draw, entry });
        grouped.set(draw.material, list);
      }
    }
    if (!Number.isSafeInteger(budget) || budget < 0 || count > budget)
      throw new Error("Prepared fragments exceeded their reserved GPU budget.");
    const outputs = [target.vertices, target.colors, target.uv1];
    if (
      new Set(outputs).size !== 3 ||
      outputs.some(
        (buffer) =>
          buffer === this.jobs ||
          this.pages.some((page) => page.buffer === buffer),
      )
    )
      throw new Error(
        "Debris packing requires separate source, job, and output buffers.",
      );
    for (const plan of this.pending.values())
      if (
        plan.target.vertices !== target.vertices &&
        outputs.some(
          (buffer) =>
            buffer === plan.target.vertices ||
            buffer === plan.target.colors ||
            buffer === plan.target.uv1,
        )
      )
        throw new Error("Pending debris slabs cannot share output buffers.");
    if (
      target.vertices.size < count * 48 ||
      target.colors.size < count * 4 ||
      target.uv1.size < count * 8
    )
      throw new Error(
        "Debris output buffers cannot hold the complete prepared geometry.",
      );
    if (!this.pending.has(target.vertices) && this.pending.size >= 3)
      throw new Error("Debris packing supports three pending output slabs.");
    const otherJobs = [...this.pending.values()].reduce(
      (total, plan) =>
        total + (plan.target.vertices === target.vertices ? 0 : plan.jobs),
      0,
    );
    const alignment =
      this.device.limits.minStorageBufferOffsetAlignment / JOB_BYTES;
    const padding = this.pages.length * 3 * Math.max(0, alignment - 1);
    if (jobs + otherJobs + padding > MAX_JOBS) {
      this.pending.delete(target.vertices);
      this.counters.cpuFallbacks++;
      return undefined;
    }
    const boundedParts = new Set<PackingPart>();
    const draws: PackingDraw[] = [],
      runs: Run[] = [],
      min: V3 = [Infinity, Infinity, Infinity],
      max: V3 = [-Infinity, -Infinity, -Infinity];
    let destination = 0;
    for (const [material, items] of grouped)
      for (const { part, draw, entry } of items) {
        runs.push({
          entry,
          first: draw.firstIndex,
          count: draw.indexCount,
          destination,
          transform: part.transform,
        });
        const previous = draws.at(-1);
        if (
          previous &&
          previous.material === material &&
          previous.instance === part.instance
        )
          previous.indexCount += draw.indexCount;
        else
          draws.push({
            material,
            firstIndex: destination,
            indexCount: draw.indexCount,
            instance: part.instance,
          });
        destination += draw.indexCount;
        // Whole chunk boxes overestimate a material subset. Outward error margin also covers GPU f32 transforms.
        if (boundedParts.has(part)) continue;
        boundedParts.add(part);
        const bounds = entry.bounds,
          m = part.transform;
        for (let corner = 0; corner < (m ? 8 : 1); corner++)
          for (let axis = 0; axis < 3; axis++) {
            let low = bounds.min[axis],
              high = bounds.max[axis];
            if (m) {
              const x = (corner & 1 ? bounds.max : bounds.min)[0],
                y = (corner & 2 ? bounds.max : bounds.min)[1],
                z = (corner & 4 ? bounds.max : bounds.min)[2];
              low = high =
                m[axis] * x + m[axis + 4] * y + m[axis + 8] * z + m[axis + 12];
            }
            const epsilon = 1e-5 * Math.max(1, Math.abs(low), Math.abs(high));
            min[axis] = Math.min(min[axis], low - epsilon);
            max[axis] = Math.max(max[axis], high + epsilon);
          }
      }
    this.pending.set(target.vertices, { target, runs, jobs });
    return { draws, ...(count ? { bounds: { min, max } } : {}) };
  }
  encode(encoder: GPUCommandEncoder): void {
    this.open();
    this.encoded.clear();
    if (!this.pending.size) return;
    const batches: { page: Page; plan: Plan; runs: Run[] }[] = [];
    for (const plan of this.pending.values()) {
      const byPage = new Map<Page, Run[]>();
      for (const run of plan.runs) {
        const list = byPage.get(run.entry.page) ?? [];
        list.push(run);
        byPage.set(run.entry.page, list);
      }
      for (const [page, runs] of byPage) batches.push({ page, plan, runs });
      this.encoded.set(plan.target.vertices, plan);
    }
    const words = new Uint32Array(this.scratch),
      floats = new Float32Array(this.scratch),
      alignment =
        this.device.limits.minStorageBufferOffsetAlignment / JOB_BYTES;
    let cursor = 0;
    const dispatches: {
      batch: (typeof batches)[number];
      first: number;
      count: number;
    }[] = [];
    for (const batch of batches) {
      cursor = Math.ceil(cursor / alignment) * alignment;
      const first = cursor;
      for (const run of batch.runs)
        for (let offset = 0; offset < run.count; offset += WORKGROUP) {
          if (cursor >= MAX_JOBS)
            throw new Error("Debris packing exceeded its bounded job table.");
          const at = cursor * 16;
          words[at] = run.entry.start + run.first + offset;
          words[at + 1] = run.destination + offset;
          words[at + 2] = Math.min(WORKGROUP, run.count - offset);
          words[at + 3] = Number(!!run.transform);
          const m = run.transform;
          if (m)
            for (let axis = 0; axis < 3; axis++)
              for (let column = 0; column < 4; column++)
                floats[at + 4 + axis * 4 + column] = m[axis + column * 4];
          cursor++;
        }
      dispatches.push({ batch, first, count: cursor - first });
    }
    if (cursor) {
      this.device.queue.writeBuffer(
        this.jobs,
        0,
        this.scratch,
        0,
        cursor * JOB_BYTES,
      );
      this.counters.descriptorBytes += cursor * JOB_BYTES;
      const pass = encoder.beginComputePass({ label: "Pack prepared debris" });
      pass.setPipeline(this.pipeline);
      for (const { batch, first, count } of dispatches) {
        const target = batch.plan.target;
        const group = this.device.createBindGroup({
          layout: this.pipeline.getBindGroupLayout(0),
          entries: [
            { binding: 0, resource: { buffer: batch.page.buffer } },
            {
              binding: 1,
              resource: {
                buffer: this.jobs,
                offset: first * JOB_BYTES,
                size: count * JOB_BYTES,
              },
            },
            { binding: 2, resource: { buffer: target.vertices } },
            { binding: 3, resource: { buffer: target.colors } },
            { binding: 4, resource: { buffer: target.uv1 } },
          ],
        });
        pass.setBindGroup(0, group);
        pass.dispatchWorkgroups(count);
        this.counters.dispatches++;
      }
      pass.end();
      this.counters.encodes++;
      for (const plan of this.encoded.values())
        for (const run of plan.runs)
          this.counters[
            run.transform ? "transformedVertices" : "copiedVertices"
          ] += run.count;
    }
  }
  discardStaged(): void {
    this.open();
    this.pending.clear();
    this.encoded.clear();
  }
  retain(chunks: readonly { chunk: FractureChunk; world: boolean }[]): void {
    this.open();
    this.pending.clear();
    this.encoded.clear();
    this.desired = new Set();
    for (const { chunk, world } of chunks) {
      const entry = this.entries.get(chunk)?.get(world);
      if (entry?.ready) this.desired.add(entry);
    }
    this.trim();
  }
  submitted(): void {
    for (const [key, plan] of this.encoded)
      if (this.pending.get(key) === plan) this.pending.delete(key);
    this.encoded.clear();
    this.trim();
  }
  snapshot() {
    return {
      backend: "gpu",
      arenaBytes: this.arenaBytes,
      descriptorBytesAllocated: this.jobs.size,
      cpuScratchBytes: this.scratch.byteLength,
      residentChunks: [...this.entries.values()].reduce(
        (n, v) => n + v.size,
        0,
      ),
      pendingTargets: this.pending.size,
      nativePipelineBytes: null,
      ...this.counters,
    };
  }
  dispose() {
    if (this.closed) return;
    this.closed = true;
    for (const page of this.pages) {
      page.buffer.destroy();
      this.account(-page.buffer.size);
    }
    this.jobs.destroy();
    this.account(-this.jobs.size);
    this.pages = [];
    this.arenaBytes = 0;
    this.entries.clear();
    this.pending.clear();
    this.encoded.clear();
    this.desired.clear();
  }
}
