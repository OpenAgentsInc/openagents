import type { MaterialTarget, MaterialBundle } from "./creator-material.ts";
import { awaitAcceptedCommand } from "./accepted-command.ts";
import type { AuthorAction } from "./authoring.ts";
import { EditorService } from "./service";
import { editorAtoms } from "./state";
import type { EngineStatus, CameraMode } from "./protocol";
import { WORLD_PROFILE } from "./world-profile";
import {
  AuthoringPermit,
  type HostGrant as Grant,
  type Project,
} from "./authoring-permit";

type View = {
  eye: [number, number, number];
  target: [number, number, number];
  mode: CameraMode;
};
type Move = {
  type: "move";
  target: string;
  position: [number, number, number];
};
type Request = {
  request_id: string;
  project: Project;
  action: {
    type: string;
    source?: string;
    grant?: Grant;
    operations?: Move[];
    checkpoint?: View | null;
    enabled?: boolean;
    target?: string | MaterialTarget;
    material?: MaterialBundle;
    release?: {
      id: string;
      manifest: {
        runtime: { engine_release: string; sdk_url: string };
        content: {
          document: import("./project-document.ts").ProjectDocument;
          materials: MaterialBundle[];
        };
      };
    };
    mode?: "shaded" | "mesh";
    /** Talk: the character id; absent means the nearest one. */
    npc?: string | null;
    intent?: "reply" | "hint" | "emote";
  };
};
type Outcome = {
  state: string;
  message: string;
  status: unknown;
  detail?: unknown;
};
interface Endpoint {
  reply(id: string, outcome: string): void;
  status(status: string): boolean;
  lookup(id: string): string;
  revoke(message: string): void;
  close(): void;
}
interface Sdk {
  default(): Promise<unknown>;
  protocol_version(): number;
  action_sha256(input: string): string;
  EngineEndpoint: new (
    host: string,
    release: string,
    abi: string,
    format: number,
    capabilities: string,
    callback: (request: string) => void,
  ) => Endpoint;
}
declare global {
  interface Window {
    openAgentsHostStop?: () => void;
  }
}

/** The checked host brokers narrow permissions; the preview receives no account token. */
export async function installOpenAgentsHost(
  service: EditorService,
): Promise<void> {
  // A framed page talks to its parent; a top-level page only has a host when a
  // native shell injected `window.aethyrHost` (ADR 0010). The SDK's endpoint
  // picks the matching channel.
  const native =
    typeof (window as { aethyrHost?: { postMessage?: unknown } }).aethyrHost
      ?.postMessage === "function";
  if (window.openAgentsHostStop || (window.parent === window && !native)) {
    return;
  }
  const host = import.meta.env.VITE_OPENAGENTS_HOST_ORIGIN as
    string | undefined;
  const sdkUrl = import.meta.env.VITE_OPENAGENTS_PROTOCOL_URL as
    string | undefined;
  const release = import.meta.env.VITE_OPENAGENTS_ENGINE_RELEASE as
    string | undefined;
  if (!host || !sdkUrl || !release) return;
  for (const value of [host, sdkUrl]) {
    const url = new URL(value);
    if (
      url.protocol !== "https:" &&
      !(
        url.protocol === "http:" &&
        ["127.0.0.1", "localhost", "[::1]"].includes(url.hostname)
      )
    )
      throw new Error("Use HTTPS for the host protocol.");
    if (url.username || url.password || url.hash || url.search)
      throw new Error("Host configuration cannot contain credentials.");
  }
  if (new URL(host).origin !== host || host === location.origin)
    throw new Error("The OpenAgents host needs its own origin.");
  // The deployment operator pins this public SDK URL. It never comes from a message.
  const sdk = (await import(/* @vite-ignore */ sdkUrl)) as Sdk;
  await sdk.default();
  if (sdk.protocol_version?.() !== 8)
    throw new Error("This world needs the matching version 8 host SDK.");
  const player = location.pathname.endsWith("/player.html");
  let loadedPlayable: string | undefined;
  let opened = false,
    observing = false,
    closed = false;
  const permit = new AuthoringPermit((action) =>
    sdk.action_sha256(JSON.stringify(action)),
  );
  const snapshot = () => {
    const state = service.registry.get(editorAtoms.status);
    return {
      project: {
        id: WORLD_PROFILE.id,
        branch: "main",
        revision: state.revision,
      },
      generation: service.sessionId,
      frame: state.frame,
      save: state.saved ? "device" : "unsaved",
      view: state.mesh ? "mesh" : "shaded",
      // The external SDK retains its camera-only checkpoint contract.
      checkpoint: state.view
        ? {
            eye: state.view.eye,
            target: state.view.target,
            mode: state.view.mode,
          }
        : null,
      ...(!player && state.authoring ? { authoring: state.authoring } : {}),
      ...(state.npc ? { npc: state.npc } : {}),
      ...(state.selection
        ? {
            selection: {
              id: state.selection.id,
              asset: state.selection.kind,
              position: state.selection.position,
            },
          }
        : {}),
    };
  };
  const ready = async (checkpoint?: View | null): Promise<EngineStatus> => {
    const deadline = performance.now() + 110000;
    let restored = false,
      restoringId: string | undefined;
    while (!closed && performance.now() < deadline) {
      const state = service.registry.get(editorAtoms.status);
      if (state.phase === "error") throw new Error("The world could not open.");
      if (state.phase === "ready" && state.view) {
        if (!checkpoint) return state;
        if (!restored) {
          restoringId = service.cameraPose(
            checkpoint.eye,
            checkpoint.target,
            undefined,
            checkpoint.mode,
          );
          restored = true;
        } else if (
          restoringId &&
          service.receipts.get(restoringId)?.ok === false
        ) {
          // Keep the validated initial place when the saved checkpoint is blocked.
          return state;
        } else if (
          state.view.mode === checkpoint.mode &&
          [...state.view.eye, ...state.view.target].every(
            (value, i) =>
              Math.abs(value - [...checkpoint.eye, ...checkpoint.target][i]) <
              0.03,
          )
        )
          return state;
      }
      await new Promise((resolve) => setTimeout(resolve, 50));
    }
    throw new Error("The world took too long to open.");
  };
  const completed = (id: string, draft?: string) =>
    awaitAcceptedCommand({
      receipt: () => service.receipts.get(id),
      status: () => service.registry.get(editorAtoms.status),
      closed: () => closed,
      draft,
    });
  const stop = () => {
    if (closed) return;
    closed = true;
    opened = false;
    observing = false;
    clearInterval(timer);
    service.close();
    document.querySelector<HTMLIFrameElement>("#preview-frame")?.remove();
    endpoint.close();
  };
  const endpoint = new sdk.EngineEndpoint(
    host,
    release,
    "aethyrforge.browser.v1",
    1,
    JSON.stringify(
      ["haunted-house", "personal-planet"].includes(WORLD_PROFILE.id) && !player
        ? ["inspect", "observe", "propose", "draft", "compile", "undo", "redo", "capture"]
        : ["inspect", "observe"],
    ),
    (raw) => {
      const request = JSON.parse(raw) as Request; // Rust has already checked the closed schema and limits.
      void (async () => {
        let result: Outcome;
        try {
          if (
            request.project.id !== WORLD_PROFILE.id ||
            request.project.branch !== "main"
          )
            throw new Error("This world is not available.");
          if (
            player &&
            ![
              "open",
              "load_playable",
              "inspect",
              "observe",
              "talk",
              "view",
              "capture",
              "lookup",
              "close",
            ].includes(request.action.type)
          )
            throw new Error("This player has no editing access.");
          switch (request.action.type) {
            case "load_playable": {
              const playable = request.action.release;
              if (
                !player ||
                !playable ||
                playable.manifest.runtime.engine_release !== release ||
                playable.manifest.runtime.sdk_url !== sdkUrl
              )
                throw new Error(
                  "This playable version needs its pinned runtime.",
                );
              if (loadedPlayable && loadedPlayable !== playable.id)
                throw new Error("This player stays on its opened version.");
              const { document, materials } = playable.manifest.content;
              await completed(
                service.send(
                  {
                    type: "loadPlayable",
                    id: playable.id,
                    base: request.project.revision,
                    action: {
                      type: "restore_material_project",
                      document,
                      materials,
                      checkpoint: null,
                    },
                  },
                  true,
                ),
              );
              loadedPlayable = playable.id;
              permit.revoke();
              result = {
                state: "applied",
                message: "Playable version opened.",
                status: snapshot(),
              };
              break;
            }
            case "open": {
              await ready(request.action.checkpoint);
              if (
                request.project.revision !==
                service.registry.get(editorAtoms.status).revision
              )
                throw new Error(
                  "The saved world has changed. Open its current revision.",
                );
              opened = true;
              result = {
                state: "applied",
                message: "World opened.",
                status: snapshot(),
              };
              break;
            }
            case "select": {
              const id = service.command({
                kind: "entity.select",
                id: request.action.target! as string,
              });
              await completed(id);
              result = {
                state: "applied",
                message: "Object selected.",
                status: snapshot(),
              };
              break;
            }
            case "authorize": {
              const g = request.action.grant!;
              if (
                !["haunted-house", "personal-planet"].includes(WORLD_PROFILE.id)
              )
                throw new Error("This world does not support edits.");
              permit.authorize(g, request.project, release);
              result = {
                state: "applied",
                message: "Change confirmed.",
                status: snapshot(),
              };
              break;
            }
            case "revoke_authorization":
              permit.revoke();
              if (
                !player &&
                ["haunted-house", "personal-planet"].includes(WORLD_PROFILE.id)
              )
                await completed(
                  service.send({ type: "cancelAuthoring" }, true),
                );
              result = {
                state: "applied",
                message: "Editing access ended.",
                status: snapshot(),
              };
              break;
            case "propose":
            case "restore_project":
            case "restore_material_project":
            case "apply_material":
            case "draft":
            case "compile":
            case "undo":
            case "redo": {
              await permit.consume(request.action, request.project);
              await completed(
                service.author(
                  request.action as AuthorAction,
                  request.project.revision,
                  request.request_id,
                ),
                request.action.type === "draft"
                  ? request.action.source
                  : undefined,
              );
              result = {
                state: "applied",
                message:
                  request.action.type === "draft"
                    ? "Source draft updated."
                    : "Change applied.",
                status: snapshot(),
                ...(request.action.type === "apply_material"
                  ? {
                      detail: {
                        type: "material",
                        material: (
                          await service.materialView(
                            {
                              mode: "inspect",
                              target: request.action.material!.document.target,
                            },
                            service.registry.get(editorAtoms.status).revision,
                          )
                        ).material,
                      },
                    }
                  : {}),
              };
              break;
            }
            case "talk": {
              const npc = request.action.npc ?? undefined;
              await completed(
                service.send(
                  {
                    type: "npc",
                    intent: request.action.intent ?? "reply",
                    ...(npc ? { npc } : {}),
                  },
                  true,
                ),
              );
              const said = service.registry.get(editorAtoms.status).npc?.lines;
              const line = said?.[said.length - 1];
              result = {
                state: "applied",
                message: line ? `${line.name}: ${line.text}` : "Nobody answered.",
                status: snapshot(),
              };
              break;
            }
            case "view": {
              await completed(
                service.send(
                  { type: "mesh", enabled: request.action.mode === "mesh" },
                  true,
                ),
              );
              result = {
                state: "applied",
                message:
                  request.action.mode === "mesh"
                    ? "Mesh overlay on."
                    : "Shaded view.",
                status: snapshot(),
              };
              break;
            }
            case "capture": {
              const image = await createImageBitmap(
                await service.captureBlob(),
              );
              let blob: Blob | undefined,
                width = 320,
                height = Math.max(
                  1,
                  Math.round((image.height / image.width) * 320),
                );
              if (height > 320) {
                width = Math.max(1, Math.round((width * 320) / height));
                height = 320;
              }
              try {
                for (let attempt = 0; attempt < 4; attempt++) {
                  const canvas = new OffscreenCanvas(width, height);
                  canvas
                    .getContext("2d")!
                    .drawImage(image, 0, 0, width, height);
                  blob = await canvas.convertToBlob({ type: "image/png" });
                  if (blob.size <= 24576) break;
                  width = Math.max(1, Math.floor(width * 0.7));
                  height = Math.max(1, Math.floor(height * 0.7));
                }
              } finally {
                image.close();
              }
              if (!blob || blob.size > 24576)
                throw new Error(
                  "The preview is too large to capture. Try a simpler view.",
                );
              const bytes = new Uint8Array(await blob.arrayBuffer());
              const sha256 = [
                ...new Uint8Array(await crypto.subtle.digest("SHA-256", bytes)),
              ]
                .map((v) => v.toString(16).padStart(2, "0"))
                .join("");
              result = {
                state: "applied",
                message: "Preview captured.",
                status: null,
                detail: {
                  type: "capture_preview",
                  png: btoa(String.fromCharCode(...bytes)),
                  sha256,
                  width,
                  height,
                },
              };
              break;
            }
            case "inspect_material": {
              const viewed = await service.materialView(
                {
                  mode: "inspect",
                  target: request.action.target as MaterialTarget,
                },
                request.project.revision,
              );
              result = {
                state: "applied",
                message: "Face material opened.",
                status: snapshot(),
                detail: { type: "material", material: viewed.material },
              };
              break;
            }
            case "preview_material": {
              const viewed = await service.materialView(
                { mode: "preview", material: request.action.material! },
                request.project.revision,
              );
              result = {
                state: "applied",
                message: "Face preview ready. Apply to keep this change.",
                status: snapshot(),
                detail: {
                  type: "material_preview",
                  action_sha256: sdk.action_sha256(
                    JSON.stringify(request.action),
                  ),
                  frame: viewed.frame,
                },
              };
              break;
            }
            case "clear_material_preview":
              await service.materialView(
                { mode: "clear" },
                request.project.revision,
              );
              result = {
                state: "applied",
                message: "Applied face material restored.",
                status: snapshot(),
              };
              break;
            case "inspect":
              result = {
                state: "applied",
                message: "World status.",
                status: snapshot(),
              };
              break;
            case "export_project":
              result = {
                state: "applied",
                message: "Editable project exported.",
                status: snapshot(),
                detail: {
                  type: "project_document",
                  document: await service.projectDocument(),
                },
              };
              break;
            case "observe":
              observing = request.action.enabled === true;
              result = {
                state: "applied",
                message: observing ? "Updates started." : "Updates stopped.",
                status: snapshot(),
              };
              break;
            case "lookup":
              result = (JSON.parse(
                endpoint.lookup(request.action.target! as string),
              ) as Outcome | null) ?? {
                state: "unknown",
                message: "No result is available.",
                status: null,
              };
              break;
            case "cancel":
              result = {
                state: "unknown",
                message:
                  "No running change was cancelled. Check the original result.",
                status: null,
              };
              break;
            case "close":
              result = {
                state: "applied",
                message: "World closed.",
                status: null,
              };
              break;
            default:
              result = {
                state: "unsupported",
                message: "This world does not support that action yet.",
                status: null,
              };
          }
        } catch (error) {
          result = {
            state: "rejected",
            message: (error instanceof Error
              ? error.message
              : "The change could not be applied."
            ).slice(0, 2048),
            status: opened ? snapshot() : null,
          };
        }
        if (closed) return;
        try {
          endpoint.reply(request.request_id, JSON.stringify(result));
          if (request.action.type === "close") stop();
        } catch {
          stop();
        }
      })();
    },
  );
  const timer = setInterval(() => {
    if (opened && observing && !closed) {
      try {
        if (service.registry.get(editorAtoms.status).phase === "error") {
          endpoint.revoke(
            "The world could not stay open. Choose Retry to try again.",
          );
          stop();
          return;
        }
        endpoint.status(JSON.stringify(snapshot()));
      } catch {
        stop();
      }
    }
  }, 100);
  // The parent owns account and editor tools; published worlds keep view controls.
  if (import.meta.env.VITE_PUBLIC_DEMO !== "1" && !player)
    document.querySelector<HTMLElement>("#ui-root")?.setAttribute("hidden", "");
  window.openAgentsHostStop = stop;
  window.addEventListener("pagehide", stop, { once: true });
}
