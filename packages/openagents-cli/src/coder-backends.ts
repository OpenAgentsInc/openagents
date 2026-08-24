/**
 * The backends `openagents coder` can send a turn to.
 *
 * The server owns the real list and publishes it at `GET /api/v3` under
 * `extensions["chat.openagents"].parameters.model`. This is the client's copy,
 * kept as data for the same reason the server keeps one: a backend the status
 * line shows and the `--model` flag accepts has to be one list, or the two
 * drift and the CLI offers something the server refuses.
 *
 * Choosing between them is not the client's call today. A coder session runs on
 * a thread, and the inference proxy takes the model from that thread's grant,
 * so this list is what the flag validates against and what the status line
 * names, nothing more.
 *
 * Adding a backend is one entry here and one entry on the server. Nothing else
 * in this package names a backend.
 *
 * The `id` is what `POST /api/v3/chat/turns` takes as `model`, so it must match
 * the server's published enum exactly. The `label` is what a person reads in
 * the status bar, where the whole line is competing for a narrow terminal.
 */

export interface CoderBackend {
  /** The value the chat API takes as `model`. Matches the server's enum. */
  readonly id: string;
  /** The short name the status line shows. */
  readonly label: string;
}

export const CODER_BACKENDS: readonly CoderBackend[] = [
  { id: "ox-alpha", label: "Ox Alpha" },
  { id: "gemini-3.7-flash", label: "Gemini 3.7 Flash" },
];

/**
 * What a coder session opens on when nobody names a backend.
 *
 * Deliberately not the server's own default, which is the catalog's first entry
 * and serves every caller of the chat API. A coder turn is a long one with tools
 * in it, and this build leads with the fast model for that; a reader who wants
 * the other says so with `--model`.
 *
 * Named rather than taken from the list's order, because the order here mirrors
 * the server's published enum and a test holds the two together. Expressing a
 * preference by reordering would have broken that agreement to say something the
 * list was never saying.
 */
export const DEFAULT_CODER_BACKEND = "gemini-3.7-flash";

export const defaultBackendId = (): string =>
  CODER_BACKENDS.some((backend) => backend.id === DEFAULT_CODER_BACKEND)
    ? DEFAULT_CODER_BACKEND
    : (CODER_BACKENDS[0]?.id ?? "");

/** Every id, for a flag's error message and its accepted values. */
export const backendIds = (): readonly string[] => CODER_BACKENDS.map((backend) => backend.id);
