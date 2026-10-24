import type { GiProbeManifest } from "./gi-probes.ts";
import { ProbeTextures, validateProbeManifest } from "./probe-textures.ts";

type ProbeMode = "day" | "night";
export type GiProbeProfiles = Partial<Record<ProbeMode, GiProbeManifest>>;

/** Keeps a scenario's grid, alpha policy, records, and visibility atlas together. */
export function selectProbeProfile(
  manifest: GiProbeManifest,
  mode: ProbeMode,
): GiProbeManifest {
  const scenario = manifest.scenarios?.[mode];
  if (!scenario)
    throw new Error(`Prepare sky lighting for the ${mode} town scene.`);
  const selected: GiProbeManifest = {
    ...manifest,
    moments: scenario.moments ?? manifest.moments,
    scenarios: { [mode]: scenario },
  };
  validateProbeManifest(selected);
  return selected;
}

/** Owns independent day/night fields. A missing profile uses one shared inactive field. */
export class ProbeProfiles {
  private readonly textures = new Map<ProbeMode, ProbeTextures>();
  private readonly descriptions = new Map<ProbeMode, GiProbeManifest>();
  private disposed = false;
  private constructor(private readonly fallback: ProbeTextures) {}
  get bytes() {
    return (
      this.fallback.bytes +
      [...this.textures.values()].reduce((sum, value) => sum + value.bytes, 0)
    );
  }

  static async create(device: GPUDevice, profiles: GiProbeProfiles = {}) {
    const selected: GiProbeProfiles = {};
    for (const mode of ["day", "night"] as const) {
      const manifest = profiles[mode];
      if (manifest) selected[mode] = selectProbeProfile(manifest, mode);
    }
    const result = new ProbeProfiles(await ProbeTextures.create(device));
    try {
      for (const mode of ["day", "night"] as const) {
        const manifest = selected[mode];
        if (!manifest) continue;
        result.textures.set(mode, await ProbeTextures.create(device, manifest));
        result.descriptions.set(mode, manifest);
      }
      return result;
    } catch (error) {
      result.dispose();
      throw error;
    }
  }
  get(mode: ProbeMode): ProbeTextures {
    return this.textures.get(mode) ?? this.fallback;
  }
  manifest(mode: ProbeMode): GiProbeManifest | undefined {
    return this.descriptions.get(mode);
  }
  dispose() {
    if (this.disposed) return;
    this.disposed = true;
    for (const value of this.textures.values()) value.dispose();
    this.fallback.dispose();
  }
}
