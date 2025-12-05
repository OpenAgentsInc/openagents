import type {
  ExecutionContext,
  HudMessage,
} from "../hud/protocol.js";
import type { ContainerRunOptions } from "./backend.js";

export interface SandboxHudAdapter {
  executionId: string;
  emitStart: (options: {
    command: string[];
    sandboxed: boolean;
    image: string;
    workdir: string;
    context?: ExecutionContext;
  }) => void;
  emitComplete: (exitCode: number, durationMs: number) => void;
  setSandboxed: (sandboxed: boolean) => void;
  callbacks: Pick<ContainerRunOptions, "onStdout" | "onStderr">;
}

const generateExecutionId = (): string =>
  `exec-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`;

/**
 * Adapter for emitting container_* HUD events in a consistent shape.
 * Keeps sandbox-runner logic focused on backend execution.
 */
export const createSandboxHudAdapter = (
  emitHud?: (message: HudMessage) => void,
): SandboxHudAdapter => {
  const executionId = generateExecutionId();
  let sandboxed = false;
  let stdoutSeq = 0;
  let stderrSeq = 0;

  const emitStart: SandboxHudAdapter["emitStart"] = ({
    command,
    sandboxed: sandboxedValue,
    image,
    workdir,
    context,
  }) => {
    sandboxed = sandboxedValue;
    stdoutSeq = 0;
    stderrSeq = 0;

    emitHud?.({
      type: "container_start",
      executionId,
      image,
      command,
      context: context ?? "verification",
      sandboxed,
      workdir,
      timestamp: new Date().toISOString(),
    });
  };

  const callbacks: SandboxHudAdapter["callbacks"] = {
    onStdout: (chunk: string) => {
      emitHud?.({
        type: "container_output",
        executionId,
        text: chunk,
        stream: "stdout",
        sequence: ++stdoutSeq,
        sandboxed,
      });
    },
    onStderr: (chunk: string) => {
      emitHud?.({
        type: "container_output",
        executionId,
        text: chunk,
        stream: "stderr",
        sequence: ++stderrSeq,
        sandboxed,
      });
    },
  };

  const emitComplete: SandboxHudAdapter["emitComplete"] = (exitCode, durationMs) => {
    emitHud?.({
      type: "container_complete",
      executionId,
      exitCode,
      durationMs,
      sandboxed,
    });
  };

  return {
    executionId,
    emitStart,
    emitComplete,
    setSandboxed: (sandboxedValue: boolean) => {
      sandboxed = sandboxedValue;
    },
    callbacks,
  };
};
