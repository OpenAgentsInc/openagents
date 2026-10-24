// SPDX-License-Identifier: Apache-2.0
// Ported from OpenAgents apps/verse-selector at 9fd3263494.
import { camera, geometry, project, worlds, type Vec3 } from "./space.ts";
import { shader } from "./shaders.ts";
import { placeLabels, type Label } from "./labels.ts";

export class Universe {
  yaw = 0.13;
  pitch = 0.57;
  distance = 27;
  target: Vec3 = [0, 0, 0];
  desired: Vec3 = [0, 0, 0];
  device: GPUDevice;
  context: GPUCanvasContext;
  uniform: GPUBuffer;
  bind: GPUBindGroup;
  planets: GPURenderPipeline;
  orbits: GPURenderPipeline;
  sky: GPURenderPipeline;
  meshes: GPUBuffer;
  rings: GPUBuffer;
  depth?: GPUTexture;
  triangleCount: number;
  lineCount: number;
  width = 0;
  height = 0;
  frame = 0;
  submittedFrames = 0;
  disposed = false;
  ambient = true;
  /** The chosen planet, whose label always shows. */
  selected: string | undefined = "openagents";
  private labelSizes = new Map<string, { width: number; height: number }>();
  private motion = matchMedia("(prefers-reduced-motion: reduce)");
  private lastAmbientFrame = 0;
  private lastYaw = NaN;
  private lastPitch = NaN;
  private lastDistance = NaN;
  matrix = new Float32Array(16);
  private data = new Float32Array(24);
  static async create(canvas: HTMLCanvasElement) {
    if (!navigator.gpu) throw new Error("WebGPU is unavailable");
    const adapter = await navigator.gpu.requestAdapter();
    if (!adapter) throw new Error("No graphics adapter");
    const device = await adapter.requestDevice();
    try {
      return new Universe(canvas, device);
    } catch (error) {
      device.destroy();
      throw error;
    }
  }
  private constructor(
    public canvas: HTMLCanvasElement,
    device: GPUDevice,
  ) {
    this.device = device;
    const context = canvas.getContext("webgpu");
    if (!context) throw new Error("No graphics context");
    this.context = context;
    const format = navigator.gpu.getPreferredCanvasFormat();
    context.configure({ device, format, alphaMode: "opaque" });
    const module = device.createShaderModule({
      label: "Verse planets and stars",
      code: shader,
    });
    this.uniform = device.createBuffer({
      size: 96,
      usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
    });
    const layout = device.createBindGroupLayout({
      entries: [
        {
          binding: 0,
          visibility: GPUShaderStage.VERTEX | GPUShaderStage.FRAGMENT,
          buffer: { type: "uniform" },
        },
      ],
    });
    this.bind = device.createBindGroup({
      layout,
      entries: [{ binding: 0, resource: { buffer: this.uniform } }],
    });
    const pipelineLayout = device.createPipelineLayout({
      bindGroupLayouts: [layout],
    });
    const buffers: GPUVertexBufferLayout[] = [
      {
        arrayStride: 40,
        attributes: [
          { shaderLocation: 0, offset: 0, format: "float32x3" },
          { shaderLocation: 1, offset: 12, format: "float32x3" },
          { shaderLocation: 2, offset: 24, format: "float32x3" },
          { shaderLocation: 3, offset: 36, format: "float32" },
        ],
      },
    ];
    this.planets = device.createRenderPipeline({
      layout: pipelineLayout,
      vertex: { module, entryPoint: "vertex", buffers },
      fragment: { module, entryPoint: "planet", targets: [{ format }] },
      primitive: { topology: "triangle-list" },
      depthStencil: {
        format: "depth24plus",
        depthWriteEnabled: true,
        depthCompare: "less",
      },
    });
    this.orbits = device.createRenderPipeline({
      layout: pipelineLayout,
      vertex: { module, entryPoint: "vertex", buffers },
      fragment: {
        module,
        entryPoint: "orbit",
        targets: [
          {
            format,
            blend: {
              color: {
                srcFactor: "src-alpha",
                dstFactor: "one-minus-src-alpha",
              },
              alpha: { srcFactor: "one", dstFactor: "one-minus-src-alpha" },
            },
          },
        ],
      },
      primitive: { topology: "line-list" },
      depthStencil: {
        format: "depth24plus",
        depthWriteEnabled: false,
        depthCompare: "less-equal",
      },
    });
    this.sky = device.createRenderPipeline({
      layout: pipelineLayout,
      vertex: { module, entryPoint: "skyVertex" },
      fragment: { module, entryPoint: "sky", targets: [{ format }] },
      primitive: { topology: "triangle-list" },
      depthStencil: {
        format: "depth24plus",
        depthWriteEnabled: false,
        depthCompare: "always",
      },
    });
    const geo = geometry();
    this.triangleCount = geo.triangles.length / 10;
    this.lineCount = geo.lines.length / 10;
    this.meshes = this.buffer(geo.triangles);
    this.rings = this.buffer(geo.lines);
  }
  buffer(data: Float32Array) {
    const b = this.device.createBuffer({
      size: data.byteLength,
      usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
    });
    this.device.queue.writeBuffer(b, 0, data.buffer);
    return b;
  }
  render(time: number) {
    if (this.disposed || document.hidden) return;
    if (this.ambient && !this.motion.matches) {
      if (time - this.lastAmbientFrame < 1000 / 30) return;
      const elapsed = Math.min((time - this.lastAmbientFrame) / 1000, 0.05);
      this.lastAmbientFrame = time;
      this.yaw += elapsed * 0.008;
    }
    const ratio = Math.min(devicePixelRatio, 1.7);
    const width = Math.max(1, Math.floor(this.canvas.clientWidth * ratio)),
      height = Math.max(1, Math.floor(this.canvas.clientHeight * ratio));
    const moving = this.desired.some(
      (value, i) => Math.abs(value - this.target[i]) > 0.001,
    );
    if (
      width === this.width &&
      height === this.height &&
      this.yaw === this.lastYaw &&
      this.pitch === this.lastPitch &&
      this.distance === this.lastDistance &&
      !moving
    )
      return;
    this.lastYaw = this.yaw;
    this.lastPitch = this.pitch;
    this.lastDistance = this.distance;
    if (width !== this.width || height !== this.height) {
      this.width = width;
      this.height = height;
      this.canvas.width = width;
      this.canvas.height = height;
      this.depth?.destroy();
      this.depth = this.device.createTexture({
        size: [width, height],
        format: "depth24plus",
        usage: GPUTextureUsage.RENDER_ATTACHMENT,
      });
    }
    for (let i = 0; i < 3; i++)
      this.target[i] += (this.desired[i] - this.target[i]) * 0.075;
    const view = camera(
      this.yaw,
      this.pitch,
      this.distance,
      this.target,
      width / height,
    );
    this.matrix = view.matrix;
    this.data.set(view.matrix);
    this.data.set([...view.eye, time * 0.001], 16);
    this.data.set([width, height, 0, 0], 20);
    this.device.queue.writeBuffer(this.uniform, 0, this.data.buffer);
    const encoder = this.device.createCommandEncoder();
    const pass = encoder.beginRenderPass({
      colorAttachments: [
        {
          view: this.context.getCurrentTexture().createView(),
          loadOp: "clear",
          storeOp: "store",
          clearValue: { r: 0, g: 0, b: 0, a: 1 },
        },
      ],
      depthStencilAttachment: {
        view: this.depth!.createView(),
        depthLoadOp: "clear",
        depthStoreOp: "store",
        depthClearValue: 1,
      },
    });
    pass.setBindGroup(0, this.bind);
    pass.setPipeline(this.sky);
    pass.draw(3);
    pass.setPipeline(this.planets);
    pass.setVertexBuffer(0, this.meshes);
    pass.draw(this.triangleCount);
    pass.setPipeline(this.orbits);
    pass.setVertexBuffer(0, this.rings);
    pass.draw(this.lineCount);
    pass.end();
    this.device.queue.submit([encoder.finish()]);
    this.submittedFrames++;
    this.placeLabels(view.eye);
  }
  /** Show the selected planet's label and every other label that has room. */
  private placeLabels(eye: Vec3) {
    const box = this.canvas.getBoundingClientRect();
    const labels: Label[] = [];
    for (const world of worlds) {
      const element = document.getElementById(`marker-${world.id}`);
      if (!element) continue;
      const p = project(world.position, this.matrix);
      element.style.left = `${p.x * 100}%`;
      element.style.top = `${p.y * 100}%`;
      let size = this.labelSizes.get(world.id);
      if (!size) {
        element.hidden = false;
        size = { width: element.offsetWidth, height: element.offsetHeight };
        if (size.width > 0) this.labelSizes.set(world.id, size);
      }
      labels.push({
        id: world.id,
        x: p.x * box.width,
        y: p.y * box.height,
        ...size,
        depth: Math.hypot(...world.position.map((v, i) => v - eye[i])),
        inFrame:
          p.visible && p.x >= 0.03 && p.x <= 0.97 && p.y >= 0.08 && p.y <= 0.9,
      });
    }
    const shown = placeLabels(labels, this.selected, box);
    for (const label of labels) {
      const element = document.getElementById(`marker-${label.id}`)!;
      element.hidden = !shown.has(label.id);
      if (label.id === this.selected) element.setAttribute("aria-current", "true");
      else element.removeAttribute("aria-current");
    }
  }
  /** Mark a planet as chosen; its label stays visible. */
  choose(id: string) {
    this.selected = id;
    // Draw the next frame even if the camera does not move.
    this.lastYaw = NaN;
  }
  start() {
    const draw = (t: number) => {
      if (this.disposed) return;
      this.render(t);
      this.frame = requestAnimationFrame(draw);
    };
    this.frame = requestAnimationFrame(draw);
  }
  focus(id: string) {
    const w = worlds.find((w) => w.id === id);
    if (w)
      this.desired = [
        w.position[0] * 0.34,
        w.position[1] * 0.34,
        w.position[2] * 0.34,
      ];
  }
  reset() {
    this.desired = [0, 0, 0];
    this.target = [0, 0, 0];
    this.yaw = 0.13;
    this.pitch = 0.57;
    this.distance = 27;
  }
  dispose() {
    if (this.disposed) return;
    this.disposed = true;
    cancelAnimationFrame(this.frame);
    this.depth?.destroy();
    this.meshes.destroy();
    this.rings.destroy();
    this.uniform.destroy();
    this.context.unconfigure();
    this.device.destroy();
  }
}
