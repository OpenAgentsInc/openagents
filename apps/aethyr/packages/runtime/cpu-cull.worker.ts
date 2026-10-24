/** Trusted CPU helper: immutable local bounds, owned scratch, no GPU or world access. */
import { frustumVisibility, type InstanceBounds } from "../renderer/culling.ts";
import type { CpuCullWorkerReply, CpuCullWorkerRequest } from "./cpu-cull.ts";
import { TaskOwnership } from "./task-ownership.ts";

/** The test port executes this exact worker handler, including the production kernel. */
export function createCpuCullWorkerHandler() {
  let bounds: InstanceBounds | undefined;
  const ownership = new TaskOwnership<{
    bounds: InstanceBounds | undefined;
    matrix: Float64Array<ArrayBuffer>;
    mask: Uint8Array<ArrayBuffer> | undefined;
  }>();
  ownership.resource("bounds", undefined, "cpu-helper");
  ownership.resource("matrix", new Float64Array(0), "cpu-helper");
  ownership.resource("mask", undefined, "cpu-helper");
  ownership.define({
    id: "frustum",
    owner: "cpu-helper",
    reads: ["bounds", "matrix"],
    writes: ["mask"],
  });
  let epoch = -1,
    part = -1,
    sequence = 0;
  return (
    message: CpuCullWorkerRequest,
  ): { reply: CpuCullWorkerReply; transfer: Transferable[] } => {
    try {
      if (message.type === "install") {
        if (
          !Number.isSafeInteger(message.epoch) ||
          message.epoch <= epoch ||
          ![0, 1].includes(message.part) ||
          !Number.isSafeInteger(message.count) ||
          message.count < 0 ||
          message.count > 1_000_000 ||
          !(message.spheres instanceof Float64Array) ||
          !(message.aabbs instanceof Float64Array) ||
          message.spheres.length !== message.count * 4 ||
          message.aabbs.length !== message.count * 6
        )
          throw new Error("Invalid CPU bounds installation.");
        bounds = {
          count: message.count,
          spheres: message.spheres,
          aabbs: message.aabbs,
        };
        ownership.safePoint(() => {
          ownership.replace("bounds", bounds);
          ownership.replace("matrix", new Float64Array(0));
          ownership.replace("mask", undefined);
        });
        epoch = message.epoch;
        part = message.part;
        sequence = 0;
        return {
          reply: { type: "ready", epoch, part, count: bounds.count },
          transfer: [],
        };
      }
      if (
        !bounds ||
        message.type !== "run" ||
        message.epoch !== epoch ||
        !Number.isSafeInteger(message.sequence) ||
        message.sequence <= sequence
      )
        throw new Error("The CPU probe does not match its resident bounds.");
      sequence = message.sequence;
      const started = performance.now(),
        mask = new Uint8Array(bounds.count);
      ownership.safePoint(() => {
        ownership.replace("matrix", message.matrix);
        ownership.replace("mask", mask);
      });
      ownership.run("frustum", (context) =>
        context.read("bounds").withView((resident) =>
          context.read("matrix").withView((matrix) =>
            context.write("mask").withView((output) => {
              if (!resident || !output)
                throw new Error("Install the CPU task buffers first.");
              frustumVisibility(resident, matrix, output);
            }),
          ),
        ),
      );
      // Only the independently owned output leaves the synchronous task scope.
      ownership.safePoint(() => ownership.replace("mask", undefined));
      const computeMs = performance.now() - started;
      return {
        reply: {
          type: "result",
          epoch,
          part,
          sequence,
          count: bounds.count,
          mask: mask.buffer,
          computeMs,
          taskRuns: ownership.snapshot().runs,
        },
        transfer: [mask.buffer],
      };
    } catch (error) {
      return {
        reply: {
          type: "error",
          epoch: message.epoch,
          part: "part" in message ? message.part : part,
          ...("sequence" in message ? { sequence: message.sequence } : {}),
          message: (error instanceof Error
            ? error.message
            : "CPU probe failed."
          ).slice(0, 512),
        },
        transfer: [],
      };
    }
  };
}

if (
  typeof WorkerGlobalScope !== "undefined" &&
  globalThis instanceof WorkerGlobalScope
) {
  const scope = globalThis as unknown as DedicatedWorkerGlobalScope,
    handle = createCpuCullWorkerHandler();
  scope.onmessage = (event: MessageEvent<CpuCullWorkerRequest>) => {
    const result = handle(event.data);
    scope.postMessage(result.reply, result.transfer);
  };
}
