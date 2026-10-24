import {
  C_TOWN_BACKEND,
  isFrozenMovementBackend,
} from "../../experiments/c-town/config.ts";
import type { CTownBackend } from "../../experiments/c-town/config.ts";
import type { PackedSimulation } from "../../experiments/c-town/simulation.ts";
import { RuntimeKernel } from "../../packages/runtime/kernel.ts";
import { readAsset } from "../../packages/assets/src/asset-download.ts";
export class Kernel {
  private packed?: PackedSimulation;
  private packedSteps = 0;
  private unsupportedMovementProfile = false;
  private status = { revision: 0, canUndo: false, canRedo: false };
  private constructor(
    private transport: RuntimeKernel,
    private backend: CTownBackend,
  ) {}
  static async create() {
    const authorBytes = await readAsset("/kernel.wasm", async (response) => {
      if (!response.ok)
        throw new Error("Build the Rust kernel before opening the world.");
      return response.arrayBuffer();
    });
    let packedBytes: ArrayBuffer | undefined;
    if (isFrozenMovementBackend(C_TOWN_BACKEND)) {
      const kind = C_TOWN_BACKEND === "c" ? "c" : "rust-packed";
      packedBytes = await readAsset(
        kind === "c"
          ? "/c-town-simulation.wasm"
          : "/c-town-rust-simulation.wasm",
        async (response) => {
          if (!response.ok)
            throw new Error(
              "Build the town experiment before opening this player.",
            );
          return response.arrayBuffer();
        },
      );
    }
    return Kernel.fromModules(C_TOWN_BACKEND, authorBytes, packedBytes);
  }
  /** Module injection for local experiments and conformance checks; world input cannot call this. */
  static async fromModules(
    backend: CTownBackend,
    authorBytes: BufferSource,
    packedBytes?: BufferSource,
  ) {
    const transport = await RuntimeKernel.fromBytes(authorBytes);
    const kernel = new Kernel(transport, backend);
    if (isFrozenMovementBackend(backend)) {
      if (!packedBytes)
        throw new Error(
          "Build the town experiment before opening this player.",
        );
      const { createPackedSimulation } =
        await import("../../experiments/c-town/simulation.ts");
      kernel.packed = await createPackedSimulation(
        backend === "c" ? "c" : "rust-packed",
        packedBytes,
      );
    }
    return kernel;
  }
  request<T = any>(message: object): T {
    if (!this.packed) return this.jsonRequest(message);
    const request = message as any;
    if (
      request.type === "set_static_collision" &&
      Array.isArray(request.triangles) &&
      request.triangles.length > 0
    ) {
      return this.withRuntime({
        ok: false,
        revision: this.status.revision,
        error: {
          code: "unsupported_collision_profile",
          message:
            "Open baseline or c-frame to use the town's triangle walking surfaces.",
        },
      });
    }
    if (this.unsupportedMovementProfile) return this.movementProfileError();
    if (request.type === "step") {
      this.packedSteps++;
      return {
        ...this.status,
        ...this.packed.step(request.elapsed, request.input),
      } as T;
    }
    if (request.type !== "init") {
      const player = this.packed.reply().player;
      const synchronized = this.jsonRequest<any>({
        type: "set_camera",
        position: player.position,
        yaw: player.yaw,
        pitch: player.pitch,
        mode: player.mode,
      });
      // Only the exact camera fields cross the closed authoring request boundary.
      if (!synchronized.ok) return this.withRuntime(synchronized);
    }
    if (request.type === "command") {
      const prepared = this.jsonRequest<any>({ ...request, type: "prepare" });
      if (prepared.ok) {
        try {
          this.packed.validateWorld(prepared);
        } catch (error) {
          return this.withRuntime({
            ok: false,
            revision: this.status.revision,
            error: {
              code: "experimental_world_limit",
              message:
                error instanceof Error
                  ? error.message
                  : "This world cannot run in the town experiment.",
            },
          });
        }
      }
      // Invalid commands still reach the authority so duplicate IDs retain their usual behavior.
    }
    const result = this.jsonRequest<any>(message);
    if (
      result.ok &&
      request.type === "init" &&
      result.player &&
      "velocity" in result.player
    ) {
      this.unsupportedMovementProfile = true;
      return this.movementProfileError();
    }
    if (result.ok && request.type === "prepare") {
      try {
        this.packed.validateWorld(result);
      } catch (error) {
        return this.withRuntime({
          ok: false,
          revision: this.status.revision,
          error: {
            code: "experimental_world_limit",
            message:
              error instanceof Error
                ? error.message
                : "This world cannot run in the town experiment.",
          },
        });
      }
    }
    if (!result.ok || request.type === "prepare")
      return this.withRuntime(result);
    if (request.type === "init") this.packed.initializeWorld(result);
    else if (request.type === "command" || request.type === "snapshot")
      this.packed.syncWorld(result);
    const replacing =
      request.type === "command" &&
      !result.duplicate &&
      request.operations?.some(
        (operation: any) => operation.op === "replace_content",
      );
    if (request.type === "set_camera")
      this.packed.setCamera(result.player, result.player.grounded);
    else if (replacing)
      this.packed.replaceContentCamera(result.player, result.player.grounded);
    this.status = {
      revision: result.revision,
      canUndo: result.canUndo,
      canRedo: result.canRedo,
    };
    return this.withRuntime(result);
  }
  private withRuntime<T = any>(result: any): T {
    if (!this.packed) return result;
    const { player, tick, droppedSeconds } = this.packed.reply();
    return { ...result, player, tick, droppedSeconds } as T;
  }
  private movementProfileError<T = any>(): T {
    return this.withRuntime({
      ok: false,
      revision: this.status.revision,
      error: {
        code: "unsupported_movement_profile",
        message:
          "Open baseline or c-frame for the current character controller. The packed movement experiment uses the earlier controller.",
      },
    });
  }
  setProfiling(enabled: boolean) {
    this.transport.setProfiling(enabled);
  }
  get lastRequestTimings() {
    return this.packed ? null : this.transport.lastRequestTimings;
  }
  benchmarkInfo() {
    return {
      backend: this.backend,
      simulation: this.packed
        ? this.backend === "c"
          ? "c-packed"
          : "rust-packed"
        : "current-rust-json",
      authorWasmBytes: this.transport.memoryBytes,
      packedWasmBytes: this.packed?.memoryBytes() ?? 0,
      authorCalls: this.transport.benchmarkInfo().authorCalls,
      packedSteps: this.packedSteps,
      speedScale: this.packed?.getSpeedScale() ?? 1,
    };
  }
  async reloadSimulation(bytes: ArrayBuffer) {
    if (!this.packed || this.backend !== "c")
      throw new Error("Open the C town to replace its movement module.");
    await this.packed.reload(bytes);
    return { ...this.packed.reply(), speedScale: this.packed.getSpeedScale() };
  }
  private jsonRequest<T = any>(message: object): T {
    return this.transport.request<T>(message);
  }
}
