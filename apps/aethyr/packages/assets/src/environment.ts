import type { CompiledEnvironments } from "./scene.ts";

/** Select a scenario with its matching optional metadata; never borrow another mode. */
export function mergeEnvironmentScenario(
  previous: CompiledEnvironments | undefined,
  source: CompiledEnvironments,
  mode: "day" | "night",
): CompiledEnvironments {
  const map = source.scenarios[mode];
  if (!map) throw new Error("Prepare the lighting map for this town scenario.");
  const result: CompiledEnvironments = {
    schema: "aethyrforge.private-environments.v1",
    scenarios: { ...previous?.scenarios, [mode]: map },
    atmosphere: { ...previous?.atmosphere },
    fogAmbient: { ...previous?.fogAmbient },
  };
  if (source.atmosphere?.[mode] !== undefined)
    result.atmosphere![mode] = source.atmosphere[mode];
  else delete result.atmosphere![mode];
  if (source.fogAmbient?.[mode] !== undefined)
    result.fogAmbient![mode] = source.fogAmbient[mode];
  else delete result.fogAmbient![mode];
  return result;
}
