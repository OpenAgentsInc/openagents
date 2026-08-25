/**
 * The backends `openagents coder` can send a turn to.
 *
 * The server owns the real list and publishes it at `GET /api/v1/models`, and a
 * session that can reach the server reads it from there — see
 * `fetchServedCatalog` below. The list in this file is the fallback for a
 * session that cannot: `--offline`, or no stored credential.
 *
 * A hardcoded copy is a copy that drifts. This one did: it named
 * `gemini-3.7-flash`, no deployment had ever served a model by that id, and a
 * session that took the name from here opened its thread against a catalog that
 * refused it. The published catalog also says which models are *available* —
 * served here, credential configured — which a static list cannot say at all,
 * and which is the difference between a thread that answers and a 422.
 *
 * Choosing between them is not the client's call today. A coder session runs on
 * a thread, and the inference proxy takes the model from that thread's grant,
 * so this list is what the flag validates against and what the status line
 * names, nothing more.
 *
 * The `id` is what `POST /api/v1/threads` takes as `model`. The `label` is what
 * a person reads in the status bar, where the whole line is competing for a
 * narrow terminal; a model the server serves and this file has never heard of
 * is labelled with its own id.
 */

import { API_VERSION_PATH } from "./constants.js";

export interface CoderBackend {
  /** The value the chat API takes as `model`. Matches the server's enum. */
  readonly id: string;
  /** The short name the status line shows. */
  readonly label: string;
}

// Labels are Coder tiers, never vendor names: the invariant is that a vendor
// model name does not render (INVARIANTS.md "Coder Model Naming"). The ids
// stay vendor ids — they are what the API takes, not what a reader sees.
export const CODER_BACKENDS: readonly CoderBackend[] = [
  { id: "gemini-3.7-flash", label: "Coder Flash" },
  { id: "ox-alpha", label: "Coder" },
  { id: "gpt-5.6-luna", label: "Coder Pro" },
];

/**
 * The backend a coder session leads with when nobody names one.
 *
 * A *preference*, not an answer: `chooseBackend` falls to the server's own
 * default where a deployment does not serve it. A session that names nothing
 * at all does not use this — it opens unpinned, as Coder Auto.
 */
export const DEFAULT_CODER_BACKEND = "gemini-3.7-flash";

/** Every id, for a flag's error message and its accepted values. */
export const backendIds = (): readonly string[] => CODER_BACKENDS.map((backend) => backend.id);

/** One model as the server publishes it at `GET /api/v1/models`. */
export interface ServedModel {
  readonly id: string;
  /** Served here *and* its provider credential configured. */
  readonly available: boolean;
  /** The model the server itself falls back to. */
  readonly isDefault: boolean;
}

/**
 * What this deployment serves, read from the server rather than assumed.
 *
 * `undefined` means the question could not be asked — an older server without
 * the route, an unreachable one, a token that cannot read it. That is not the
 * same as "serves nothing", so the caller falls back to the static list rather
 * than refusing to start.
 */
export const fetchServedCatalog = async (
  api: { readonly origin: string; readonly token: string },
  signal?: AbortSignal,
): Promise<readonly ServedModel[] | undefined> => {
  try {
    const response = await fetch(new URL(`${API_VERSION_PATH}/models`, api.origin), {
      headers: { authorization: `Bearer ${api.token}`, accept: "application/json" },
      signal: signal ?? AbortSignal.timeout(5_000),
    });
    if (!response.ok) return undefined;

    const body = (await response.json()) as {
      readonly models?: readonly {
        readonly id?: unknown;
        readonly availability?: unknown;
        readonly default?: unknown;
      }[];
    };
    if (!Array.isArray(body.models)) return undefined;

    const served = body.models.flatMap((model) =>
      typeof model.id === "string" && model.id.length > 0
        ? [
            {
              id: model.id,
              // Anything other than the word `available` is treated as not
              // available: a vocabulary this client has not seen is a reason to
              // pick a different model, not to assume the new word is benign.
              available: model.availability === "available",
              isDefault: model.default === true,
            },
          ]
        : [],
    );
    return served.length === 0 ? undefined : served;
  } catch {
    return undefined;
  }
};

/**
 * The backend to open a thread on, given what the server actually serves.
 *
 * The preference wins where it is served and available. Otherwise the server's
 * own default, then whatever else is available — because a session that can run
 * on something should run, and a reader who wanted the other model says so with
 * `--model` and gets told plainly when it cannot be had.
 *
 * `undefined` means the catalog is real and nothing in it can answer. That is a
 * server with no provider credential configured, and it is worth saying rather
 * than opening a thread that will fail at its first turn.
 */
export const chooseBackend = (
  served: readonly ServedModel[],
  preferred: string = DEFAULT_CODER_BACKEND,
): ServedModel | undefined =>
  served.find((model) => model.id === preferred && model.available) ??
  served.find((model) => model.isDefault && model.available) ??
  served.find((model) => model.available);

/**
 * Why this named model cannot be opened, or `undefined` if it can.
 *
 * Refusing here turns the server's `422 Validation Failed` — which names no
 * model and suggests no alternative — into a sentence that says which model,
 * why, and what this deployment does serve.
 */
export const refuseBackend = (
  served: readonly ServedModel[],
  named: string,
): string | undefined => {
  const match = served.find((model) => model.id === named);
  const usable = served.filter((model) => model.available).map((model) => model.id);
  const alternatives =
    usable.length === 0
      ? "This server has no model with a configured credential."
      : `This server serves ${usable.join(", ")}.`;

  if (match === undefined) return `No model called '${named}' is served here. ${alternatives}`;
  if (!match.available) {
    return `'${named}' is served here but its provider credential is not configured. ${alternatives}`;
  }
  return undefined;
};

/** The status-line name for a model id, which may be one the static list lacks. */
export const backendLabel = (id: string): string =>
  CODER_BACKENDS.find((backend) => backend.id === id)?.label ?? id;
