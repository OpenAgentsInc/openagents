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

/** Every id, for a flag's error message and its accepted values. */
export const backendIds = (): readonly string[] => CODER_BACKENDS.map((backend) => backend.id);
