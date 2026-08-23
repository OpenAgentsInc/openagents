/**
 * The backends `openagents coder` can send a turn to.
 *
 * The server owns the real list and publishes it at `GET /api/v3` under
 * `extensions["chat.openagents"].parameters.model`. This is the client's copy,
 * kept as data for the same reason the server keeps one: a backend the status
 * line offers, the `--model` flag accepts, and Tab cycles through has to be one
 * list, or the three drift and the CLI offers something the server refuses.
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

/** The backend a turn uses when nothing named one. */
export const defaultBackend = (): CoderBackend => CODER_BACKENDS[0] as CoderBackend;

/** The backend with this id, or `undefined` when nothing matches. */
export const findBackend = (id: string): CoderBackend | undefined =>
  CODER_BACKENDS.find((backend) => backend.id === id);

/**
 * The next backend after this one, wrapping at the end.
 *
 * Cycling rather than toggling is what makes a third backend data: Tab keeps
 * working without a second key or a menu.
 */
export const nextBackend = (current: CoderBackend): CoderBackend => {
  const index = CODER_BACKENDS.findIndex((backend) => backend.id === current.id);
  return CODER_BACKENDS[(index + 1) % CODER_BACKENDS.length] as CoderBackend;
};

/** Every id, for a flag's error message and its accepted values. */
export const backendIds = (): readonly string[] => CODER_BACKENDS.map((backend) => backend.id);
