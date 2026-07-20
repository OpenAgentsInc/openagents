import { Exit, Schema } from "effect";

const DAP_MAX_SEQUENCE = 2_147_483_647;
const DAP_DEFAULT_HEADER_BYTES = 8_192;
const DAP_DEFAULT_BODY_BYTES = 4 * 1_024 * 1_024;
const DAP_DEFAULT_PENDING_REQUESTS = 256;

const DapSequenceSchema = Schema.Number.check(
  Schema.isInt(),
  Schema.isBetween({ minimum: 1, maximum: DAP_MAX_SEQUENCE }),
);
const DapNameSchema = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(256));

export const DapRequestSchema = Schema.Struct({
  seq: DapSequenceSchema,
  type: Schema.Literal("request"),
  command: DapNameSchema,
  arguments: Schema.optionalKey(Schema.Json),
}).annotate({ identifier: "DapRequest" });
export interface DapRequest extends Schema.Schema.Type<typeof DapRequestSchema> {}

export const DapResponseSchema = Schema.Struct({
  seq: DapSequenceSchema,
  type: Schema.Literal("response"),
  request_seq: DapSequenceSchema,
  success: Schema.Boolean,
  command: DapNameSchema,
  message: Schema.optionalKey(Schema.String.check(Schema.isMaxLength(16_384))),
  body: Schema.optionalKey(Schema.Json),
}).annotate({ identifier: "DapResponse" });
export interface DapResponse extends Schema.Schema.Type<typeof DapResponseSchema> {}

export const DapEventSchema = Schema.Struct({
  seq: DapSequenceSchema,
  type: Schema.Literal("event"),
  event: DapNameSchema,
  body: Schema.optionalKey(Schema.Json),
}).annotate({ identifier: "DapEvent" });
export interface DapEvent extends Schema.Schema.Type<typeof DapEventSchema> {}

export const DapProtocolMessageSchema = Schema.Union([
  DapRequestSchema,
  DapResponseSchema,
  DapEventSchema,
]).annotate({ identifier: "DapProtocolMessage" });
export type DapProtocolMessage = typeof DapProtocolMessageSchema.Type;
export type DapJson = typeof Schema.Json.Type;

const decodeDapProtocolMessageExit = Schema.decodeUnknownExit(DapProtocolMessageSchema, {
  onExcessProperty: "preserve",
});
const decodeDapRequestExit = Schema.decodeUnknownExit(DapRequestSchema);

export const DapTransportFailurePhaseSchema = Schema.Literals([
  "header",
  "body",
  "json",
  "message",
  "request",
  "response",
  "cancel",
  "teardown",
]);
export type DapTransportFailurePhase = typeof DapTransportFailurePhaseSchema.Type;

export class DapTransportFailure extends Schema.TaggedErrorClass<DapTransportFailure>()(
  "DapTransportFailure",
  {
    phase: DapTransportFailurePhaseSchema,
    detail: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(1_024)),
    retryable: Schema.Boolean,
  },
) {
  override get message(): string {
    return this.detail;
  }
}

const boundedDetail = (detail: string, fallback: string): string => {
  const normalized = detail.trim();
  return (normalized.length === 0 ? fallback : normalized).slice(0, 1_024);
};

export const dapTransportFailure = (
  phase: DapTransportFailurePhase,
  detail: string,
  retryable: boolean,
): DapTransportFailure =>
  new DapTransportFailure({
    phase,
    detail: boundedDetail(detail, "DAP transport failed."),
    retryable,
  });

export interface DapDecoderOptions {
  readonly maxHeaderBytes?: number;
  readonly maxBodyBytes?: number;
  readonly maxBufferedBytes?: number;
}

const parseHeader = (bytes: Buffer, maxBodyBytes: number): number => {
  if (bytes.some((byte) => byte > 0x7f)) {
    throw dapTransportFailure("header", "DAP headers must contain ASCII bytes only.", false);
  }

  const values = new Map<string, string>();
  for (const line of bytes.toString("ascii").split("\r\n")) {
    const separator = line.indexOf(":");
    if (separator <= 0) {
      throw dapTransportFailure(
        "header",
        "DAP header lines must contain a field name and colon.",
        false,
      );
    }
    const name = line.slice(0, separator).trim().toLowerCase();
    const value = line.slice(separator + 1).trim();
    if (!/^[a-z][a-z0-9-]*$/u.test(name) || value.length === 0) {
      throw dapTransportFailure("header", "DAP header field syntax is invalid.", false);
    }
    if (values.has(name)) {
      throw dapTransportFailure("header", `DAP header ${name} was repeated.`, false);
    }
    values.set(name, value);
  }

  const contentLength = values.get("content-length");
  if (contentLength === undefined || !/^(?:0|[1-9][0-9]*)$/u.test(contentLength)) {
    throw dapTransportFailure("header", "DAP Content-Length is missing or invalid.", false);
  }
  const length = Number(contentLength);
  if (!Number.isSafeInteger(length) || length < 2 || length > maxBodyBytes) {
    throw dapTransportFailure(
      "body",
      `DAP body length ${contentLength} is outside the admitted limit.`,
      false,
    );
  }
  return length;
};

const decodeMessage = (body: Buffer): DapProtocolMessage => {
  let text: string;
  try {
    text = new TextDecoder("utf-8", { fatal: true }).decode(body);
  } catch {
    throw dapTransportFailure("json", "DAP body is not valid UTF-8.", false);
  }

  let parsed: unknown;
  try {
    parsed = JSON.parse(text);
  } catch {
    throw dapTransportFailure("json", "DAP body is not valid JSON.", false);
  }

  const decoded = decodeDapProtocolMessageExit(parsed);
  if (Exit.isFailure(decoded)) {
    throw dapTransportFailure(
      "message",
      "DAP body does not match a request, response, or event envelope.",
      false,
    );
  }
  return decoded.value;
};

export interface DapMessageDecoder {
  readonly push: (chunk: Uint8Array) => ReadonlyArray<DapProtocolMessage>;
  readonly finish: () => void;
  readonly bufferedBytes: () => number;
}

export const makeDapMessageDecoder = (options: DapDecoderOptions = {}): DapMessageDecoder => {
  const maxHeaderBytes = options.maxHeaderBytes ?? DAP_DEFAULT_HEADER_BYTES;
  const maxBodyBytes = options.maxBodyBytes ?? DAP_DEFAULT_BODY_BYTES;
  const maxBufferedBytes = options.maxBufferedBytes ?? maxHeaderBytes + maxBodyBytes + 4;
  if (
    !Number.isInteger(maxHeaderBytes) ||
    maxHeaderBytes < 32 ||
    !Number.isInteger(maxBodyBytes) ||
    maxBodyBytes < 2 ||
    !Number.isInteger(maxBufferedBytes) ||
    maxBufferedBytes < maxHeaderBytes + 4
  ) {
    throw dapTransportFailure("header", "DAP decoder limits are invalid.", false);
  }

  let buffered = Buffer.alloc(0);
  let expectedBodyBytes: number | null = null;

  const push = (chunk: Uint8Array): ReadonlyArray<DapProtocolMessage> => {
    if (chunk.byteLength > 0) {
      if (buffered.byteLength + chunk.byteLength > maxBufferedBytes) {
        throw dapTransportFailure(
          "body",
          "DAP buffered input exceeded the admitted byte limit.",
          false,
        );
      }
      buffered = Buffer.concat([buffered, Buffer.from(chunk)]);
    }

    const messages: DapProtocolMessage[] = [];
    while (true) {
      if (expectedBodyBytes === null) {
        const boundary = buffered.indexOf("\r\n\r\n");
        if (boundary < 0) {
          if (buffered.byteLength > maxHeaderBytes) {
            throw dapTransportFailure(
              "header",
              "DAP header exceeded the admitted byte limit.",
              false,
            );
          }
          break;
        }
        if (boundary === 0 || boundary > maxHeaderBytes) {
          throw dapTransportFailure(
            "header",
            "DAP header is empty or exceeds the admitted byte limit.",
            false,
          );
        }
        expectedBodyBytes = parseHeader(buffered.subarray(0, boundary), maxBodyBytes);
        buffered = buffered.subarray(boundary + 4);
      }

      if (buffered.byteLength < expectedBodyBytes) break;
      const body = buffered.subarray(0, expectedBodyBytes);
      buffered = buffered.subarray(expectedBodyBytes);
      expectedBodyBytes = null;
      messages.push(decodeMessage(body));
    }
    return messages;
  };

  return {
    push,
    finish: () => {
      if (expectedBodyBytes !== null || buffered.byteLength !== 0) {
        throw dapTransportFailure(
          "teardown",
          "DAP transport ended with an incomplete message.",
          true,
        );
      }
    },
    bufferedBytes: () => buffered.byteLength,
  };
};

export const encodeDapProtocolMessage = (message: DapProtocolMessage): Buffer => {
  const decoded = decodeDapProtocolMessageExit(message);
  if (Exit.isFailure(decoded)) {
    throw dapTransportFailure("message", "Cannot encode an invalid DAP envelope.", false);
  }
  const body = Buffer.from(JSON.stringify(decoded.value), "utf8");
  if (body.byteLength > DAP_DEFAULT_BODY_BYTES) {
    throw dapTransportFailure(
      "body",
      "Cannot encode a DAP body above the admitted byte limit.",
      false,
    );
  }
  return Buffer.concat([Buffer.from(`Content-Length: ${body.byteLength}\r\n\r\n`, "ascii"), body]);
};

export interface DapRequestOptions {
  readonly signal?: AbortSignal;
}

export type DapTimeoutScheduler = (timeoutMs: number, onTimeout: () => void) => () => void;

export interface DapRequestBrokerOptions {
  readonly timeoutMs?: number;
  readonly maxPendingRequests?: number;
  readonly onSend: (request: DapRequest) => void;
  readonly scheduleTimeout?: DapTimeoutScheduler;
}

export interface DapPendingRequest {
  readonly request: DapRequest;
  readonly response: Promise<DapResponse>;
  readonly cancel: (detail?: string) => boolean;
}

export interface DapRequestBroker {
  readonly request: (
    command: string,
    argumentsValue?: DapJson,
    options?: DapRequestOptions,
  ) => DapPendingRequest;
  readonly accept: (response: DapResponse) => boolean;
  readonly failAll: (detail: string) => void;
  readonly pendingCount: () => number;
}

type Pending = Readonly<{
  command: string;
  resolve: (response: DapResponse) => void;
  reject: (failure: DapTransportFailure) => void;
  cancelTimeout: () => void;
  removeAbortListener: () => void;
}>;

const defaultScheduleTimeout: DapTimeoutScheduler = (timeoutMs, onTimeout) => {
  const timer = setTimeout(onTimeout, timeoutMs);
  timer.unref?.();
  return () => clearTimeout(timer);
};

const ignoreResolvedValue = <A>(_value: A): void => undefined;
const ignoreRejectedFailure = (_failure: DapTransportFailure): void => undefined;

const makePromiseResolvers = <A>(): Readonly<{
  promise: Promise<A>;
  resolve: (value: A) => void;
  reject: (failure: DapTransportFailure) => void;
}> => {
  let resolveValue = ignoreResolvedValue<A>;
  let rejectValue = ignoreRejectedFailure;
  const promise = new Promise<A>((resolve, reject) => {
    resolveValue = resolve;
    rejectValue = reject;
  });
  return { promise, resolve: resolveValue, reject: rejectValue };
};

export const makeDapRequestBroker = (options: DapRequestBrokerOptions): DapRequestBroker => {
  const timeoutMs = options.timeoutMs ?? 30_000;
  const maxPendingRequests = options.maxPendingRequests ?? DAP_DEFAULT_PENDING_REQUESTS;
  const scheduleTimeout = options.scheduleTimeout ?? defaultScheduleTimeout;
  if (
    !Number.isInteger(timeoutMs) ||
    timeoutMs < 1 ||
    !Number.isInteger(maxPendingRequests) ||
    maxPendingRequests < 1 ||
    maxPendingRequests > DAP_MAX_SEQUENCE
  ) {
    throw dapTransportFailure("request", "DAP request broker limits are invalid.", false);
  }

  let nextSequence = 1;
  const pending = new Map<number, Pending>();

  const settle = (sequence: number, complete: (entry: Pending) => void): boolean => {
    const entry = pending.get(sequence);
    if (entry === undefined) return false;
    pending.delete(sequence);
    entry.cancelTimeout();
    entry.removeAbortListener();
    complete(entry);
    return true;
  };

  const reserveSequence = (): number => {
    let candidate = nextSequence;
    for (let index = 0; index <= pending.size; index += 1) {
      if (!pending.has(candidate)) {
        nextSequence = candidate === DAP_MAX_SEQUENCE ? 1 : candidate + 1;
        return candidate;
      }
      candidate = candidate === DAP_MAX_SEQUENCE ? 1 : candidate + 1;
    }
    throw dapTransportFailure(
      "request",
      "DAP sequence space is exhausted by pending requests.",
      true,
    );
  };

  const request = (
    command: string,
    argumentsValue?: DapJson,
    requestOptions: DapRequestOptions = {},
  ): DapPendingRequest => {
    if (pending.size >= maxPendingRequests) {
      throw dapTransportFailure("request", "DAP pending request limit was reached.", true);
    }
    if (requestOptions.signal?.aborted === true) {
      throw dapTransportFailure(
        "cancel",
        `DAP request ${command} was cancelled before send.`,
        true,
      );
    }

    const sequence = reserveSequence();
    const outboundResult = decodeDapRequestExit({
      seq: sequence,
      type: "request",
      command,
      ...(argumentsValue === undefined ? {} : { arguments: argumentsValue }),
    });
    if (Exit.isFailure(outboundResult)) {
      throw dapTransportFailure("request", "DAP request command or arguments are invalid.", false);
    }
    const outbound = outboundResult.value;
    const deferred = makePromiseResolvers<DapResponse>();

    const cancel = (detail = `DAP request ${command} was cancelled.`): boolean =>
      settle(sequence, (entry) => entry.reject(dapTransportFailure("cancel", detail, true)));
    const onAbort = (): void => {
      cancel();
    };
    const removeAbortListener =
      requestOptions.signal === undefined
        ? () => undefined
        : () => requestOptions.signal?.removeEventListener("abort", onAbort);
    requestOptions.signal?.addEventListener("abort", onAbort, { once: true });

    const cancelTimeout = scheduleTimeout(timeoutMs, () => {
      settle(sequence, (entry) =>
        entry.reject(
          dapTransportFailure(
            "response",
            `DAP request ${command} timed out after ${timeoutMs} ms.`,
            true,
          ),
        ),
      );
    });
    pending.set(sequence, {
      command,
      resolve: deferred.resolve,
      reject: deferred.reject,
      cancelTimeout,
      removeAbortListener,
    });

    try {
      options.onSend(outbound);
    } catch (cause) {
      const detail = cause instanceof Error ? cause.message : String(cause);
      settle(sequence, (entry) =>
        entry.reject(
          dapTransportFailure("request", `DAP request could not be sent: ${detail}`, true),
        ),
      );
    }
    return { request: outbound, response: deferred.promise, cancel };
  };

  return {
    request,
    accept: (response) =>
      settle(response.request_seq, (entry) => {
        if (entry.command !== response.command) {
          entry.reject(
            dapTransportFailure(
              "response",
              "DAP response command does not match its request.",
              false,
            ),
          );
          return;
        }
        entry.resolve(response);
      }),
    failAll: (detail) => {
      for (const sequence of pending.keys()) {
        settle(sequence, (entry) => entry.reject(dapTransportFailure("teardown", detail, true)));
      }
    },
    pendingCount: () => pending.size,
  };
};
