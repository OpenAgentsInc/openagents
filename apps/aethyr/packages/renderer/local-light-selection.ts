import type { SourceLight } from "../assets/src/scene.ts";

export const LOCAL_LIGHT_LIMIT = 32;
interface Candidate {
  light: SourceLight | undefined;
  distance: number;
}

/** Reuse a bounded nearest-light list while preserving source order on ties. */
export class LocalLightSelection {
  private records: Candidate[] = Array.from(
    { length: LOCAL_LIGHT_LIMIT },
    () => ({
      light: undefined,
      distance: 0,
    }),
  );
  private selected: SourceLight[] = [];

  /** The returned array belongs to this selector and is reused on the next call. */
  select(
    lights: readonly SourceLight[] | undefined,
    eye: ArrayLike<number>,
  ): SourceLight[] {
    let count = 0;
    if (lights)
      for (let index = 0; index < lights.length; index++) {
        const light = lights[index];
        if (!(light.enabled && light.range && light.range > 0)) continue;
        const distance = Math.hypot(
          light.position[0] - eye[0],
          light.position[1] - eye[1],
          light.position[2] - eye[2],
        );
        if (!(distance < (light.range ?? 0) + 45)) continue;
        if (
          count === LOCAL_LIGHT_LIMIT &&
          !(distance < this.records[count - 1].distance)
        )
          continue;
        const slot = this.records[Math.min(count, LOCAL_LIGHT_LIMIT - 1)];
        let insert = Math.min(count, LOCAL_LIGHT_LIMIT - 1);
        while (insert > 0 && distance < this.records[insert - 1].distance) {
          this.records[insert] = this.records[insert - 1];
          insert--;
        }
        slot.light = light;
        slot.distance = distance;
        this.records[insert] = slot;
        if (count < LOCAL_LIGHT_LIMIT) count++;
      }
    this.selected.length = count;
    for (let index = 0; index < count; index++)
      this.selected[index] = this.records[index].light!;
    for (let index = count; index < LOCAL_LIGHT_LIMIT; index++)
      this.records[index].light = undefined;
    return this.selected;
  }
}
