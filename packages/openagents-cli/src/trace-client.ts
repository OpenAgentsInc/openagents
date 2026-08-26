/**
 * The trace ingest client: `POST /api/v1/traces`.
 *
 * This is the server half `trace upload` used to refuse for. The route exists
 * -- it takes an ATIF v1 document as the whole request body, stores it against
 * the calling account, and answers 201 for a document it did not hold or 200
 * for one it already has under the same digest.
 *
 * Three things this client will not do:
 *
 * - It does not invent a visibility. The server's vocabulary is the forge
 *   transparency ladder, and `dark` -- nothing public -- is the default there
 *   and here. A caller who wants more has to say which rung.
 * - It does not call an existing trace a new one. The status is the only thing
 *   that distinguishes them, so it is read rather than discarded, and a 200 is
 *   reported as "already held" rather than as an upload that wrote something.
 * - It does not report a stored trace by a link. The response carries a `url`
 *   pointing at `GET /api/v1/traces/:id`, and that route does not exist, so
 *   printing it would hand someone a 404 dressed as a receipt. The id and the
 *   digest are real, and they are what get reported.
 */

import { Effect, Layer } from "effect";
import * as Context from "effect/Context";

import { ApiTransport } from "./api-transport.js";
import { API_VERSION_PATH } from "./constants.js";
import { ApiError, type CliError } from "./errors.js";
import type { AuthenticatedApi } from "./repository-client.js";
import { asNumber, asRecord, asText, trackerErrorDetails } from "./tracker-request.js";

/**
 * The transparency ladder the server stores a trace under.
 *
 * `dark` is nothing public, `pulse` is metadata only, `ledger` is content and
 * metadata, and `glass` is full access. The database enforces this exact set
 * with a CHECK constraint, so a name outside it is refused here with the list
 * rather than sent on to earn a 422 that does not say what the choices were.
 */
export const TRACE_VISIBILITIES = ["dark", "pulse", "ledger", "glass"] as const;

export type TraceVisibility = (typeof TRACE_VISIBILITIES)[number];

/** What the server defaults to when no visibility is named: nothing public. */
export const DEFAULT_TRACE_VISIBILITY: TraceVisibility = "dark";

/** The largest body the ingest route accepts, in bytes. */
export const MAXIMUM_TRACE_BYTES = 10_485_760;

export const isTraceVisibility = (value: string): value is TraceVisibility =>
  (TRACE_VISIBILITIES as ReadonlyArray<string>).includes(value);

export interface TraceUploadInput extends AuthenticatedApi {
  /** The ATIF document, parsed. It is sent as the whole request body. */
  readonly document: unknown;
  readonly visibility: TraceVisibility;
  /** The forge attempt this trajectory belongs to, when there is one. */
  readonly assignmentId?: string;
}

/** What the server said it stored. Every field is the server's, not a guess. */
export interface TraceUploadResult {
  readonly id: string;
  readonly digest: string;
  readonly byte_size: number;
  readonly visibility: string;
  readonly inserted_at: string;
  /** 201: the server did not hold this document. 200: it already did. */
  readonly created: boolean;
}

interface TraceClientInterface {
  readonly upload: (input: TraceUploadInput) => Effect.Effect<TraceUploadResult, CliError>;
}

export class TraceClient extends Context.Service<TraceClient, TraceClientInterface>()(
  "@openagentsinc/cli/TraceClient",
) {}

export const traceClientLayer = Layer.effect(
  TraceClient,
  Effect.gen(function* () {
    const transport = yield* ApiTransport;

    return TraceClient.of({
      upload: Effect.fn("TraceClient.upload")(function* (input: TraceUploadInput) {
        // Visibility and the attempt binding are query parameters; the body is
        // the document itself, with nothing wrapped around it.
        const parameters = new URLSearchParams({ visibility: input.visibility });
        if (input.assignmentId !== undefined) {
          parameters.set("assignment_id", input.assignmentId);
        }

        const response = yield* transport.request({
          origin: input.origin,
          token: input.token,
          method: "POST",
          path: `${API_VERSION_PATH}/traces?${parameters.toString()}`,
          body: input.document,
        });

        if (response.status !== 200 && response.status !== 201) {
          const details = trackerErrorDetails(response.body, response.status);
          return yield* new ApiError({
            operation: "upload a trace",
            status: response.status,
            ...(details.code === undefined ? {} : { code: details.code }),
            message: details.message,
            ...(response.requestId === undefined && details.requestId === undefined
              ? {}
              : { requestId: response.requestId ?? details.requestId }),
          });
        }

        const stored = asRecord(response.body);
        const id = asText(stored["id"]);
        const digest = asText(stored["digest"]);
        if (id === undefined || digest === undefined) {
          // An accepted status with no id is not a stored trace. Reporting one
          // anyway is how a caller comes to believe a trace exists server-side
          // that nothing can ever be found by.
          return yield* new ApiError({
            operation: "upload a trace",
            status: response.status,
            message:
              "The server accepted the trace but did not say what it stored: the response carries no id or digest.",
            ...(response.requestId === undefined ? {} : { requestId: response.requestId }),
          });
        }

        return {
          id,
          digest,
          byte_size: asNumber(stored["byte_size"]) ?? 0,
          visibility: asText(stored["visibility"]) ?? input.visibility,
          inserted_at: asText(stored["inserted_at"]) ?? "",
          created: response.status === 201,
        } satisfies TraceUploadResult;
      }),
    });
  }),
);
