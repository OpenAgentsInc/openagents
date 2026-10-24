import type { FrameTargetHz } from "./frame-pacer.ts";

export interface FrameBenchmarkRequest {
  op: "start" | "read" | "stop" | "pinActors";
  capacity?: number;
  route?: "static" | "sway";
  instrumented?: boolean;
  drain?: boolean;
  canonical?: boolean;
  targetHz?: FrameTargetHz;
}
export interface FrameSample {
  frame: number;
  at: number;
  callbackEnteredAt?: number;
  elapsedMs: number;
  stepMs: number | null;
  renderCpuMs: number | null;
  cullMs: number | null;
  clusterMs: number | null;
  submitted: boolean;
  gpuMs: number | null;
  fullGpuMs?: number | null;
  bridgeEncodeMs?: number | null;
  wasmRequestMs?: number | null;
  bridgeDecodeMs?: number | null;
  bridgeOverheadMs?: number | null;
  commandEncodeMs?: number | null;
  kernelDroppedSeconds?: number;
  hostClampedSeconds?: number;
  submittedInstances?: number;
  submittedTriangles?: number;
  /** Executed source draw-loop commands; includes admitted zero-instance bundle slots. */
  sourceRenderCommands?: {
    draws: number;
    pipelines: number;
    materials: number;
    windBindings: number;
  };
  cameraEye?: [number, number, number];
  cameraYaw?: number;
  cameraPitch?: number;
  animationTime?: number;
}
export interface FrameCompletionSnapshot {
  completionRange: { firstFrame: number | null; lastFrame: number | null };
  completedFrames: Array<{
    frame: number;
    submittedAt: number;
    submitReturnedAt?: number;
    completedAt: number;
    latencyMs: number;
    ok: boolean;
  }>;
  completionTruncated: boolean;
}
export interface FrameBenchmarkResult<
  Variant extends string = string,
> extends FrameCompletionSnapshot {
  variant: Variant;
  recording: boolean;
  instrumented: boolean;
  capacity: number;
  route: "static" | "sway";
  targetHz: FrameTargetHz;
  sampleScope: "render-attempts";
  callbackClock?: "entry";
  callbackCounts: {
    rawCallbacks: number;
    intentionalPacedCallbacks: number;
    suspendedCallbacks: number;
    renderAttempts: number;
  };
  truncated: boolean;
  samples: FrameSample[];
  observation: { startedAtMs: number | null; stoppedAtMs: number | null };
  resources: Record<string, unknown>;
}
export function validateFrameRequest(value: FrameBenchmarkRequest) {
  if (
    !value ||
    !["start", "read", "stop", "pinActors"].includes(value.op) ||
    (value.op === "pinActors" &&
      (value.capacity !== undefined ||
        value.route !== undefined ||
        value.instrumented !== undefined ||
        value.targetHz !== undefined)) ||
    (value.targetHz !== undefined &&
      (value.op !== "start" ||
        (value.targetHz !== 60 && value.targetHz !== "display"))) ||
    (value.canonical !== undefined &&
      (value.op !== "pinActors" || typeof value.canonical !== "boolean")) ||
    (value.drain !== undefined && typeof value.drain !== "boolean") ||
    (value.op === "start" && value.drain === true) ||
    (value.instrumented !== undefined &&
      typeof value.instrumented !== "boolean") ||
    (value.capacity !== undefined &&
      (!Number.isSafeInteger(value.capacity) ||
        value.capacity < 1 ||
        value.capacity > 20_000)) ||
    (value.route !== undefined && !["static", "sway"].includes(value.route))
  )
    throw new Error("Choose a bounded frame benchmark request.");
}
/** Explicitly bounded observation; samples stop accumulating at capacity. */
export class FrameMetrics<Variant extends string = string> {
  recording = false;
  instrumented = true;
  capacity = 20_000;
  route: "static" | "sway" = "static";
  targetHz: FrameTargetHz = 60;
  truncated = false;
  private samples: FrameSample[] = [];
  private startedAtMs: number | null = null;
  private stoppedAtMs: number | null = null;
  private rawCallbacks = 0;
  private intentionalPacedCallbacks = 0;
  private suspendedCallbacks = 0;
  start(request: FrameBenchmarkRequest) {
    validateFrameRequest(request);
    this.capacity = request.capacity ?? 20_000;
    this.instrumented = request.instrumented ?? true;
    this.route = request.route ?? "static";
    this.targetHz = request.targetHz ?? 60;
    this.rawCallbacks = this.intentionalPacedCallbacks = 0;
    this.suspendedCallbacks = 0;
    this.samples = [];
    this.truncated = false;
    this.startedAtMs = performance.now();
    this.stoppedAtMs = null;
    this.recording = true;
  }
  stop() {
    if (this.recording) this.stoppedAtMs = performance.now();
    this.recording = false;
  }
  record(sample: FrameSample) {
    if (!this.recording) return;
    if (this.samples.length === this.capacity) {
      this.truncated = true;
      return;
    }
    this.samples.push(sample);
  }
  observeCallback(due: boolean | null): void {
    if (!this.recording) return;
    this.rawCallbacks++;
    this.intentionalPacedCallbacks += Number(due === false);
    this.suspendedCallbacks += Number(due === null);
  }
  result(
    variant: Variant,
    resources: Record<string, unknown>,
    gpu: (frame: number) => number | null,
    fullGpu: (frame: number) => number | null = () => null,
    completions: FrameCompletionSnapshot = {
      completionRange: { firstFrame: null, lastFrame: null },
      completedFrames: [],
      completionTruncated: false,
    },
  ): FrameBenchmarkResult<Variant> {
    return {
      ...completions,
      variant,
      recording: this.recording,
      instrumented: this.instrumented,
      capacity: this.capacity,
      route: this.route,
      targetHz: this.targetHz,
      sampleScope: "render-attempts",
      callbackClock: "entry",
      callbackCounts: {
        rawCallbacks: this.rawCallbacks,
        intentionalPacedCallbacks: this.intentionalPacedCallbacks,
        suspendedCallbacks: this.suspendedCallbacks,
        renderAttempts:
          this.rawCallbacks -
          this.intentionalPacedCallbacks -
          this.suspendedCallbacks,
      },
      truncated: this.truncated,
      resources,
      observation: {
        startedAtMs: this.startedAtMs,
        stoppedAtMs: this.stoppedAtMs,
      },
      samples: this.samples.map((s) => ({
        ...s,
        cameraEye: s.cameraEye ? [...s.cameraEye] : undefined,
        gpuMs: s.submitted && this.instrumented ? gpu(s.frame) : null,
        fullGpuMs: s.submitted && this.instrumented ? fullGpu(s.frame) : null,
      })),
    };
  }
}
