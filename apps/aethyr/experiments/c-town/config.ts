export type CTownBackend =
  "baseline" | "rust-packed" | "ts-packed" | "c" | "c-frame";
export function isFrozenMovementBackend(
  value: CTownBackend,
): value is "rust-packed" | "ts-packed" | "c" {
  return value === "rust-packed" || value === "ts-packed" || value === "c";
}
/** Current town code requires the current controller; historical movement stays reproducible at its pinned revision. */
export function assertCurrentControllerBackend(value: CTownBackend) {
  if (isFrozenMovementBackend(value))
    throw new Error(
      "This town needs the current character controller. Choose c-frame, or open the saved movement comparison described in experiments/c-town/README.md.",
    );
}
export function selectCTownBackend(value?: string): CTownBackend {
  if (value === undefined || value === "baseline") return "baseline";
  if (
    value === "rust-packed" ||
    value === "ts-packed" ||
    value === "c" ||
    value === "c-frame"
  )
    return value;
  throw new Error(
    "Choose baseline, rust-packed, ts-packed, c, or c-frame for the town experiment.",
  );
}
/** Deployment configuration only; world content cannot choose an engine implementation. */
export const C_TOWN_BACKEND = selectCTownBackend(
  import.meta.env?.VITE_C_TOWN_BACKEND,
);
export const C_TOWN_BENCHMARK = import.meta.env?.VITE_C_TOWN_BENCHMARK === "1";
