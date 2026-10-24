export interface EnvironmentSettings {
  mode: "day" | "night";
  skySource: "generated" | "kit";
  exposure: number;
  sunIntensity: number;
  ambientIntensity: number;
  fogDensity: number;
  sunAzimuth: number;
  sunElevation: number;
  roughness: number;
  saturation: number;
  cloudCoverage: number;
  cloudSpeed: number;
  skyIntensity: number;
  /** Visible kit sky multiplier; omitted v1 settings use 1. */
  skyBackgroundIntensity?: number;
  moonIntensity: number;
  bloomStrength: number;
  normalStrength: number;
}
export const DEFAULT_ENVIRONMENT: EnvironmentSettings = {
  mode: "day",
  skySource: "generated",
  exposure: 1,
  sunIntensity: 25,
  ambientIntensity: 0.3,
  fogDensity: 0.0022,
  sunAzimuth: 225,
  sunElevation: 48,
  roughness: 1,
  saturation: 1,
  cloudCoverage: 0.45,
  cloudSpeed: 0.15,
  skyIntensity: 1,
  skyBackgroundIntensity: 1,
  moonIntensity: 25,
  bloomStrength: 0.12,
  normalStrength: 1,
};
export type NumericEnvironmentKey = Exclude<
  keyof EnvironmentSettings,
  "mode" | "skySource"
>;

export const ENVIRONMENT_RANGES: Record<
  NumericEnvironmentKey,
  [number, number]
> = {
  exposure: [0.1, 4],
  sunIntensity: [0, 100],
  ambientIntensity: [0, 8],
  fogDensity: [0, 0.06],
  sunAzimuth: [0, 360],
  sunElevation: [-20, 90],
  roughness: [0.1, 2],
  saturation: [0, 2],
  cloudCoverage: [0, 1],
  cloudSpeed: [0, 2],
  skyIntensity: [0, 3],
  skyBackgroundIntensity: [0, 4],
  moonIntensity: [0, 100],
  bloomStrength: [0, 1],
  normalStrength: [0, 2],
};
export function validateEnvironment(
  value: EnvironmentSettings,
): EnvironmentSettings {
  if (value.mode !== "day" && value.mode !== "night")
    throw new Error("Choose day or night.");
  if (value.skySource !== "generated" && value.skySource !== "kit")
    throw new Error("Choose a generated sky or the kit sky.");
  const result = {
    ...value,
    skyBackgroundIntensity:
      value.skyBackgroundIntensity === undefined
        ? 1
        : value.skyBackgroundIntensity,
  };
  for (const [key, [min, max]] of Object.entries(ENVIRONMENT_RANGES)) {
    const field = key as NumericEnvironmentKey;
    const amount = result[field];
    if (
      typeof amount !== "number" ||
      !Number.isFinite(amount) ||
      amount < min ||
      amount > max
    )
      throw new Error(`Adjust ${key} within its supported range.`);
    result[field] = amount;
  }
  return result;
}

/** Keeps imported defaults separate from each source's latest accepted artist setting. */
export class EnvironmentProfiles {
  readonly #fills = new Map<string, number>();
  readonly #glows = new Map<string, number>();
  #sourceIntensity: Record<"day" | "night", number> = { day: 1, night: 1 };
  #sourceBloom: Record<"day" | "night", number> = {
    day: DEFAULT_ENVIRONMENT.bloomStrength,
    night: DEFAULT_ENVIRONMENT.bloomStrength,
  };

  configureSourceBloom(values: Record<"day" | "night", number>): void {
    for (const value of Object.values(values))
      if (!Number.isFinite(value) || value < 0 || value > 1)
        throw new Error("Use a source light glow strength from 0 to 1.");
    this.#sourceBloom = { ...values };
  }

  configureSourceIntensity(
    values: Partial<Record<"day" | "night", number>>,
  ): void {
    for (const mode of ["day", "night"] as const) {
      const value = values[mode] ?? 1;
      if (!Number.isFinite(value) || value < 0 || value > 8)
        throw new Error("Use a source Skylight intensity from 0 to 8.");
    }
    this.#sourceIntensity = { day: values.day ?? 1, night: values.night ?? 1 };
  }
  sourceIntensity(mode: "day" | "night"): number {
    return this.#sourceIntensity[mode];
  }
  initial(settings: EnvironmentSettings): EnvironmentSettings {
    const accepted = validateEnvironment(settings);
    return {
      ...accepted,
      ambientIntensity:
        this.#fills.get(this.key(settings)) ?? this.defaultFill(settings),
      bloomStrength:
        this.#glows.get(this.key(settings)) ?? this.defaultGlow(settings),
    };
  }
  apply(
    previous: EnvironmentSettings,
    candidate: EnvironmentSettings,
    restoreSourceFill = false,
  ): EnvironmentSettings {
    const accepted = validateEnvironment(candidate);
    this.#fills.set(this.key(previous), previous.ambientIntensity);
    this.#glows.set(this.key(previous), previous.bloomStrength);
    if (restoreSourceFill && this.key(previous) !== this.key(accepted)) {
      accepted.ambientIntensity =
        this.#fills.get(this.key(accepted)) ?? this.defaultFill(accepted);
      accepted.bloomStrength =
        this.#glows.get(this.key(accepted)) ?? this.defaultGlow(accepted);
    }
    this.#fills.set(this.key(accepted), accepted.ambientIntensity);
    this.#glows.set(this.key(accepted), accepted.bloomStrength);
    return accepted;
  }
  private key(settings: EnvironmentSettings): string {
    return `${settings.mode}:${settings.skySource}`;
  }
  private defaultFill(settings: EnvironmentSettings): number {
    return settings.skySource === "kit"
      ? this.sourceIntensity(settings.mode)
      : DEFAULT_ENVIRONMENT.ambientIntensity;
  }
  private defaultGlow(settings: EnvironmentSettings): number {
    return settings.skySource === "kit"
      ? this.#sourceBloom[settings.mode]
      : DEFAULT_ENVIRONMENT.bloomStrength;
  }
}
