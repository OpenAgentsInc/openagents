/// <reference types="@webgpu/types" />
// The browser half of `af-gpu`: a handle table and the `af_gpu` imports the
// Rust renderer calls. Descriptors arrive as JSON once, at creation; per-frame
// calls pass integers. Pass commands arrive batched by default: `replay` runs
// a whole pass's commands from Wasm memory in one call (ADR 0004,
// `gpu-batch.ts`); the per-call imports remain for A/B runs.

import { replay } from "./gpu-batch.ts";

type Exports = {
  memory: WebAssembly.Memory;
  af_gpu_alloc(len: number): number;
  af_gpu_read_done(ticket: number, ptr: number, len: number): void;
};

const CREATE = { buffer: 1, texture: 2, sampler: 3, compute: 4, render: 5, bindGroup: 6, querySet: 7 };
// Release codes are af_gpu::Kind + 1; these own GPU memory and are destroyed.
const DESTROY_ON_RELEASE = new Set([1, 2, 10]);

export interface GpuHost {
  imports: Record<string, Function>;
  bind(exports: Exports): void;
  table: unknown[];
  device: GPUDevice;
  context: GPUCanvasContext;
  format: GPUTextureFormat;
  currentTexture: GPUTexture | null;
  errors: string[];
}

/// With `offscreen`, frames render into a texture instead of the canvas. Headless
/// browsers without a working swapchain (CI containers) use it for checks.
export function createGpuHost(
  device: GPUDevice,
  adapter: GPUAdapter,
  context: GPUCanvasContext | null,
  format: GPUTextureFormat,
  offscreen: () => [number, number] | null = () => null,
): GpuHost {
  const table: unknown[] = [null, device];
  const free: number[] = [];
  let exports: Exports | null = null;
  const errors: string[] = [];
  const decoder = new TextDecoder();
  const encoder = new TextEncoder();
  const host = { table, device, context, format, currentTexture: null as GPUTexture | null, errors } as GpuHost;

  device.lost.then((info) => errors.push(`device lost (${info.reason}): ${info.message}`));
  device.addEventListener("uncapturederror", (event) => {
    const message = (event as GPUUncapturedErrorEvent).error.message;
    if (errors.length < 64) errors.push(message);
  });

  const memory = () => new Uint8Array(exports!.memory.buffer);
  const text = (ptr: number, len: number) => decoder.decode(memory().subarray(ptr, ptr + len));
  const json = (ptr: number, len: number) => JSON.parse(text(ptr, len));
  const put = (value: unknown) => {
    const slot = free.pop();
    if (slot !== undefined) {
      table[slot] = value;
      return slot;
    }
    table.push(value);
    return table.length - 1;
  };
  const get = <T>(handle: number) => table[handle] as T;
  const size = (value: number) => (value < 0 ? undefined : value);

  const createRenderPipeline = (d: any): GPURenderPipeline => {
    const module = get<GPUShaderModule>(d.module);
    const descriptor: GPURenderPipelineDescriptor = {
      label: d.label,
      layout: "auto",
      vertex: { module, entryPoint: d.vertexEntry, buffers: d.buffers },
      primitive: { topology: "triangle-list", cullMode: d.cullMode },
      multisample: { count: d.sampleCount },
    };
    if (d.depth) descriptor.depthStencil = d.depth;
    if (d.fragmentEntry) {
      descriptor.fragment = {
        module,
        entryPoint: d.fragmentEntry,
        targets: d.targets.map((t: { format: GPUTextureFormat; blend: boolean }) => ({
          format: t.format,
          blend: t.blend
            ? { color: { srcFactor: "one", dstFactor: "one-minus-src-alpha" }, alpha: { srcFactor: "one", dstFactor: "one-minus-src-alpha" } }
            : undefined,
        })),
      };
    }
    return device.createRenderPipeline(descriptor);
  };

  const create = (kind: number, d: any): unknown => {
    switch (kind) {
      case CREATE.buffer:
        return device.createBuffer({ label: d.label, size: d.size, usage: d.usage });
      case CREATE.texture:
        return device.createTexture({ label: d.label, size: d.size, mipLevelCount: d.mipLevelCount, sampleCount: d.sampleCount, format: d.format, usage: d.usage });
      case CREATE.sampler:
        return device.createSampler(d);
      case CREATE.compute:
        return device.createComputePipeline({ label: d.label, layout: "auto", compute: { module: get(d.module), entryPoint: d.entryPoint } });
      case CREATE.render:
        return createRenderPipeline(d);
      case CREATE.bindGroup:
        return device.createBindGroup({
          label: d.label,
          layout: get(d.layout),
          entries: d.entries.map((e: any) => ({
            binding: e.binding,
            resource:
              e.buffer !== undefined
                ? { buffer: get<GPUBuffer>(e.buffer), offset: e.offset, size: e.size ?? undefined }
                : e.sampler !== undefined
                  ? get<GPUSampler>(e.sampler)
                  : get<GPUTextureView>(e.view),
          })),
        });
      case CREATE.querySet:
        return device.createQuerySet({ type: "timestamp", count: d.count });
    }
    throw new Error(`unknown create kind ${kind}`);
  };

  const timestampWrites = (t: any) =>
    t ? { querySet: get<GPUQuerySet>(t.querySet), beginningOfPassWriteIndex: t.begin, endOfPassWriteIndex: t.end } : undefined;

  host.imports = {
    host_device: () => 1,
    host_info: (out: number, cap: number) => {
      const info = adapter.info;
      const bytes = encoder.encode(
        JSON.stringify({ vendor: info.vendor, architecture: info.architecture, description: info.description, features: [...device.features] }),
      );
      memory().set(bytes.subarray(0, cap), out);
      return Math.min(bytes.length, cap);
    },
    release: (kind: number, handle: number) => {
      const object = table[handle] as { destroy?: () => void } | null;
      if (handle <= 1 || object === null) {
        errors.push(`invalid release of handle ${handle} (kind ${kind})${object === null ? ", already released" : ""}`);
        return;
      }
      if (DESTROY_ON_RELEASE.has(kind)) object?.destroy?.();
      table[handle] = null;
      free.push(handle);
    },
    create: (kind: number, ptr: number, len: number) => put(create(kind, json(ptr, len))),
    create_shader_module: (lp: number, ll: number, cp: number, cl: number) => {
      const module = device.createShaderModule({ label: text(lp, ll), code: text(cp, cl) });
      module.getCompilationInfo().then((info) => {
        for (const m of info.messages) if (m.type === "error") errors.push(`${text(lp, ll)}:${m.lineNum}:${m.linePos} ${m.message}`);
      });
      return put(module);
    },
    get_bind_group_layout: (pipeline: number, _isRender: number, index: number) =>
      put(get<GPURenderPipeline | GPUComputePipeline>(pipeline).getBindGroupLayout(index)),
    create_view: (texture: number, ptr: number, len: number) => {
      const d = json(ptr, len);
      return put(
        get<GPUTexture>(texture).createView({
          dimension: d.dimension,
          baseMipLevel: d.baseMipLevel,
          mipLevelCount: d.mipLevelCount,
          baseArrayLayer: d.baseArrayLayer,
          arrayLayerCount: d.arrayLayerCount,
        }),
      );
    },
    write_buffer: (buffer: number, offset: number, ptr: number, len: number) => {
      device.queue.writeBuffer(get<GPUBuffer>(buffer), offset, exports!.memory.buffer, ptr, len);
    },
    write_texture: (texture: number, mip: number, layer: number, ptr: number, len: number, bytesPerRow: number, w: number, h: number, d: number) => {
      device.queue.writeTexture(
        { texture: get<GPUTexture>(texture), mipLevel: mip, origin: { x: 0, y: 0, z: layer } },
        new Uint8Array(exports!.memory.buffer, ptr, len),
        { bytesPerRow, rowsPerImage: h },
        { width: w, height: h, depthOrArrayLayers: d },
      );
    },
    create_command_encoder: () => put(device.createCommandEncoder()),
    begin_compute_pass: (enc: number, ptr: number, len: number) => {
      const d = json(ptr, len);
      return put(get<GPUCommandEncoder>(enc).beginComputePass({ label: d.label, timestampWrites: timestampWrites(d.timestamps) }));
    },
    begin_render_pass: (enc: number, ptr: number, len: number) => {
      const d = json(ptr, len);
      return put(
        get<GPUCommandEncoder>(enc).beginRenderPass({
          label: d.label,
          colorAttachments: d.color.map((c: any) => ({
            view: get<GPUTextureView>(c.view),
            resolveTarget: c.resolveTarget == null ? undefined : get<GPUTextureView>(c.resolveTarget),
            loadOp: c.clear ? "clear" : "load",
            clearValue: c.clear ?? undefined,
            storeOp: c.store ? "store" : "discard",
          })),
          depthStencilAttachment: d.depth
            ? {
                view: get<GPUTextureView>(d.depth.view),
                depthLoadOp: d.depth.clear == null ? "load" : "clear",
                depthClearValue: d.depth.clear ?? undefined,
                depthStoreOp: d.depth.store ? "store" : "discard",
              }
            : undefined,
          timestampWrites: timestampWrites(d.timestamps),
        }),
      );
    },
    copy_buffer_to_buffer: (enc: number, src: number, so: number, dst: number, d: number, n: number) =>
      get<GPUCommandEncoder>(enc).copyBufferToBuffer(get(src), so, get(dst), d, n),
    clear_buffer: (enc: number, b: number, o: number, n: number) => get<GPUCommandEncoder>(enc).clearBuffer(get(b), o, n),
    resolve_queries: (enc: number, set: number, first: number, count: number, dst: number, offset: number) =>
      get<GPUCommandEncoder>(enc).resolveQuerySet(get(set), first, count, get(dst), offset),
    finish: (enc: number) => {
      const buffer = get<GPUCommandEncoder>(enc).finish();
      table[enc] = null;
      free.push(enc);
      return put(buffer);
    },
    submit: (ptr: number, count: number) => {
      const handles = new Uint32Array(exports!.memory.buffer, ptr, count);
      device.queue.submit(Array.from(handles, (h) => get<GPUCommandBuffer>(h)));
    },
    set_pipeline: (pass: number, p: number) => get<GPURenderPassEncoder>(pass).setPipeline(get(p)),
    set_bind_group: (pass: number, i: number, g: number) => get<GPURenderPassEncoder>(pass).setBindGroup(i, get<GPUBindGroup>(g)),
    dispatch: (pass: number, x: number, y: number, z: number) => get<GPUComputePassEncoder>(pass).dispatchWorkgroups(x, y, z),
    dispatch_indirect: (pass: number, b: number, o: number) => get<GPUComputePassEncoder>(pass).dispatchWorkgroupsIndirect(get(b), o),
    end_pass: (pass: number) => {
      get<GPURenderPassEncoder>(pass).end();
      table[pass] = null;
      free.push(pass);
    },
    set_vertex_buffer: (pass: number, slot: number, b: number, o: number, s: number) =>
      get<GPURenderPassEncoder>(pass).setVertexBuffer(slot, get(b), o, size(s)),
    set_index_buffer: (pass: number, b: number, u32: number, o: number, s: number) =>
      get<GPURenderPassEncoder>(pass).setIndexBuffer(get(b), u32 ? "uint32" : "uint16", o, size(s)),
    draw: (pass: number, v: number, i: number, fv: number, fi: number) => get<GPURenderPassEncoder>(pass).draw(v, i, fv, fi),
    draw_indexed: (pass: number, n: number, i: number, fi: number, bv: number, fin: number) =>
      get<GPURenderPassEncoder>(pass).drawIndexed(n, i, fi, bv, fin),
    draw_indirect: (pass: number, b: number, o: number) => get<GPURenderPassEncoder>(pass).drawIndirect(get(b), o),
    draw_indexed_indirect: (pass: number, b: number, o: number) => get<GPURenderPassEncoder>(pass).drawIndexedIndirect(get(b), o),
    read_buffer: (buffer: number, offset: number, n: number, ticket: number) => {
      const b = get<GPUBuffer>(buffer);
      b.mapAsync(GPUMapMode.READ, offset, n).then(
        () => {
          const bytes = new Uint8Array(b.getMappedRange(offset, n)).slice();
          b.unmap();
          const ptr = exports!.af_gpu_alloc(bytes.length);
          new Uint8Array(exports!.memory.buffer, ptr, bytes.length).set(bytes);
          exports!.af_gpu_read_done(ticket, ptr, bytes.length);
        },
        (error) => errors.push(`read failed: ${error}`),
      );
    },
    surface_view: () => {
      const size = offscreen();
      if (size) {
        const [width, height] = size;
        if (!host.currentTexture || host.currentTexture.width !== width || host.currentTexture.height !== height) {
          host.currentTexture?.destroy();
          host.currentTexture = device.createTexture({ size: [width, height], format, usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.COPY_SRC });
        }
      } else {
        host.currentTexture = context!.getCurrentTexture();
      }
      return put(host.currentTexture.createView());
    },
    surface_format: () => (format === "bgra8unorm" ? 1 : 0),
    take_error: (out: number, cap: number) => {
      const message = errors.shift();
      if (!message) return 0;
      const bytes = encoder.encode(message);
      memory().set(bytes.subarray(0, cap), out);
      return Math.min(bytes.length, cap);
    },
    replay: (pass: number, ptr: number, len: number) => {
      const words = new Uint32Array(exports!.memory.buffer, ptr, len);
      if (replay(table[pass] as GPURenderPassEncoder, words, table)) {
        table[pass] = null;
        free.push(pass);
      }
    },
    now: () => performance.now(),
    log: (ptr: number, len: number) => console.log(text(ptr, len)),
  };
  host.bind = (e) => {
    exports = e;
  };
  return host;
}

/// Encodes the CPU path's draws directly in TypeScript, the comparison for the
/// Rust import path. Log layout: [list buffer, then per draw: bind group,
/// byte offset, byte size, index count, instance count, first index, base vertex].
export function encodeLogInTypeScript(host: GpuHost, passHandle: number, log: Uint32Array): number {
  const pass = host.table[passHandle] as GPURenderPassEncoder;
  const list = host.table[log[0]] as GPUBuffer;
  const groups = host.table as GPUBindGroup[];
  const start = performance.now();
  let last = -1;
  for (let i = 1; i + 6 < log.length; i += 7) {
    const group = log[i];
    if (group !== last) {
      pass.setBindGroup(1, groups[group]);
      last = group;
    }
    pass.setVertexBuffer(1, list, log[i + 1], log[i + 2]);
    pass.drawIndexed(log[i + 3], log[i + 4], log[i + 5], log[i + 6], 0);
  }
  return performance.now() - start;
}
