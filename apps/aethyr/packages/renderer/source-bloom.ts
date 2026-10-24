import type { SceneLook } from "../assets/src/scene.ts";

/** Known unbound source overrides; inherited engine defaults remain caller-owned. */
export function sourceBloomStrength(
  look: SceneLook | undefined,
  mode: "day" | "night",
  inherited: number,
): number {
  if (!Number.isFinite(inherited) || inherited < 0 || inherited > 1)
    throw new Error("Use a light glow strength from 0 to 1.");
  const volumes = (look?.scenarios[mode].environment ?? [])
    .map((actor, order) => ({ actor, order }))
    .filter(({ actor }) => {
      const p = actor.properties as Record<string, unknown> | undefined;
      return (
        actor.class === "PostProcessVolume" &&
        p?.enabled === true &&
        p.unbound === true &&
        typeof p.blend_weight === "number" &&
        Number.isFinite(p.blend_weight) &&
        p.blend_weight >= 0 &&
        p.blend_weight <= 1 &&
        (p.priority === undefined ||
          (typeof p.priority === "number" && Number.isFinite(p.priority)))
      );
    })
    .sort((a, b) => {
      const ap = a.actor.properties as Record<string, unknown>;
      const bp = b.actor.properties as Record<string, unknown>;
      return (
        Number(ap.priority ?? 0) - Number(bp.priority ?? 0) || a.order - b.order
      );
    });
  let value = inherited;
  for (const { actor } of volumes) {
    const p = actor.properties as Record<string, unknown>;
    if (typeof p.settings !== "string") continue;
    const setting = (key: string) =>
      new RegExp(`(?:^|[,(])\\s*${key}\\s*=\\s*([^,()]+)`)
        .exec(p.settings as string)?.[1]
        .trim();
    if (setting("bOverride_BloomIntensity") !== "True") continue;
    const raw = setting("BloomIntensity");
    const candidate = raw === undefined || raw === "" ? NaN : Number(raw);
    // The local editor supports this bounded range. Unsupported source values
    // retain the caller's inherited setting rather than being silently clipped.
    if (!Number.isFinite(candidate) || candidate < 0 || candidate > 1) continue;
    value += (candidate - value) * Number(p.blend_weight);
  }
  return value;
}
