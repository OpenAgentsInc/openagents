export const DISPLAY_LUT_SHAPER_A = 2 ** -10;
export const DISPLAY_LUT_MAX_INPUT = 64;
export const DISPLAY_LUT_MAX_BYTES = 65 ** 3 * 8;

/** Both versions sample positive scene-linear Rec709 after external exposure.
 * v1 fixes a=2^-10 and max=64. v2 admits a in [2^-20,1] and max in [1,65504].
 * Inputs clamp to [0,max]; NaN maps to zero and infinities saturate. Outputs
 * retain their finite signed tone-linear values until the final display clip. */
export interface DisplayLutDescriptor {
  schema: "aethyrforge.display-lut.v1" | "aethyrforge.display-lut.v2";
  id: string;
  edge: number;
  inputSpace: "linear-rec709";
  outputSpace: "tone-linear-rec709";
  exposure: "external";
  shaper: { kind: "log2-1p"; a: number; max: number };
  order: "r-fastest";
  format: "rgba16float-le";
  byteLength: number;
  sha256: string;
  url: string;
}

/** Return a bounded copy; unrelated source metadata does not enter the runtime. */
export function validateDisplayLut(value: unknown): DisplayLutDescriptor {
  const d = value as Partial<DisplayLutDescriptor> | null;
  if (
    !d ||
    !["aethyrforge.display-lut.v1", "aethyrforge.display-lut.v2"].includes(
      d.schema!,
    ) ||
    typeof d.id !== "string" ||
    !/^[a-z0-9][a-z0-9._-]{0,127}$/.test(d.id) ||
    !Number.isInteger(d.edge) ||
    d.edge! < 33 ||
    d.edge! > 65 ||
    d.inputSpace !== "linear-rec709" ||
    d.outputSpace !== "tone-linear-rec709" ||
    d.exposure !== "external" ||
    d.shaper?.kind !== "log2-1p" ||
    !Number.isFinite(d.shaper.a) ||
    d.shaper.a < 2 ** -20 ||
    d.shaper.a > 1 ||
    !Number.isFinite(d.shaper.max) ||
    d.shaper.max < 1 ||
    d.shaper.max > 65504 ||
    (d.schema === "aethyrforge.display-lut.v1" &&
      (d.shaper.a !== DISPLAY_LUT_SHAPER_A ||
        d.shaper.max !== DISPLAY_LUT_MAX_INPUT)) ||
    d.order !== "r-fastest" ||
    d.format !== "rgba16float-le" ||
    d.byteLength !== d.edge! ** 3 * 8 ||
    typeof d.sha256 !== "string" ||
    !/^[a-f0-9]{64}$/.test(d.sha256) ||
    typeof d.url !== "string" ||
    d.url.length < 1 ||
    d.url.length > 2048 ||
    /[\u0000-\u0020\u007f]/.test(d.url) ||
    /^(?!https?:)[a-z][a-z0-9+.-]*:/i.test(d.url)
  )
    throw new RangeError("Display lookup descriptor is invalid");
  return {
    schema: d.schema!,
    id: d.id,
    edge: d.edge!,
    inputSpace: d.inputSpace,
    outputSpace: d.outputSpace,
    exposure: d.exposure,
    shaper: { ...d.shaper },
    order: d.order,
    format: d.format,
    byteLength: d.byteLength!,
    sha256: d.sha256,
    url: d.url,
  };
}

/** Negative/NaN inputs become zero; positive infinity saturates at the declared maximum. */
export function displayLutShaper(
  value: number,
  descriptor?: Pick<DisplayLutDescriptor, "shaper">,
): number {
  const { a, max } = descriptor?.shaper ?? {
    a: DISPLAY_LUT_SHAPER_A,
    max: DISPLAY_LUT_MAX_INPUT,
  };
  const x = value > 0 ? Math.min(value, max) : 0;
  return Math.log2(1 + x / a) / Math.log2(1 + max / a);
}

/** Passing an edge alone retains the fixed v1 grid; v2 callers pass the descriptor. */
export function displayLutGridInput(
  index: number,
  grid: number | DisplayLutDescriptor,
): number {
  const edge = typeof grid === "number" ? grid : grid.edge;
  const { a, max } =
    typeof grid === "number"
      ? { a: DISPLAY_LUT_SHAPER_A, max: DISPLAY_LUT_MAX_INPUT }
      : grid.shaper;
  if (
    !Number.isInteger(edge) ||
    edge < 33 ||
    edge > 65 ||
    !Number.isInteger(index) ||
    index < 0 ||
    index >= edge
  )
    throw new RangeError("Display lookup grid coordinate is invalid");
  if (index === edge - 1) return max;
  return a * (2 ** ((index / (edge - 1)) * Math.log2(1 + max / a)) - 1);
}

/** Output remains unclipped, including finite negative values and values above one. */
export function decodeDisplayLut(
  descriptor: DisplayLutDescriptor,
  bytes: Uint8Array,
): Float32Array {
  const d = validateDisplayLut(descriptor);
  if (bytes.byteLength !== d.byteLength)
    throw new RangeError("Display lookup byte count does not match");
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const values = new Float32Array(bytes.byteLength / 2);
  for (let i = 0; i < values.length; i++) {
    const bits = view.getUint16(i * 2, true),
      exponent = (bits >>> 10) & 31,
      fraction = bits & 1023,
      sign = bits & 32768 ? -1 : 1;
    if (exponent === 31)
      throw new RangeError("Display lookup contains a nonfinite value");
    values[i] =
      exponent === 0
        ? sign * fraction * 2 ** -24
        : sign * (1 + fraction / 1024) * 2 ** (exponent - 15);
    if (i % 4 === 3 && values[i] !== 1)
      throw new RangeError("Display lookup alpha must be one");
  }
  return values;
}

export async function verifyDisplayLut(
  descriptor: unknown,
  source: Uint8Array,
) {
  const d = validateDisplayLut(descriptor);
  if (source.byteLength !== d.byteLength)
    throw new RangeError("Display lookup byte count does not match");
  // Hash and upload the same snapshot even if the caller later reuses its buffer.
  const bytes = new Uint8Array(source);
  const hash = Array.from(
    new Uint8Array(await crypto.subtle.digest("SHA-256", bytes)),
    (b) => b.toString(16).padStart(2, "0"),
  ).join("");
  if (hash !== d.sha256) throw new Error("Display lookup hash does not match");
  return { descriptor: d, bytes, values: decodeDisplayLut(d, bytes) };
}

/** CPU reference for already decoded, verified response samples. */
export function sampleDisplayLut(
  descriptor: DisplayLutDescriptor,
  values: Float32Array,
  color: readonly [number, number, number],
): [number, number, number] {
  const edge = descriptor.edge;
  if (
    !Number.isInteger(edge) ||
    edge < 33 ||
    edge > 65 ||
    values.length !== edge ** 3 * 4
  )
    throw new RangeError("Display lookup sample grid is invalid");
  const coordinates = color.map(
    (c) => displayLutShaper(c, descriptor) * (edge - 1),
  );
  const lo = coordinates.map(Math.floor),
    hi = lo.map((i) => Math.min(i + 1, edge - 1));
  const f = coordinates.map((v, i) => v - lo[i]);
  const result: [number, number, number] = [0, 0, 0];
  for (let b = 0; b < 2; b++)
    for (let g = 0; g < 2; g++)
      for (let r = 0; r < 2; r++) {
        const index =
          4 *
          ((r ? hi[0] : lo[0]) +
            edge * ((g ? hi[1] : lo[1]) + edge * (b ? hi[2] : lo[2])));
        const weight =
          (r ? f[0] : 1 - f[0]) * (g ? f[1] : 1 - f[1]) * (b ? f[2] : 1 - f[2]);
        for (let channel = 0; channel < 3; channel++)
          result[channel] += values[index + channel] * weight;
      }
  return result;
}

export function displayLutParams(
  descriptor?: DisplayLutDescriptor,
  enabled = true,
): Float32Array {
  return new Float32Array([
    descriptor?.edge ?? 1,
    descriptor?.shaper.a ?? DISPLAY_LUT_SHAPER_A,
    descriptor?.shaper.max ?? DISPLAY_LUT_MAX_INPUT,
    Number(!!descriptor && enabled),
  ]);
}

export class DisplayLut {
  readonly view: GPUTextureView;
  readonly sampler: GPUSampler;
  readonly params: Float32Array;
  readonly bytes: number;
  private disposed = false;

  private constructor(
    private readonly texture: GPUTexture,
    readonly descriptor: DisplayLutDescriptor,
    sampler: GPUSampler,
  ) {
    this.view = texture.createView({ dimension: "3d" });
    this.sampler = sampler;
    this.params = displayLutParams(descriptor);
    this.bytes = descriptor.byteLength;
  }

  static async create(
    device: GPUDevice,
    descriptor: unknown,
    source: Uint8Array,
  ): Promise<DisplayLut> {
    const verified = await verifyDisplayLut(descriptor, source);
    const d = verified.descriptor;
    if (d.edge > device.limits.maxTextureDimension3D)
      throw new RangeError("Display lookup exceeds the device texture limit");
    let texture: GPUTexture | undefined,
      owner: DisplayLut | undefined,
      constructionError: unknown;
    device.pushErrorScope("out-of-memory");
    device.pushErrorScope("validation");
    try {
      texture = device.createTexture({
        label: `Display response ${d.id}`,
        dimension: "3d",
        size: [d.edge, d.edge, d.edge],
        format: "rgba16float",
        usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST,
      });
      device.queue.writeTexture(
        { texture },
        verified.bytes,
        { bytesPerRow: d.edge * 8, rowsPerImage: d.edge },
        [d.edge, d.edge, d.edge],
      );
      const sampler = device.createSampler({
        minFilter: "linear",
        magFilter: "linear",
        mipmapFilter: "nearest",
        addressModeU: "clamp-to-edge",
        addressModeV: "clamp-to-edge",
        addressModeW: "clamp-to-edge",
      });
      owner = new DisplayLut(texture, d, sampler);
    } catch (error) {
      constructionError = error;
    }
    // Pop both scopes before awaiting so concurrent loaders cannot nest here.
    const validation = device.popErrorScope(),
      allocation = device.popErrorScope();
    try {
      const failures = await Promise.all([validation, allocation]);
      if (constructionError) throw constructionError;
      const failure = failures.find(Boolean);
      if (failure)
        throw new Error(
          `Display lookup could not initialize: ${failure.message}`,
        );
      return owner!;
    } catch (error) {
      texture?.destroy();
      throw error;
    }
  }

  dispose() {
    if (this.disposed) return;
    this.disposed = true;
    this.texture.destroy();
  }
}

/** Binding-free helper. Exposure, vignette and saturation precede this lookup. */
export const WGSL_DISPLAY_LUT = `fn sampleDisplayLut(image:texture_3d<f32>,filtering:sampler,params:vec4<f32>,color:vec3<f32>)->vec3<f32>{if params.w<0.5{return color;}let bounded=min(select(vec3(0.0),color,color>vec3(0.0)),vec3(params.z));let u=log2(vec3(1.0)+bounded/params.y)/log2(1.0+params.z/params.y);let coordinate=(u*(params.x-1.0)+vec3(0.5))/params.x;return textureSampleLevel(image,filtering,coordinate,0.0).rgb;}`;
