import {
  OMEGA_EFFECTD_MAX_FRAME_BYTES,
  OMEGA_EFFECTD_MAX_HOST_REQUESTS,
  OMEGA_EFFECTD_PROTOCOL_SCHEMA,
  redactDiagnosticText,
  type OmegaEffectdHostMethod,
  type OmegaEffectdHostRequest,
  type OmegaEffectdHostResponse,
} from "./framed.ts";

export class OmegaEffectdHostBridgeError extends Error {
  readonly _tag = "OmegaEffectdHostBridgeError";
  override readonly name = "OmegaEffectdHostBridgeError";

  constructor(
    readonly reason:
      | "host_unavailable"
      | "stale_generation"
      | "invalid_request"
      | "unsupported"
      | "unavailable"
      | "internal"
      | "invalid_response"
      | "request_limit"
      | "frame_too_large"
      | "timeout",
    message: string,
  ) {
    super(redactDiagnosticText(message));
  }
}

export type OmegaEffectdHostFrameEmitter = (frame: OmegaEffectdHostRequest) => void | Promise<void>;

type PendingHostRequest = Readonly<{
  generation: number;
  resolve: (value: unknown) => void;
  reject: (error: OmegaEffectdHostBridgeError) => void;
  cancelTimeout: () => void;
}>;

export type OmegaEffectdHostBridgeOptions = Readonly<{
  requestTimeoutMs?: number;
  scheduleTimeout?: (callback: () => void, durationMs: number) => () => void;
}>;

export type HostReplyDisposition = "accepted" | "stale_generation" | "unknown_or_late";

export class OmegaEffectdHostBridge {
  readonly #pending = new Map<string, PendingHostRequest>();
  #generation = 0;
  #counter = 0;
  readonly #requestTimeoutMs: number;
  readonly #scheduleTimeout: (callback: () => void, durationMs: number) => () => void;

  constructor(
    private emit: OmegaEffectdHostFrameEmitter | undefined,
    options: OmegaEffectdHostBridgeOptions = {},
  ) {
    this.#requestTimeoutMs = options.requestTimeoutMs ?? 30_000;
    this.#scheduleTimeout =
      options.scheduleTimeout ??
      ((callback, durationMs) => {
        const timeout = setTimeout(callback, durationMs);
        return () => clearTimeout(timeout);
      });
  }

  setEmitter(emit: OmegaEffectdHostFrameEmitter): void {
    this.emit = emit;
  }

  beginGeneration(generation: number): void {
    this.rejectPending(
      "stale_generation",
      "The supervisor generation changed before the host operation completed.",
    );
    this.#generation = generation;
    this.#counter = 0;
  }

  rejectPending(reason: OmegaEffectdHostBridgeError["reason"], message: string): void {
    for (const pending of this.#pending.values()) {
      pending.cancelTimeout();
      pending.reject(new OmegaEffectdHostBridgeError(reason, message));
    }
    this.#pending.clear();
  }

  request(method: OmegaEffectdHostMethod, params: unknown): Promise<unknown> {
    if (this.emit === undefined) {
      return Promise.reject(
        new OmegaEffectdHostBridgeError(
          "host_unavailable",
          "The Omega host bridge is unavailable.",
        ),
      );
    }
    if (this.#generation < 1) {
      return Promise.reject(
        new OmegaEffectdHostBridgeError(
          "host_unavailable",
          "The Omega host bridge has not been initialized.",
        ),
      );
    }
    if (this.#pending.size >= OMEGA_EFFECTD_MAX_HOST_REQUESTS) {
      return Promise.reject(
        new OmegaEffectdHostBridgeError(
          "request_limit",
          "The Omega host bridge request limit was reached.",
        ),
      );
    }
    const id = `host.${this.#generation}.${++this.#counter}`;
    const frame: OmegaEffectdHostRequest = {
      schema: OMEGA_EFFECTD_PROTOCOL_SCHEMA,
      kind: "host_request",
      id,
      generation: this.#generation,
      method,
      params,
    };
    if (Buffer.byteLength(JSON.stringify(frame), "utf8") > OMEGA_EFFECTD_MAX_FRAME_BYTES) {
      return Promise.reject(
        new OmegaEffectdHostBridgeError(
          "frame_too_large",
          "The Omega host request exceeded the frame-size limit.",
        ),
      );
    }
    return new Promise((resolve, reject) => {
      const cancelTimeout = this.#scheduleTimeout(() => {
        const pending = this.#pending.get(id);
        if (pending === undefined) return;
        this.#pending.delete(id);
        pending.reject(
          new OmegaEffectdHostBridgeError(
            "timeout",
            `The Omega host did not answer ${method} within the request deadline.`,
          ),
        );
      }, this.#requestTimeoutMs);
      this.#pending.set(id, {
        generation: this.#generation,
        resolve,
        reject,
        cancelTimeout,
      });
      Promise.resolve(this.emit?.(frame)).catch((error) => {
        const pending = this.#pending.get(id);
        if (pending === undefined) return;
        this.#pending.delete(id);
        pending.cancelTimeout();
        pending.reject(
          new OmegaEffectdHostBridgeError(
            "host_unavailable",
            error instanceof Error ? error.message : "The Omega host bridge write failed.",
          ),
        );
      });
    });
  }

  accept(response: OmegaEffectdHostResponse): HostReplyDisposition {
    if (response.generation !== this.#generation) return "stale_generation";
    const pending = this.#pending.get(response.id);
    if (pending === undefined) return "unknown_or_late";
    if (pending.generation !== response.generation) return "stale_generation";
    this.#pending.delete(response.id);
    pending.cancelTimeout();
    if (response.ok) {
      pending.resolve(response.result);
    } else {
      const reason = (() => {
        switch (response.error?.code) {
          case "stale_generation":
          case "invalid_request":
          case "unsupported":
          case "unavailable":
          case "internal":
            return response.error.code;
          default:
            return "invalid_response";
        }
      })();
      pending.reject(
        new OmegaEffectdHostBridgeError(
          reason,
          response.error?.message ?? "The Omega host rejected the operation.",
        ),
      );
    }
    return "accepted";
  }
}
