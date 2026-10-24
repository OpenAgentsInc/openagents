import { SpellSwarm } from "../../packages/renderer/spell-swarm.ts";
import { lookAt, multiply, perspective } from "../../packages/renderer/math.ts";

interface OrientationImage {
  sha256: string;
  coveredPixels: number;
  finite: boolean;
}
export interface SpellOrientationVerification {
  schema: "aethyrforge.spell-orientation-verification.v1";
  pass: boolean;
  exact: boolean;
  count: number;
  trianglesPerDraw: number;
  width: number;
  height: number;
  format: "rgba16float";
  sampleCount: 4;
  time: number;
  simulationSteps: readonly [number, number];
  vertex: OrientationImage;
  compute: OrientationImage;
  differentBytes: number;
  differentPixels: number;
  maxAbsoluteChannelDifference: number;
  maxChannelUlps: number;
  coverageDifferences: number;
  alphaDifferences: number;
  firstDifferences: readonly {
    x: number;
    y: number;
    channel: number;
    vertex: number;
    compute: number;
  }[];
}

function halfFloat(word: number): number {
  const sign = word & 0x8000 ? -1 : 1;
  const exponent = (word >>> 10) & 31;
  const mantissa = word & 1023;
  if (exponent === 0) return sign * mantissa * 2 ** -24;
  if (exponent === 31) return mantissa ? NaN : sign * Infinity;
  return sign * (1 + mantissa / 1024) * 2 ** (exponent - 15);
}

async function describeImage(bytes: Uint8Array): Promise<OrientationImage> {
  const words = new Uint16Array(
    bytes.buffer,
    bytes.byteOffset,
    bytes.byteLength / 2,
  );
  let coveredPixels = 0;
  let finite = true;
  for (let i = 0; i < words.length; i += 4) {
    if (words[i] || words[i + 1] || words[i + 2]) coveredPixels++;
    for (let channel = 0; channel < 4; channel++)
      if (!Number.isFinite(halfFloat(words[i + channel]))) finite = false;
  }
  const hash = await crypto.subtle.digest(
    "SHA-256",
    bytes as Uint8Array<ArrayBuffer>,
  );
  return {
    sha256: Array.from(new Uint8Array(hash), (v) =>
      v.toString(16).padStart(2, "0"),
    ).join(""),
    coveredPixels,
    finite,
  };
}

/** Run before warm-up, never inside an FPS measurement window. */
export async function verifySpellOrientation(): Promise<SpellOrientationVerification> {
  if (!navigator.gpu)
    throw new Error("Spell orientation verification needs WebGPU.");
  const adapter = await navigator.gpu.requestAdapter({
    powerPreference: "high-performance",
  });
  if (!adapter)
    throw new Error("No WebGPU adapter is available for spell verification.");
  const device = await adapter.requestDevice();
  const swarms: SpellSwarm[] = [];
  const textures: GPUTexture[] = [];
  const reads: GPUBuffer[] = [];
  const errors: string[] = [];
  let scopes = 0;
  let failure: unknown;
  let result: SpellOrientationVerification | undefined;
  let closing = false;
  const onError = (event: GPUUncapturedErrorEvent) =>
    errors.push(event.error.message);
  device.addEventListener("uncapturederror", onError);
  void device.lost.then((info) => {
    if (!closing) errors.push(`Device lost: ${info.message || info.reason}`);
  });
  try {
    for (const scope of ["out-of-memory", "validation"] as const) {
      device.pushErrorScope(scope);
      scopes++;
    }
    const width = 256,
      height = 256,
      bytesPerRow = width * 8;
    const count = 1000,
      time = 1.25;
    const eye: [number, number, number] = [0, 0, 32];
    const view = multiply(
      perspective((50 * Math.PI) / 180, width / height, 0.1, 100),
      lookAt(eye, [0, 0, 0]),
    );
    for (const orientationPreparation of ["vertex", "compute"] as const)
      swarms.push(
        await SpellSwarm.create(device, {
          count,
          seed: 0x5eaf00d,
          center: [0, 0, 0],
          radius: 12,
          size: 0.11,
          format: "rgba16float",
          depthFormat: "depth24plus",
          sampleCount: 4,
          orientationPreparation,
        }),
      );
    const encoder = device.createCommandEncoder({
      label: "Spell orientation exact-image comparison",
    });
    for (const [index, swarm] of swarms.entries()) {
      const resolved = device.createTexture({
        label: `Spell orientation ${index}: resolved HDR`,
        size: [width, height],
        format: "rgba16float",
        usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.COPY_SRC,
      });
      textures.push(resolved);
      const color = device.createTexture({
        label: `Spell orientation ${index}: four-sample HDR`,
        size: [width, height],
        format: "rgba16float",
        sampleCount: 4,
        usage: GPUTextureUsage.RENDER_ATTACHMENT,
      });
      textures.push(color);
      const depth = device.createTexture({
        label: `Spell orientation ${index}: four-sample depth`,
        size: [width, height],
        format: "depth24plus",
        sampleCount: 4,
        usage: GPUTextureUsage.RENDER_ATTACHMENT,
      });
      textures.push(depth);
      const read = device.createBuffer({
        label: `Spell orientation ${index}: exact HDR readback`,
        size: bytesPerRow * height,
        usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
      });
      reads.push(read);
      swarm.prepareRender(encoder);
      const pass = encoder.beginRenderPass({
        label: `Spell orientation ${index}: original full mesh`,
        colorAttachments: [
          {
            view: color.createView(),
            resolveTarget: resolved.createView(),
            clearValue: [0, 0, 0, 0],
            loadOp: "clear",
            storeOp: "discard",
          },
        ],
        depthStencilAttachment: {
          view: depth.createView(),
          depthClearValue: 1,
          depthLoadOp: "clear",
          depthStoreOp: "discard",
        },
      });
      swarm.render(pass, view, eye, time);
      pass.end();
      encoder.copyTextureToBuffer(
        { texture: resolved },
        { buffer: read, bytesPerRow, rowsPerImage: height },
        [width, height],
      );
    }
    device.queue.submit([encoder.finish()]);
    for (const swarm of swarms) swarm.afterSubmit();
    await Promise.all(reads.map((read) => read.mapAsync(GPUMapMode.READ)));
    const images = reads.map(
      (read) => new Uint8Array(read.getMappedRange().slice(0)),
    );
    for (const read of reads) read.unmap();
    const [vertex, compute] = await Promise.all(images.map(describeImage));
    const words = images.map((image) => new Uint16Array(image.buffer));
    let differentBytes = 0,
      differentPixels = 0,
      maxAbsoluteChannelDifference = 0,
      maxChannelUlps = 0,
      coverageDifferences = 0,
      alphaDifferences = 0;
    const firstDifferences: SpellOrientationVerification["firstDifferences"][number][] =
      [];
    for (let i = 0; i < images[0].length; i++)
      if (images[0][i] !== images[1][i]) differentBytes++;
    for (let pixel = 0; pixel < width * height; pixel++) {
      let changed = false;
      const at = pixel * 4;
      const covered = words.map((image) =>
        Boolean(image[at] || image[at + 1] || image[at + 2]),
      );
      if (covered[0] !== covered[1]) coverageDifferences++;
      if (words[0][at + 3] !== words[1][at + 3]) alphaDifferences++;
      for (let channel = 0; channel < 4; channel++) {
        const index = pixel * 4 + channel;
        if (words[0][index] === words[1][index]) continue;
        changed = true;
        maxChannelUlps = Math.max(
          maxChannelUlps,
          Math.abs(words[0][index] - words[1][index]),
        );
        const a = halfFloat(words[0][index]),
          b = halfFloat(words[1][index]);
        maxAbsoluteChannelDifference = Math.max(
          maxAbsoluteChannelDifference,
          Math.abs(a - b),
        );
        if (firstDifferences.length < 16)
          firstDifferences.push({
            x: pixel % width,
            y: Math.floor(pixel / width),
            channel,
            vertex: a,
            compute: b,
          });
      }
      if (changed) differentPixels++;
    }
    const exact = differentBytes === 0;
    result = {
      schema: "aethyrforge.spell-orientation-verification.v1",
      pass:
        // A stored f32 basis can change final half-float rounding by one ULP.
        // Keep complete geometry coverage and alpha exact, with no larger color error.
        maxChannelUlps <= 1 &&
        coverageDifferences === 0 &&
        alphaDifferences === 0 &&
        vertex.coveredPixels > 0 &&
        compute.coveredPixels > 0 &&
        vertex.finite &&
        compute.finite,
      exact,
      count,
      trianglesPerDraw: count * 310,
      width,
      height,
      format: "rgba16float",
      sampleCount: 4,
      time,
      simulationSteps: [
        swarms[0].inspect().simulationSteps,
        swarms[1].inspect().simulationSteps,
      ],
      vertex,
      compute,
      differentBytes,
      differentPixels,
      maxAbsoluteChannelDifference,
      maxChannelUlps,
      coverageDifferences,
      alphaDifferences,
      firstDifferences,
    };
    await device.queue.onSubmittedWorkDone();
  } catch (error) {
    failure = error;
  } finally {
    while (scopes-- > 0) {
      try {
        const error = await device.popErrorScope();
        if (error) errors.push(error.message);
      } catch (error) {
        errors.push(String(error));
      }
    }
    closing = true;
    for (const read of reads) {
      if (read.mapState === "mapped") read.unmap();
      read.destroy();
    }
    for (const texture of textures) texture.destroy();
    for (const swarm of swarms) swarm.dispose();
    device.removeEventListener("uncapturederror", onError);
    device.destroy();
  }
  if (failure) throw failure;
  if (errors.length)
    throw new Error(`Spell orientation graphics errors: ${errors.join("; ")}`);
  return result!;
}
