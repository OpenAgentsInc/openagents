import type { GiProbeManifest } from "./gi-probes.ts";
import {
  readAsset,
  AssetIntegrityError,
} from "../assets/src/asset-download.ts";

export function validateProbeManifest(manifest: GiProbeManifest): void {
  const { grid, moments, records, lighting } = manifest;
  if (
    manifest.schema !== "aethyrforge.private-sky-probes.v1" ||
    manifest.profile !== "occluded-sky-sh9-v1" ||
    !/^[a-f0-9]{64}$/.test(manifest.sourceGeometrySha256) ||
    !grid ||
    grid.origin.length !== 3 ||
    !grid.origin.every(Number.isFinite) ||
    grid.spacing.length !== 3 ||
    !grid.spacing.every((v) => Number.isFinite(v) && v > 0 && v <= 64) ||
    grid.dimensions.length !== 3 ||
    !grid.dimensions.every((v) => Number.isInteger(v) && v >= 2 && v <= 64) ||
    grid.count !== grid.dimensions.reduce((a, b) => a * b, 1) ||
    grid.count > 4096 ||
    records?.strideFloats !== 40 ||
    records.positionValidOffset !== 0 ||
    records.shOffset !== 4 ||
    lighting?.convention !== "world-sh9-lambert-E-over-pi" ||
    lighting.gainBaked !== false ||
    lighting.skyBounceCount !== 0 ||
    !["masked-solid-preview", "alpha-tested-material-v1"].includes(
      lighting.alphaPolicy,
    )
  )
    throw new Error("Prepare the sky probes with the current asset tools.");
  const validateMoments = (moments: GiProbeManifest["moments"]) => {
    if (
      !moments ||
      moments.format !== "rgba16float" ||
      moments.interior !== 8 ||
      moments.tileSize !== 10 ||
      !Number.isInteger(moments.tilesPerRow) ||
      moments.tilesPerRow < 1 ||
      moments.tilesPerRow > 64 ||
      moments.width !== moments.tilesPerRow * 10 ||
      moments.height !== Math.ceil(grid.count / moments.tilesPerRow) * 10 ||
      moments.bytes !== moments.width * moments.height * 8 ||
      !Number.isFinite(moments.rayDistanceMeters) ||
      moments.rayDistanceMeters <= 0 ||
      moments.rayDistanceMeters > 128 ||
      moments.distanceConvention !== "mean-meters,mean-square-meters-squared"
    )
      throw new Error("The sky probe visibility map is invalid.");
  };
  validateMoments(moments);
  for (const scenario of Object.values(manifest.scenarios)) {
    if (
      !scenario ||
      scenario.bytes !== grid.count * 160 ||
      !/^[a-f0-9]{64}$/.test(scenario.sourceEnvironmentSha256)
    )
      throw new Error("The sky probe records do not match their grid.");
    if (
      scenario.sourceGeometrySha256 !== undefined &&
      !/^[a-f0-9]{64}$/.test(scenario.sourceGeometrySha256)
    )
      throw new Error("The sky probe geometry identity is invalid.");
    if (scenario.moments) validateMoments(scenario.moments);
    if (scenario.constantFog !== undefined) {
      const blob = scenario.constantFog;
      if (
        !blob ||
        !/^\/private-assets\/[a-zA-Z0-9_.-]+$/.test(blob.url) ||
        !/^[a-f0-9]{64}$/.test(blob.sha256) ||
        blob.bytes !== grid.count * 144
      )
        throw new Error(
          "The constant fog probe coefficients do not match their grid.",
        );
    }
  }
}

export function probeSettings(
  manifest: GiProbeManifest | undefined,
  enabled: boolean,
  mode: "day" | "night" = "day",
): Float32Array {
  if (!manifest) return new Float32Array(16);
  const { grid } = manifest;
  const moments = manifest.scenarios[mode]?.moments ?? manifest.moments;
  return new Float32Array([
    ...grid.origin,
    Number(enabled),
    ...grid.spacing,
    moments.rayDistanceMeters,
    ...grid.dimensions,
    moments.tilesPerRow,
    moments.width,
    moments.height,
    moments.tileSize,
    moments.interior,
  ]);
}

export async function probeSha256(bytes: Uint8Array): Promise<string> {
  const digest = await crypto.subtle.digest("SHA-256", bytes.slice().buffer);
  return Array.from(new Uint8Array(digest), (v) =>
    v.toString(16).padStart(2, "0"),
  ).join("");
}

export class ProbeTextures {
  readonly scenarios = new Map<string, GPUBuffer>();
  readonly scenarioMoments = new Map<string, GPUTexture>();
  private readonly constantFogModes = new Set<string>();
  bytes = 168;
  private constructor(
    readonly moments: GPUTexture,
    readonly fallback: GPUBuffer,
    readonly manifest?: GiProbeManifest,
  ) {}

  static async create(device: GPUDevice, manifest?: GiProbeManifest) {
    if (manifest) validateProbeManifest(manifest);
    const description = manifest?.moments;
    const moments = device.createTexture({
      label: "Sky probe distance moments",
      size: [description?.width ?? 1, description?.height ?? 1],
      format: "rgba16float",
      usage: GPUTextureUsage.COPY_DST | GPUTextureUsage.TEXTURE_BINDING,
    });
    const fallback = device.createBuffer({
      label: "Inactive sky probe",
      size: 160,
      usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
    });
    const result = new ProbeTextures(moments, fallback, manifest);
    try {
      if (!manifest) {
        device.queue.writeTexture(
          { texture: moments },
          new Uint16Array([0, 0, 0, 0x3c00]),
          { bytesPerRow: 8 },
          [1, 1],
        );
        return result;
      }
      async function blob(
        source: { url: string; bytes: number; sha256: string },
        expected: number,
      ) {
        if (
          !/^\/private-assets\/[a-zA-Z0-9_.-]+$/.test(source.url) ||
          source.bytes !== expected ||
          !/^[a-f0-9]{64}$/.test(source.sha256)
        )
          throw new Error("The prepared sky probe file is invalid.");
        return readAsset(source.url, async (response) => {
          if (!response.ok)
            throw new Error("The prepared sky probes could not open.");
          const bytes = new Uint8Array(await response.arrayBuffer());
          if (
            bytes.byteLength !== expected ||
            (await probeSha256(bytes)) !== source.sha256
          )
            throw new AssetIntegrityError(
              "The prepared sky probe file is incomplete.",
            );
          return bytes;
        });
      }
      const loadMoments = async (
        texture: GPUTexture,
        description: GiProbeManifest["moments"],
      ) => {
        const momentData = await blob(description, description.bytes);
        const half = new Uint16Array(momentData.buffer);
        for (let i = 0; i < half.length; i++)
          if ((half[i] & 0x7c00) === 0x7c00 || (half[i] & 0x8000) !== 0)
            throw new Error(
              "Sky probe distances must be finite and nonnegative.",
            );
        device.queue.writeTexture(
          { texture },
          momentData,
          { bytesPerRow: description.width * 8 },
          [description.width, description.height],
        );
        result.bytes += momentData.byteLength;
      };
      await loadMoments(moments, manifest.moments);
      for (const [mode, scenario] of Object.entries(manifest.scenarios)) {
        if (
          scenario.moments &&
          scenario.moments.sha256 !== manifest.moments.sha256
        ) {
          const description = scenario.moments;
          const texture = device.createTexture({
            label: mode + " sky probe moments",
            size: [description.width, description.height],
            format: "rgba16float",
            usage: GPUTextureUsage.COPY_DST | GPUTextureUsage.TEXTURE_BINDING,
          });
          result.scenarioMoments.set(mode, texture);
          await loadMoments(texture, description);
        }
        const data = await blob(scenario, manifest.grid.count * 160);
        const floats = new Float32Array(data.buffer);
        if (!floats.every(Number.isFinite))
          throw new Error("Sky probe records must contain finite values.");
        for (let i = 0; i < manifest.grid.count; i++)
          if (![0, 1].includes(floats[i * 40 + 3]))
            throw new Error("Sky probe validity must be zero or one.");
        let packed = data;
        if (scenario.constantFog) {
          const constant = await blob(
            scenario.constantFog,
            manifest.grid.count * 144,
          );
          if (!new Float32Array(constant.buffer).every(Number.isFinite))
            throw new Error(
              "Constant fog probe coefficients must contain finite values.",
            );
          packed = new Uint8Array(data.byteLength + constant.byteLength);
          packed.set(data);
          packed.set(constant, data.byteLength);
        }
        const buffer = device.createBuffer({
          label: mode + " occluded sky probes",
          size: packed.byteLength,
          usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
        });
        result.scenarios.set(mode, buffer);
        device.queue.writeBuffer(buffer, 0, packed);
        if (scenario.constantFog) result.constantFogModes.add(mode);
        result.bytes += packed.byteLength;
      }
      return result;
    } catch (error) {
      result.dispose();
      throw error;
    }
  }

  hasConstantFog(mode: "day" | "night"): boolean {
    return this.constantFogModes.has(mode);
  }

  dispose() {
    this.constantFogModes.clear();
    this.moments.destroy();
    for (const texture of this.scenarioMoments.values()) texture.destroy();
    this.fallback.destroy();
    for (const buffer of this.scenarios.values()) buffer.destroy();
  }
}
