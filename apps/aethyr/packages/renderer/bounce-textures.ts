import type { SourceLight } from "../assets/src/scene.ts";
import {
  readAsset,
  AssetIntegrityError,
} from "../assets/src/asset-download.ts";
import type { GiProbeManifest } from "./gi-probes.ts";
import type { GiProbeProfiles } from "./probe-profiles.ts";
import { selectProbeProfile } from "./probe-profiles.ts";
import { probeSha256 } from "./probe-textures.ts";
import {
  GI_BOUNCE_LAYERS,
  GI_BOUNCE_PROFILE,
  giBounceBaseIdentity,
  giBounceLocalLightsIdentityPayload,
  type GiBounceManifest,
  type GiBounceBlob,
} from "./gi-bounce.ts";
import { GI_MATERIAL_PROFILE } from "./gi-materials.ts";

type Mode = "day" | "night";
export type GiBounceManifests = Partial<Record<Mode, GiBounceManifest>>;
type LocalLights = Partial<Record<Mode, readonly SourceLight[]>>;
const hash = (value: string) => probeSha256(new TextEncoder().encode(value));
const validHash = (value: unknown) =>
  typeof value === "string" && /^[a-f0-9]{64}$/.test(value);

/** Binds each bounce field to the exact published base, without merging scenario grids. */
export async function validateBounceManifest(
  base: GiProbeManifest,
  manifest: GiBounceManifest,
  mode: Mode,
): Promise<void> {
  const selected = selectProbeProfile(base, mode),
    record = selected.scenarios[mode]!,
    scenario = manifest.scenarios?.[mode];
  if (
    manifest.schema !== "aethyrforge.private-bounce-probes.v1" ||
    manifest.profile !== GI_BOUNCE_PROFILE ||
    !scenario ||
    manifest.layout?.order !== "layer-major-sh9-rgb-vec4" ||
    manifest.layout.layers?.join(",") !== GI_BOUNCE_LAYERS.join(",") ||
    manifest.layout.coefficientStrideFloats !== 4 ||
    manifest.layout.coefficientsPerProbe !== 9 ||
    manifest.layout.bytesPerProbePerLayer !== 144 ||
    manifest.lighting?.convention !== "world-sh9-lambert-E-over-pi" ||
    manifest.lighting.diffuseBounceCount !== 1 ||
    manifest.lighting.gainBaked !== false ||
    manifest.lighting.localSourceStrengthBaked !== true ||
    manifest.lighting.sun !== "white-per-unit-lux" ||
    manifest.lighting.emissiveGi !== false ||
    manifest.lighting.materialProfile !== GI_MATERIAL_PROFILE ||
    manifest.lighting.alphaPolicy !== "alpha-tested-material-v1" ||
    selected.lighting.alphaPolicy !== "alpha-tested-material-v1"
  )
    throw new Error(
      "Prepare matching alpha-aware bounce lighting with the current asset tools.",
    );
  const grid = manifest.grid,
    expected = selected.grid;
  if (
    !grid ||
    grid.count !== expected.count ||
    !["origin", "spacing", "dimensions"].every((key) => {
      const actual = grid[key as "origin" | "spacing" | "dimensions"],
        wanted = expected[key as "origin" | "spacing" | "dimensions"];
      return (
        Array.isArray(actual) &&
        actual.length === 3 &&
        actual.every((n, i) => n === wanted[i])
      );
    }) ||
    manifest.base?.scenarios?.[mode]?.recordsSha256 !== record.sha256 ||
    manifest.base.scenarios[mode]?.momentsSha256 !== selected.moments.sha256 ||
    !validHash(manifest.base.identitySha256) ||
    (await hash(giBounceBaseIdentity(base))) !== manifest.base.identitySha256
  )
    throw new Error("Rebuild bounce lighting for the exact sky probe field.");
  if (
    scenario.sourceGeometrySha256 !==
      (record.sourceGeometrySha256 ?? selected.sourceGeometrySha256) ||
    scenario.sourceEnvironmentSha256 !== record.sourceEnvironmentSha256 ||
    ![
      scenario.sourceGeometrySha256,
      scenario.sourceEnvironmentSha256,
      scenario.sourceCoverageSha256,
      scenario.sourceMaterialSha256,
      scenario.sourceLocalLightsSha256,
    ].every(validHash)
  )
    throw new Error(
      "Bounce lighting does not match its source scene and environment.",
    );
  if (
    scenario.sourceSunDirection !== null &&
    (!Array.isArray(scenario.sourceSunDirection) ||
      scenario.sourceSunDirection.length !== 3 ||
      !scenario.sourceSunDirection.every(Number.isFinite) ||
      Math.abs(Math.hypot(...scenario.sourceSunDirection) - 1) > 0.001)
  )
    throw new Error("Bounce sunlight needs a finite source direction.");
  for (const layer of GI_BOUNCE_LAYERS) {
    const blob = scenario.layers?.[layer];
    if (
      !blob ||
      !/^\/private-assets\/[a-zA-Z0-9_.-]+$/.test(blob.url) ||
      !validHash(blob.sha256) ||
      blob.bytes !== expected.count * 144
    )
      throw new Error("The prepared bounce coefficient layout is invalid.");
  }
  if (
    (scenario.constantFog !== undefined) !==
    (record.constantFog !== undefined)
  )
    throw new Error(
      "Bounce constant fog must match its exact sky probe field.",
    );
  if (scenario.constantFog !== undefined) {
    const blob = scenario.constantFog;
    if (
      !blob ||
      !/^\/private-assets\/[a-zA-Z0-9_.-]+$/.test(blob.url) ||
      !validHash(blob.sha256) ||
      blob.bytes !== expected.count * 144
    )
      throw new Error("The constant fog bounce layout is invalid.");
  }
}

/** Owns one layer-major storage buffer per selected scenario and a shared inactive binding. */
export class BounceTextures {
  private readonly fields = new Map<
    Mode,
    {
      buffer: GPUBuffer;
      count: number;
      sun: readonly number[] | null;
      localHash: string;
      constantFog: boolean;
    }
  >();
  private readonly localValid = new Map<Mode, boolean>();
  private readonly localRevision = new Map<Mode, number>();
  private disposed = false;
  bytes = 16;
  private constructor(private readonly fallback: GPUBuffer) {}

  static async create(
    device: GPUDevice,
    profiles: GiProbeProfiles = {},
    manifests: GiBounceManifests = {},
    sourceLights: LocalLights = {},
  ) {
    for (const mode of ["day", "night"] as const) {
      const manifest = manifests[mode];
      if (!manifest) continue;
      if (!profiles[mode])
        throw new Error("Bounce lighting needs its matching sky probe field.");
      await validateBounceManifest(profiles[mode]!, manifest, mode);
    }
    const fallback = device.createBuffer({
      label: "Inactive bounce lighting",
      size: 16,
      usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
    });
    const result = new BounceTextures(fallback);
    const blobs = new Map<string, Promise<Uint8Array>>();
    const load = async (blob: GiBounceBlob) => {
      let pending = blobs.get(blob.url);
      if (!pending) {
        pending = readAsset(blob.url, async (response) => {
          if (!response.ok)
            throw new Error("The prepared bounce lighting could not open.");
          const bytes = new Uint8Array(await response.arrayBuffer());
          if (
            bytes.byteLength !== blob.bytes ||
            (await probeSha256(bytes)) !== blob.sha256
          )
            throw new AssetIntegrityError(
              "The prepared bounce lighting file is incomplete.",
            );
          return bytes;
        });
        blobs.set(blob.url, pending);
      }
      const bytes = await pending;
      if (
        bytes.byteLength !== blob.bytes ||
        (await probeSha256(bytes)) !== blob.sha256
      )
        throw new Error("The prepared bounce lighting file is incomplete.");
      return bytes;
    };
    try {
      device.queue.writeBuffer(fallback, 0, new Float32Array(4));
      for (const mode of ["day", "night"] as const) {
        const manifest = manifests[mode];
        if (!manifest) continue;
        const scenario = manifest.scenarios[mode]!,
          count = manifest.grid.count;
        const descriptions = GI_BOUNCE_LAYERS.map(
          (layer) => scenario.layers[layer],
        );
        if (scenario.constantFog) descriptions.push(scenario.constantFog);
        const packed = new Float32Array(count * 36 * descriptions.length);
        for (let layer = 0; layer < descriptions.length; layer++) {
          const bytes = await load(descriptions[layer]);
          const input = new DataView(
            bytes.buffer,
            bytes.byteOffset,
            bytes.byteLength,
          );
          for (let i = 0; i < count * 36; i++) {
            const value = input.getFloat32(i * 4, true);
            if (!Number.isFinite(value))
              throw new Error(
                "Bounce coefficients must contain finite values.",
              );
            packed[layer * count * 36 + i] = value;
          }
        }
        const buffer = device.createBuffer({
          label: mode + " diffuse bounce layers",
          size: packed.byteLength,
          usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
        });
        result.fields.set(mode, {
          buffer,
          count,
          sun:
            scenario.sourceSunDirection?.map(
              (n) => -n / Math.hypot(...scenario.sourceSunDirection!),
            ) ?? null,
          localHash: scenario.sourceLocalLightsSha256,
          constantFog: !!scenario.constantFog,
        });
        result.bytes += packed.byteLength;
        device.queue.writeBuffer(buffer, 0, packed);
        await result.setLocalLights(mode, sourceLights[mode]);
      }
      return result;
    } catch (error) {
      result.dispose();
      throw error;
    }
  }
  get(mode: Mode): GPUBuffer {
    return this.fields.get(mode)?.buffer ?? this.fallback;
  }
  hasConstantFog(mode: Mode): boolean {
    return !this.disposed && this.fields.get(mode)?.constantFog === true;
  }
  /** Unknown or edited local sources immediately disable only their baked local layer. */
  async setLocalLights(
    mode: Mode,
    lights?: readonly SourceLight[],
  ): Promise<boolean> {
    const revision = (this.localRevision.get(mode) ?? 0) + 1;
    this.localRevision.set(mode, revision);
    this.localValid.set(mode, false);
    const field = this.fields.get(mode);
    if (!field || !lights || this.disposed) return false;
    const matches =
      (await hash(giBounceLocalLightsIdentityPayload(lights))) ===
      field.localHash;
    if (this.disposed || this.localRevision.get(mode) !== revision)
      return false;
    this.localValid.set(mode, matches);
    return matches;
  }
  settings(mode: Mode, currentSunDirection: readonly number[]): Float32Array {
    const field = this.fields.get(mode);
    if (!field || this.disposed) return new Float32Array(4);
    let sun = false;
    const length =
      currentSunDirection.length === 3 ? Math.hypot(...currentSunDirection) : 0;
    if (
      field.sun &&
      currentSunDirection.length === 3 &&
      currentSunDirection.every(Number.isFinite) &&
      length > 1e-8
    )
      sun =
        field.sun.reduce(
          (sum, n, i) => sum + (n * currentSunDirection[i]) / length,
          0,
        ) >=
        1 - 1e-6;
    return new Float32Array([
      1,
      Number(sun),
      Number(this.localValid.get(mode) === true),
      field.count,
    ]);
  }
  dispose() {
    if (this.disposed) return;
    this.disposed = true;
    for (const field of this.fields.values()) field.buffer.destroy();
    this.fallback.destroy();
  }
}
