/** Read-only Aethyrforge universe profile over the public, checked host protocol. */
type Status = {
  project: { id: string; branch: string; revision: number };
  generation: string;
  frame: number;
  save: string;
  checkpoint: unknown;
  selection?: { id: string; asset: string; position: number[] };
};
type Request = {
  request_id: string;
  project: Status["project"];
  action: {
    type: string;
    checkpoint?: unknown;
    target?: string;
    enabled?: boolean;
  };
};
type Endpoint = {
  reply(id: string, outcome: string): void;
  status(status: string): boolean;
  close(): void;
};
const host = import.meta.env.VITE_OPENAGENTS_HOST_ORIGIN as string;
const release = import.meta.env.VITE_OPENAGENTS_ENGINE_RELEASE as string;
const sdkURL = import.meta.env.VITE_OPENAGENTS_PROTOCOL_URL as string;
const previewURL = new URL(
  "universe-preview.html",
  import.meta.env.VITE_PREVIEW_URL as string,
);
if (
  new URL(host).origin !== host ||
  new URL(sdkURL).origin !== location.origin ||
  previewURL.origin === location.origin ||
  host === location.origin
)
  throw Error("The planets need separate host and preview origins.");
const sdk = await import(/* @vite-ignore */ sdkURL);
await sdk.default();
if (sdk.protocol_version() !== 8)
  throw Error("The planets need the matching host SDK.");
const frame = document.querySelector<HTMLIFrameElement>("#universe-preview")!;
let endpoint: Endpoint,
  snapshot: Status | undefined,
  closed = false,
  observing = false,
  session = "";
let ready: ((value: Status) => void) | undefined,
  failed: ((error: Error) => void) | undefined;
const waiting = new Map<
  string,
  {
    resolve: (value: Status) => void;
    reject: (error: Error) => void;
    timer: number;
  }
>();
const control = (type: string, extra: object = {}) =>
  new Promise<Status>((resolve, reject) => {
    const request = crypto.randomUUID();
    const timer = window.setTimeout(() => {
      waiting.delete(request);
      reject(Error("The view took too long to change. Try again."));
    }, 5000);
    waiting.set(request, { resolve, reject, timer });
    frame.contentWindow!.postMessage(
      { session, request, type, ...extra },
      previewURL.origin,
    );
  });
const stop = () => {
  closed = true;
  observing = false;
  frame.src = "about:blank";
  for (const p of waiting.values()) {
    clearTimeout(p.timer);
    p.reject(Error("The map closed."));
  }
  waiting.clear();
};
window.addEventListener("message", (event) => {
  if (
    event.origin !== previewURL.origin ||
    event.source !== frame.contentWindow ||
    event.data?.session !== session
  )
    return;
  const value = event.data;
  if (value.type === "error") {
    failed?.(Error(value.message));
    return;
  }
  if (value.type === "reply") {
    const held = waiting.get(value.request);
    if (!held) return;
    clearTimeout(held.timer);
    waiting.delete(value.request);
    if (value.error) held.reject(Error(value.error));
    else held.resolve(value.status);
    return;
  }
  if (
    value.type !== "status" ||
    !value.status ||
    value.status.project?.id !== "universe" ||
    value.status.generation !== session
  )
    return;
  // The public SDK validates bounds, identities, and every status field before delivery.
  const text = JSON.stringify(value.status);
  if (text.length > 65536) return;
  if (snapshot && observing) {
    if (!endpoint.status(text)) return;
  }
  snapshot = value.status;
  ready?.(snapshot!);
  ready = undefined;
});
endpoint = new sdk.EngineEndpoint(
  host,
  release,
  "aethyrforge.browser.v1",
  1,
  JSON.stringify(["inspect", "observe"]),
  (text: string) => {
    const request = JSON.parse(text) as Request;
    void (async () => {
      const action = request.action;
      try {
        if (closed) throw Error("The map closed. Reload to return.");
        if (
          request.project.id !== "universe" ||
          request.project.branch !== "main" ||
          request.project.revision !== 0
        )
          throw Error("Choose this map again.");
        if (action.type === "open") {
          session = crypto.randomUUID();
          const opened = new Promise<Status>((resolve, reject) => {
            ready = resolve;
            failed = reject;
          });
          previewURL.hash = new URLSearchParams({
            session,
            editorOrigin: location.origin,
          }).toString();
          frame.src = previewURL.href;
          const timeout = window.setTimeout(
            () =>
              failed?.(
                Error(
                  "The planets took too long to open. Reload to try again.",
                ),
              ),
            20000,
          );
          try {
            snapshot = await opened;
            if (action.checkpoint)
              snapshot = await control("restore", {
                checkpoint: action.checkpoint,
              });
          } finally {
            clearTimeout(timeout);
            failed = undefined;
          }
        } else if (action.type === "inspect") {
        } else if (action.type === "select")
          snapshot = await control("select", { target: action.target });
        else if (action.type === "observe") observing = Boolean(action.enabled);
        else if (action.type === "close") {
          const last = snapshot;
          stop();
          endpoint.reply(
            request.request_id,
            JSON.stringify({
              state: "applied",
              message: "Map closed.",
              status: last,
            }),
          );
          return;
        } else {
          endpoint.reply(
            request.request_id,
            JSON.stringify({
              state: "unsupported",
              message: "This map is for exploring.",
              status: snapshot,
            }),
          );
          return;
        }
        endpoint.reply(
          request.request_id,
          JSON.stringify({
            state: "applied",
            message: action.type === "open" ? "Map opened." : "View updated.",
            status: snapshot,
          }),
        );
      } catch (error) {
        endpoint.reply(
          request.request_id,
          JSON.stringify({
            state: "rejected",
            message:
              error instanceof Error
                ? error.message
                : "The map could not open.",
            status: snapshot ?? null,
          }),
        );
      }
    })();
  },
) as Endpoint;
window.addEventListener(
  "pagehide",
  () => {
    stop();
    endpoint.close();
  },
  { once: true },
);
Object.assign(window, {
  aethyrforgeUniverse: { snapshot: () => snapshot, closed: () => closed },
});
