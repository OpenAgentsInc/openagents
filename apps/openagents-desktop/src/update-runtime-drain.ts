import {
  childRuntimeKinds,
  type ChildRuntimeDrainReceipt,
  type ChildRuntimeKind,
} from "./update-platform-applier.ts";

export type ChildRuntimeDrainer = Readonly<{
  kind: ChildRuntimeKind;
  drain: () => void | Promise<void>;
}>;

/** Bounded, all-settled drain: one wedged runtime cannot hide the others. */
export const drainChildRuntimes = async (
  input: Readonly<{
    drainers: ReadonlyArray<ChildRuntimeDrainer>;
    timeoutMs: number;
    now?: () => number;
    setTimer?: typeof setTimeout;
    clearTimer?: typeof clearTimeout;
  }>,
): Promise<ChildRuntimeDrainReceipt> => {
  const now = input.now ?? Date.now;
  const setTimer = input.setTimer ?? setTimeout;
  const clearTimer = input.clearTimer ?? clearTimeout;
  const startedAt = now();
  const byKind = new Map(input.drainers.map((drainer) => [drainer.kind, drainer]));
  const drained: ChildRuntimeKind[] = [];
  const timedOut: ChildRuntimeKind[] = [];

  await Promise.all(
    childRuntimeKinds.map(async (kind) => {
      const drainer = byKind.get(kind);
      if (drainer === undefined) {
        drained.push(kind);
        return;
      }
      let timer: ReturnType<typeof setTimeout> | undefined;
      const outcome = await Promise.race([
        Promise.resolve()
          .then(drainer.drain)
          .then(
            () => "drained" as const,
            () => "failed" as const,
          ),
        new Promise<"timeout">((resolve) => {
          timer = setTimer(() => resolve("timeout"), input.timeoutMs);
        }),
      ]);
      if (timer !== undefined) clearTimer(timer);
      if (outcome === "drained") drained.push(kind);
      else timedOut.push(kind);
    }),
  );

  return {
    ok: timedOut.length === 0,
    drained: childRuntimeKinds.filter((kind) => drained.includes(kind)),
    timedOut: childRuntimeKinds.filter((kind) => timedOut.includes(kind)),
    elapsedMs: Math.max(0, now() - startedAt),
  };
};
