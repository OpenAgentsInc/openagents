import {
  validateEnvironment,
  type EnvironmentSettings,
} from "./environment.ts";

export interface OverviewPresentation {
  camera: {
    position: [number, number, number];
    target: [number, number, number];
    up: [number, number, number];
    verticalFovDegrees: number;
  };
  environment: EnvironmentSettings;
}

/** Read only camera/daylight data; resource and executable origins stay deployment-owned. */
export function parseOverviewPresentation(
  value: unknown,
): OverviewPresentation {
  const failure =
    "Prepare the overview camera and daylight settings with the current asset tools.";
  try {
    if (!value || typeof value !== "object" || Array.isArray(value))
      throw new Error(failure);
    const data = value as Record<string, unknown>;
    if (data.schema !== "aethyrforge.owned-overview-presentation.v1")
      throw new Error(failure);
    const camera = data.camera as Record<string, unknown>;
    const lighting = data.lighting as Record<string, unknown>;
    if (
      !camera ||
      !lighting ||
      lighting.kind !== "owned-overview-daylight-v1" ||
      lighting.sourceOverviewLightingRecovered !== false
    )
      throw new Error(failure);
    const vector = (input: unknown): [number, number, number] => {
      if (
        !Array.isArray(input) ||
        input.length !== 3 ||
        !input.every(
          (n) =>
            typeof n === "number" &&
            Number.isFinite(n) &&
            Math.abs(n) <= 1_000_000,
        )
      )
        throw new Error(failure);
      return [input[0], input[1], input[2]];
    };
    const position = vector(camera.position),
      target = vector(camera.target),
      up = vector(camera.up);
    const direction = target.map((n, i) => n - position[i]);
    const cross = [
      direction[1] * up[2] - direction[2] * up[1],
      direction[2] * up[0] - direction[0] * up[2],
      direction[0] * up[1] - direction[1] * up[0],
    ];
    if (
      Math.hypot(...direction) < 0.0001 ||
      Math.hypot(...up) < 0.0001 ||
      Math.hypot(...cross) < 0.0001 ||
      typeof camera.verticalFovDegrees !== "number" ||
      !Number.isFinite(camera.verticalFovDegrees) ||
      camera.verticalFovDegrees < 0.1 ||
      camera.verticalFovDegrees > 150
    )
      throw new Error(failure);
    if (!lighting.environment || typeof lighting.environment !== "object")
      throw new Error(failure);
    const environment = validateEnvironment(
      lighting.environment as EnvironmentSettings,
    );
    if (environment.mode !== "day" || environment.skySource !== "kit")
      throw new Error(failure);
    return {
      camera: {
        position,
        target,
        up,
        verticalFovDegrees: camera.verticalFovDegrees,
      },
      environment,
    };
  } catch {
    throw new Error(failure);
  }
}
