import { BOID_STRIDE, type BoidSimulation } from "./simulation.ts";

const WGSL = /* wgsl */ `
struct Boid { position: vec4f, velocity: vec4f }
struct Params { dimensions: vec4u, world: vec4f, steering: vec4f, avoidance: vec4f, goal: vec4f, obstacle: vec4f }
@group(0) @binding(0) var<storage,read> source: array<Boid>;
@group(0) @binding(1) var<storage,read_write> result: array<Boid>;
@group(0) @binding(2) var<storage,read_write> grid: array<atomic<i32>>;
@group(0) @binding(3) var<uniform> p: Params;
fn cellFor(v: vec3f) -> u32 {
  let c = vec3u(clamp(floor((v+vec3f(p.world.y))/p.world.z),vec3f(0),vec3f(f32(p.dimensions.y-1u))));
  return (c.x+c.y*p.dimensions.y+c.z*p.dimensions.y*p.dimensions.y)*8u;
}
fn safeNormal(v: vec3f) -> vec3f { let d=dot(v,v); if(d>1e-12){return v*inverseSqrt(d);} return vec3f(0); }
@compute @workgroup_size(128) fn clear(@builtin(global_invocation_id) id: vec3u) {
  if(id.x<p.dimensions.z*8u){atomicStore(&grid[id.x],0);}
}
@compute @workgroup_size(128) fn scatter(@builtin(global_invocation_id) id: vec3u) {
  if(id.x>=p.dimensions.x){return;}
  let b=source[id.x]; let c=cellFor(b.position.xyz);
  atomicAdd(&grid[c],1);
  atomicAdd(&grid[c+1u],i32(b.position.x*128.0));
  atomicAdd(&grid[c+2u],i32(b.position.y*128.0));
  atomicAdd(&grid[c+3u],i32(b.position.z*128.0));
  atomicAdd(&grid[c+4u],i32(b.velocity.x*128.0));
  atomicAdd(&grid[c+5u],i32(b.velocity.y*128.0));
  atomicAdd(&grid[c+6u],i32(b.velocity.z*128.0));
}
@compute @workgroup_size(128) fn update(@builtin(global_invocation_id) id: vec3u) {
  if(id.x>=p.dimensions.x){return;}
  let b=source[id.x]; let pos=b.position.xyz; let f=b.velocity.xyz/p.world.w;
  let c=cellFor(pos); let count=atomicLoad(&grid[c]);
  let average=1.0/(f32(count)*128.0);
  let meanP=vec3f(f32(atomicLoad(&grid[c+1u])),f32(atomicLoad(&grid[c+2u])),f32(atomicLoad(&grid[c+3u])))*average;
  let meanV=vec3f(f32(atomicLoad(&grid[c+4u])),f32(atomicLoad(&grid[c+5u])),f32(atomicLoad(&grid[c+6u])))*average;
  var separation=vec3f(0); if(count>1){separation=safeNormal(pos-meanP);}
  let desired=safeNormal(p.goal.xyz-pos);
  let bound=max(0.0,(length(pos)/p.world.y-0.78)/0.22)*3.0/p.world.y;
  var obstacle=pos-p.obstacle.xyz; let distance2=dot(obstacle,obstacle); var scale=0.0;
  if(distance2<p.avoidance.y*p.avoidance.y){
    if(distance2<=1e-12){obstacle=vec3f(1,0,0);scale=p.avoidance.x;}
    else {let d=sqrt(distance2);scale=p.avoidance.x*(1.0-d/p.avoidance.y)/d;}
  }
  var heading=f+p.world.x*p.steering.x*(p.steering.y*(meanV/p.world.w-f)+p.steering.z*separation+p.steering.w*desired-pos*bound+obstacle*scale);
  if(dot(heading,heading)<=1e-12){heading=f;if(dot(heading,heading)<=1e-12){heading=vec3f(1,0,0);}}
  var velocity=normalize(heading)*p.world.w; var next=pos+velocity*p.world.x;
  if(dot(next,next)>p.world.y*p.world.y){let n=normalize(next);next=n*p.world.y*0.999;velocity-=2.0*max(0.0,dot(velocity,n))*n;}
  result[id.x]=Boid(vec4f(next,0),vec4f(velocity,0));
}
`;

interface QuerySlot {
  query: GPUQuerySet;
  resolve: GPUBuffer;
  read: GPUBuffer;
  busy: boolean;
  generation: number;
}

/** Persistent double-buffered state and order-independent integer cell reduction. */
export class GpuBoidSimulation {
  readonly states: readonly [GPUBuffer, GPUBuffer];
  readonly bytes: number;
  private grid: GPUBuffer;
  private uniform: GPUBuffer;
  private bindings: GPUBindGroup[];
  private pipelines: GPUComputePipeline[];
  private front = 0;
  private params = new Float32Array(24);
  private dimensions = new Uint32Array(this.params.buffer);
  private slots: QuerySlot[] = [];
  private pending: QuerySlot | undefined;
  private serial = 0;
  private generation = 0;
  private disposed = false;
  lastSample: { serial: number; ms: number } | null = null;

  constructor(
    readonly device: GPUDevice,
    readonly simulation: BoidSimulation,
    options: { timestamps?: boolean } = {},
  ) {
    const stateBytes = simulation.current.byteLength;
    this.states = [0, 1].map((i) =>
      device.createBuffer({
        label: `boids-state-${i}`,
        size: stateBytes,
        usage:
          GPUBufferUsage.STORAGE |
          GPUBufferUsage.COPY_DST |
          GPUBufferUsage.COPY_SRC,
      }),
    ) as unknown as [GPUBuffer, GPUBuffer];
    this.grid = device.createBuffer({
      label: "boids-integer-cells",
      size: simulation.cellCount * 32,
      usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_SRC,
    });
    this.uniform = device.createBuffer({
      size: 96,
      usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
    });
    this.bytes = stateBytes * 2 + simulation.cellCount * 32 + 96;
    const module = device.createShaderModule({
      label: "Aethyrforge cell-aggregate boids",
      code: WGSL,
    });
    const layout = device.createBindGroupLayout({
      entries: [
        {
          binding: 0,
          visibility: GPUShaderStage.COMPUTE,
          buffer: { type: "read-only-storage" },
        },
        {
          binding: 1,
          visibility: GPUShaderStage.COMPUTE,
          buffer: { type: "storage" },
        },
        {
          binding: 2,
          visibility: GPUShaderStage.COMPUTE,
          buffer: { type: "storage" },
        },
        {
          binding: 3,
          visibility: GPUShaderStage.COMPUTE,
          buffer: { type: "uniform" },
        },
      ],
    });
    const pipelineLayout = device.createPipelineLayout({
      bindGroupLayouts: [layout],
    });
    this.pipelines = ["clear", "scatter", "update"].map((entryPoint) =>
      device.createComputePipeline({
        layout: pipelineLayout,
        compute: { module, entryPoint },
      }),
    );
    this.bindings = [0, 1].map((i) =>
      device.createBindGroup({
        layout,
        entries: [
          { binding: 0, resource: { buffer: this.states[i] } },
          { binding: 1, resource: { buffer: this.states[1 - i] } },
          { binding: 2, resource: { buffer: this.grid } },
          { binding: 3, resource: { buffer: this.uniform } },
        ],
      }),
    );
    if (options.timestamps !== false && device.features.has("timestamp-query"))
      for (let i = 0; i < 4; i++)
        this.slots.push({
          query: device.createQuerySet({ type: "timestamp", count: 2 }),
          resolve: device.createBuffer({
            size: 16,
            usage: GPUBufferUsage.QUERY_RESOLVE | GPUBufferUsage.COPY_SRC,
          }),
          read: device.createBuffer({
            size: 16,
            usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
          }),
          busy: false,
          generation: 0,
        });
    this.reset();
  }
  get current(): GPUBuffer {
    return this.states[this.front];
  }
  reset(): void {
    if (this.disposed) throw new Error("The boids simulation is closed.");
    this.generation++;
    for (const state of this.states)
      this.device.queue.writeBuffer(
        state,
        0,
        this.simulation.current as Float32Array<ArrayBuffer>,
      );
    this.front = 0;
    this.lastSample = null;
  }
  encode(
    encoder: GPUCommandEncoder,
    dt: number,
    target: readonly number[],
    obstacle: readonly number[],
  ): void {
    if (this.disposed) throw new Error("The boids simulation is closed.");
    if (this.pending)
      throw new Error(
        "Submit the preceding simulation step before encoding another.",
      );
    if (
      !Number.isFinite(dt) ||
      dt <= 0 ||
      dt > 0.1 ||
      target.length !== 3 ||
      obstacle.length !== 3 ||
      [...target, ...obstacle].some(
        (v) => !Number.isFinite(v) || Math.abs(v) > 1024,
      )
    )
      throw new Error("Choose a valid simulation step and target.");
    const c = this.simulation.config,
      u = this.params;
    this.dimensions.set(
      [c.count, this.simulation.gridDimension, this.simulation.cellCount, 0],
      0,
    );
    u.set(
      [
        dt,
        c.radius,
        c.cellSize,
        c.speed,
        c.turnRate,
        c.alignment,
        c.separation,
        c.targetWeight,
        c.obstacleWeight,
        c.obstacleRadius,
        0,
        0,
        ...target,
        0,
        ...obstacle,
        0,
      ],
      4,
    );
    this.device.queue.writeBuffer(
      this.uniform,
      0,
      u as Float32Array<ArrayBuffer>,
    );
    const slot = this.slots.find((s) => !s.busy);
    if (slot) {
      slot.busy = true;
      slot.generation = this.generation;
      this.pending = slot;
    }
    for (let stage = 0; stage < 3; stage++) {
      const pass = encoder.beginComputePass({
        label: ["clear cells", "aggregate boids", "steer boids"][stage],
        timestampWrites:
          slot && stage !== 1
            ? {
                querySet: slot.query,
                ...(stage === 0 ? { beginningOfPassWriteIndex: 0 } : {}),
                ...(stage === 2 ? { endOfPassWriteIndex: 1 } : {}),
              }
            : undefined,
      });
      pass.setPipeline(this.pipelines[stage]);
      pass.setBindGroup(0, this.bindings[this.front]);
      pass.dispatchWorkgroups(
        Math.ceil(
          (stage === 0 ? this.simulation.cellCount * 8 : c.count) / 128,
        ),
      );
      pass.end();
    }
    if (slot) {
      encoder.resolveQuerySet(slot.query, 0, 2, slot.resolve, 0);
      encoder.copyBufferToBuffer(slot.resolve, 0, slot.read, 0, 16);
    }
    this.front ^= 1;
  }
  afterSubmit(): void {
    const slot = this.pending;
    this.pending = undefined;
    if (!slot) return;
    const serial = ++this.serial;
    void slot.read
      .mapAsync(GPUMapMode.READ)
      .then(() => {
        const values = new BigUint64Array(slot.read.getMappedRange());
        const ms = Number(values[1] - values[0]) / 1e6;
        slot.read.unmap();
        if (
          !this.disposed &&
          slot.generation === this.generation &&
          Number.isFinite(ms) &&
          ms >= 0 &&
          (!this.lastSample || serial > this.lastSample.serial)
        )
          this.lastSample = { serial, ms };
      })
      .catch(() => {})
      .finally(() => {
        slot.busy = false;
      });
  }
  async readState(): Promise<Float32Array> {
    if (this.disposed) throw new Error("The boids simulation is closed.");
    const read = this.device.createBuffer({
      size: this.simulation.current.byteLength,
      usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
    });
    try {
      const encoder = this.device.createCommandEncoder();
      encoder.copyBufferToBuffer(this.current, 0, read, 0, read.size);
      this.device.queue.submit([encoder.finish()]);
      await read.mapAsync(GPUMapMode.READ);
      return new Float32Array(read.getMappedRange().slice(0));
    } finally {
      if (read.mapState === "mapped") read.unmap();
      read.destroy();
    }
  }
  async readGrid(): Promise<Int32Array> {
    if (this.disposed) throw new Error("The boids simulation is closed.");
    const read = this.device.createBuffer({
      size: this.grid.size,
      usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
    });
    try {
      const encoder = this.device.createCommandEncoder();
      encoder.copyBufferToBuffer(this.grid, 0, read, 0, read.size);
      this.device.queue.submit([encoder.finish()]);
      await read.mapAsync(GPUMapMode.READ);
      return new Int32Array(read.getMappedRange().slice(0));
    } finally {
      if (read.mapState === "mapped") read.unmap();
      read.destroy();
    }
  }
  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    for (const s of this.states) s.destroy();
    this.grid.destroy();
    this.uniform.destroy();
    for (const s of this.slots) {
      s.query.destroy();
      s.resolve.destroy();
      s.read.destroy();
    }
  }
}
