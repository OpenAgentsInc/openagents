/**
 * The `remember` tool: store one memory for the account.
 *
 * The write half of OpenAgentsInc/openagents#51. A memory goes to
 * `POST /api/v1/memories` in the openagents.com database, account-scoped —
 * not to a file on this machine. The local engram ledger under
 * `~/.openagents/memory` is frozen and nothing here reads or writes it.
 *
 * There is deliberately no recall tool, and no `get`. Recall runs server-side
 * inside `POST /api/v1/responses`: the server retrieves the account's relevant
 * memories against the incoming input and attaches them to the model context
 * before the provider call. The model does not go looking, and the CLI does no
 * retrieval at all. So the one genuine model action left is the one this tool
 * carries — writing down something the reader asked to have remembered.
 *
 * Explicit only, never inferred. This fires when the reader asks for something
 * to be remembered, not when a conversation happens to reveal a preference.
 * The description below says so to the model, because the description is the
 * only thing that governs when a tool is called.
 *
 * A refusal is returned as text rather than thrown. A model that is told the
 * memory could not be stored can say so; a model handed a silent success for a
 * write that never landed will tell the reader their preference is saved when
 * it is not. So every path out of this tool — no credential, a rejected token,
 * a full account, an unreachable server — names what happened.
 */

import type { CoderTool } from "./coder-tools.js";
import { TOOL_DESCRIPTION_SURFACE } from "./coder-surfaces.generated.js";
import { MEMORIES_PATH } from "./constants.js";
import { trackerErrorDetails } from "./tracker-request.js";

/** The subset of `fetch` this tool uses. An injection seam for tests. */
export type RememberTransport = (
  input: URL,
  init: {
    readonly method: string;
    readonly headers: Readonly<Record<string, string>>;
    readonly body: string;
  },
) => Promise<{ readonly status: number; json: () => Promise<unknown> }>;

export interface RememberOptions {
  readonly origin: string;
  /**
   * The account token.
   *
   * `undefined` where the session holds no credential — an offline session, or
   * one started before `openagents auth login`. The tool is still declared in
   * that case and refuses honestly when called, rather than disappearing and
   * leaving the model to answer that it cannot remember things at all.
   */
  readonly token?: string | undefined;
  /**
   * The thread this session records to, carried onto every memory it writes.
   *
   * `undefined` on a lane that keeps no server record, which is honest: a
   * source reference naming no readable thread would be worse than none.
   */
  readonly sourceRef?: string | undefined;
  /** Injection seam for tests. Defaults to the global `fetch`. */
  readonly fetch?: RememberTransport | undefined;
}

/** Reads a JSON object, or an empty one when the value is not an object. */
const record = (value: unknown): Record<string, unknown> =>
  value !== null && typeof value === "object" && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : {};

export function rememberTool(options: RememberOptions): CoderTool {
  const send: RememberTransport =
    options.fetch ?? (globalThis.fetch.bind(globalThis) as unknown as RememberTransport);

  return {
    name: "remember",
    description: TOOL_DESCRIPTION_SURFACE["node.remember"],
    parameters: {
      type: "object",
      properties: {
        body: {
          type: "string",
          description:
            "What to remember, as one self-contained sentence that will still make sense in a " +
            'later session with none of this conversation around it. For example: "Uses pnpm, ' +
            'not npm, in every repository."',
        },
        supersedes: {
          type: "string",
          description:
            "Optional. The id of an existing memory this one corrects and replaces. Use it " +
            "instead of writing a second, contradictory memory.",
        },
      },
      required: ["body"],
      additionalProperties: false,
    },
    run: async (args) => {
      const body = typeof args["body"] === "string" ? args["body"].trim() : "";
      if (body.length === 0) {
        return "Nothing was stored: `body` is required and must say what to remember.";
      }
      const supersedes = typeof args["supersedes"] === "string" ? args["supersedes"] : undefined;

      if (options.token === undefined) {
        return (
          "Refusal: nothing was stored. This session holds no OpenAgents credential, so it " +
          "cannot reach the account's memory. Run `openagents auth login` and ask again."
        );
      }

      const sourceRef = options.sourceRef;

      let response: { readonly status: number; json: () => Promise<unknown> };
      try {
        response = await send(new URL(MEMORIES_PATH, options.origin), {
          method: "POST",
          headers: {
            authorization: `Bearer ${options.token}`,
            "content-type": "application/json",
            accept: "application/json",
          },
          body: JSON.stringify({
            body,
            bucket: "user",
            ...(sourceRef === undefined ? {} : { source_ref: sourceRef }),
            ...(supersedes === undefined ? {} : { supersedes }),
          }),
        });
      } catch (cause) {
        return (
          `Refusal: nothing was stored. The API at ${options.origin} could not be reached ` +
          `(${String(cause)}).`
        );
      }

      const envelope = await response.json().catch(() => ({}));

      if (response.status === 201) {
        const memory = record(record(envelope)["memory"]);
        const id = typeof memory["id"] === "string" ? memory["id"] : "";
        return supersedes === undefined
          ? `Stored. Memory ${id} now holds: ${body}`
          : `Stored. Memory ${id} now holds: ${body} (it supersedes ${supersedes}.)`;
      }

      // The server's own code and sentence carry through. A full account is
      // told which limit it met and what to do about it, and a rejected field
      // is named, so the model can act rather than retry the same write.
      const details = trackerErrorDetails(envelope, response.status);
      if (response.status === 401 || response.status === 403) {
        return (
          "Refusal: nothing was stored. This session's credential cannot write the account's " +
          "memory. Run `openagents auth login` to sign in again."
        );
      }
      return `Refusal: nothing was stored. ${details.message}`;
    },
  };
}
