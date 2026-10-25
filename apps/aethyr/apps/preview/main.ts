import {
  PREVIEW_PROTOCOL,
  previewEventTransfers,
  CHARACTER_ORBIT_RANGE,
  DEFAULT_CHARACTER_ORBIT_DISTANCE,
  hasCharacterView,
  validMmorpgEditingRequest,
  type CameraMode,
  type CharacterPerspective,
  type PreviewMessage,
  type PreviewEvent,
} from "../../packages/editor-services/protocol.ts";
import { WORLD_PROFILE } from "../../packages/editor-services/world-profile.ts";
import { BATTLE_ORBIT_RANGE } from "../../packages/runtime/battle-view.ts";
import {
  DRUID_ABILITIES,
  type DruidAbility,
} from "../../packages/runtime/druid-abilities.ts";
import { NpcOverlay } from "./npc-overlay.ts";
const canvas = document.querySelector<HTMLCanvasElement>("#world")!;
canvas.setAttribute("aria-label", WORLD_PROFILE.name);
const notice = document.querySelector<HTMLDivElement>("#notice")!;
const editorOrigin =
  import.meta.env.VITE_EDITOR_ORIGIN || "http://127.0.0.1:55304";
const worker = new Worker(new URL("./preview.worker.ts", import.meta.url), {
  type: "module",
});
let session = "",
  attached = false,
  mode: CameraMode = WORLD_PROFILE.mode;
let perspective: CharacterPerspective = "third";
let orbitDistance = DEFAULT_CHARACTER_ORBIT_DISTANCE;
let selectedAbility: DruidAbility | null = null;
let gameplayEnabled = false;
let castleAvailable = false;
let castleRunning = false;
let characterView = false;
let quickMmorpg = false;
let multiplayerMmorpg = false;
let mmorpgEditing = false;
let battleParticipation = false;
let battleView = "overview";
let mouseAttack = false;
let battleOrbitDrag: { id: number; x: number; y: number } | undefined;
let pointerPosition: { x: number; y: number } | undefined;
const npcOverlay = new NpcOverlay(document.body, (intent, npc) =>
  askNpc(intent, npc),
);
let npcPresent = false;
function askNpc(intent: "reply" | "hint" | "emote", npc?: string) {
  if (!session) return;
  worker.postMessage({
    type: "npc",
    intent,
    ...(npc ? { npc } : {}),
    protocol: PREVIEW_PROTOCOL,
    sessionId: session,
    requestId: crypto.randomUUID(),
  });
}
function sendAim(clientX?: number, clientY?: number) {
  const locked = document.pointerLockElement === canvas || !!battleOrbitDrag;
  if (!locked && clientX !== undefined && clientY !== undefined)
    pointerPosition = { x: clientX, y: clientY };
  const bounds = canvas.getBoundingClientRect();
  if (!bounds.width || !bounds.height) return;
  worker.postMessage({
    type: "aim",
    x: pointerPosition
      ? ((pointerPosition.x - bounds.left) / bounds.width) * 2 - 1
      : 0,
    y: pointerPosition
      ? 1 - ((pointerPosition.y - bounds.top) / bounds.height) * 2
      : 0,
    locked: locked || !pointerPosition,
  });
}
const navigation = (
  request:
    | { type: "perspective"; perspective: CharacterPerspective }
    | { type: "orbitDistance"; distance: number }
    | { type: "selectAbility"; ability: DruidAbility | null }
    | { type: "castAbility" }
    | { type: "cancelAbility" },
) => {
  if (!session || WORLD_PROFILE.readOnly) return;
  if (
    (request.type === "perspective" || request.type === "orbitDistance") &&
    (mode !== "walk" || !characterView)
  )
    return;
  if (
    (request.type === "selectAbility" ||
      request.type === "castAbility" ||
      request.type === "cancelAbility") &&
    !gameplayEnabled
  )
    return;
  if (request.type === "selectAbility") selectedAbility = request.ability;
  if (request.type === "selectAbility" || request.type === "castAbility")
    sendAim();
  worker.postMessage({
    ...request,
    protocol: PREVIEW_PROTOCOL,
    sessionId: session,
    requestId: crypto.randomUUID(),
  });
};
const keys = new Set<string>();
const touchMove = { forward: 0, right: 0 };
const touches = new Map<
  number,
  { x: number; y: number; lastX: number; lastY: number; move: boolean }
>();
let lastTouch = Number.NEGATIVE_INFINITY;
canvas.style.touchAction = "none";
const input = () =>
  worker.postMessage({
    type: "rawInput",
    forward: mmorpgEditing
      ? 0
      : Math.max(
          -1,
          Math.min(
            1,
            Number(keys.has("KeyW") || keys.has("ArrowUp")) -
              Number(keys.has("KeyS") || keys.has("ArrowDown")) +
              touchMove.forward,
          ),
        ),
    right: mmorpgEditing
      ? 0
      : Math.max(
          -1,
          Math.min(
            1,
            Number(keys.has("KeyD") || keys.has("ArrowRight")) -
              Number(keys.has("KeyA") || keys.has("ArrowLeft")) +
              touchMove.right,
          ),
        ),
    up: mmorpgEditing
      ? 0
      : Number(keys.has("Space")) - Number(keys.has("KeyC")),
    sprint: !mmorpgEditing && (keys.has("ShiftLeft") || keys.has("ShiftRight")),
    jump: !mmorpgEditing && !quickMmorpg && keys.has("Space"),
    attack: !mmorpgEditing && quickMmorpg && (keys.has("Space") || mouseAttack),
    dance: !mmorpgEditing && quickMmorpg && keys.has("Backspace"),
  });
window.addEventListener("message", (event) => {
  if (event.origin !== editorOrigin || event.source !== parent) return;
  const message = event.data as PreviewMessage;
  if (
    message?.protocol !== PREVIEW_PROTOCOL ||
    typeof message.sessionId !== "string" ||
    typeof message.requestId !== "string"
  )
    return;
  if (message.type === "initialize") {
    if (session && session !== message.sessionId) return;
    session = message.sessionId;
    if (!attached) {
      attached = true;
      const offscreen = canvas.transferControlToOffscreen();
      worker.postMessage(
        {
          type: "attach",
          canvas: offscreen,
          width: canvas.clientWidth,
          height: canvas.clientHeight,
          animationTime: message.animationTime,
        },
        [offscreen],
      );
    }
  } else if (message.sessionId !== session) return;
  if (message.type === "mmorpgEditing") {
    if (
      !validMmorpgEditingRequest(message) ||
      import.meta.env.VITE_PUBLIC_DEMO === "1"
    )
      return;
    clear();
    mmorpgEditing = message.editing;
    if (document.pointerLockElement === canvas) document.exitPointerLock();
  }
  if (message.type === "mode") clear();
  if (message.type === "selectAbility") {
    selectedAbility = message.ability;
    sendAim();
  }
  worker.postMessage(message);
});
worker.addEventListener("message", (event) => {
  const message = event.data as PreviewEvent;
  if (message.type === "npcLabels") {
    // Preview-local: the page places the labels; the editor never sees them.
    npcOverlay.update(message.labels);
    return;
  }
  if (message.type === "status" || message.type === "ready") {
    const status = message.status;
    npcPresent = !!status.npc?.npcs.length;
    if (!npcPresent) npcOverlay.clear();
    notice.textContent = status.phase === "ready" ? "" : status.message;
    if (status.view) mode = status.view.mode;
    gameplayEnabled = status.gameplay?.enabled === true;
    castleAvailable = !!status.gameplay?.castle?.total;
    castleRunning = !!(
      status.gameplay?.preparing ||
      status.gameplay?.castle?.active ||
      status.gameplay?.castle?.preparing
    );
    selectedAbility = status.gameplay?.selected ?? null;
    characterView = hasCharacterView(status.character);
    quickMmorpg = status.character?.controller === "quick-mmorpg-v1";
    multiplayerMmorpg = !!status.mmorpg;
    battleView = status.battle?.camera ?? "overview";
    battleParticipation = status.battle?.participant?.active === true && ["first", "third", "player"].includes(battleView);
    const editing = status.mmorpgEditing === true;
    if (editing !== mmorpgEditing) {
      mmorpgEditing = editing;
      clear();
    }
    if (((multiplayerMmorpg && !battleParticipation) || (status.battle?.participant?.ready && !status.battle.participant.alive)) && document.pointerLockElement === canvas)
      document.exitPointerLock();
    if (
      status.combat &&
      !status.combat.player.alive &&
      document.pointerLockElement === canvas
    )
      document.exitPointerLock();
    if (status.character) {
      perspective = status.character.perspective;
      orbitDistance = status.character.orbitDistance;
    }
  }
  if (message.type === "error") {
    notice.textContent = message.message;
  }
  parent.postMessage(message, editorOrigin, previewEventTransfers(message));
});
worker.addEventListener("error", (event) => {
  notice.textContent = event.message;
  parent.postMessage(
    {
      protocol: PREVIEW_PROTOCOL,
      sessionId: session,
      type: "error",
      message: event.message,
    },
    editorOrigin,
  );
});
new ResizeObserver(() => {
  if (attached) {
    worker.postMessage({
      type: "resize",
      width: canvas.clientWidth,
      height: canvas.clientHeight,
    });
    sendAim();
  }
}).observe(canvas);
canvas.addEventListener("click", (event) => {
  if (performance.now() - lastTouch < 700) return;
  canvas.focus();
  if (battleParticipation) {
    if (document.pointerLockElement !== canvas) void canvas.requestPointerLock();
    return;
  }
  if (multiplayerMmorpg) {
    if (mmorpgEditing) pickAt(event.clientX, event.clientY);
    return;
  }
  if (selectedAbility) {
    sendAim(event.clientX, event.clientY);
    if (!castleRunning) navigation({ type: "castAbility" });
  } else {
    if (document.pointerLockElement !== canvas)
      void canvas.requestPointerLock();
    else worker.postMessage({ type: "pick" });
  }
});
function pickAt(clientX: number, clientY: number) {
  const bounds = canvas.getBoundingClientRect();
  if (!bounds.width || !bounds.height) return;
  worker.postMessage({
    type: "pick",
    ndc: [
      Math.max(
        -1,
        Math.min(1, ((clientX - bounds.left) / bounds.width) * 2 - 1),
      ),
      Math.max(
        -1,
        Math.min(1, 1 - ((clientY - bounds.top) / bounds.height) * 2),
      ),
    ],
  });
}
canvas.addEventListener("contextmenu", (event) => {
  if (battleParticipation) event.preventDefault();
  if (!selectedAbility) return;
  event.preventDefault();
  navigation({ type: "selectAbility", ability: null });
});
canvas.addEventListener("pointerdown", (event) => {
  if (battleParticipation && battleView !== "first" && event.pointerType !== "touch" && event.button === 2 && document.pointerLockElement !== canvas) {
    event.preventDefault(); canvas.focus();
    battleOrbitDrag = { id: event.pointerId, x: event.clientX, y: event.clientY };
    canvas.setPointerCapture(event.pointerId);
    pointerPosition = undefined; sendAim();
    return;
  }
  if (battleParticipation && event.pointerType !== "touch" && event.button === 0 && document.pointerLockElement === canvas) {
    mouseAttack = true; input();
  }
  if (event.pointerType !== "touch") return;
  event.preventDefault();
  lastTouch = performance.now();
  canvas.focus();
  if (mmorpgEditing) {
    pickAt(event.clientX, event.clientY);
    return;
  }
  canvas.setPointerCapture(event.pointerId);
  const bounds = canvas.getBoundingClientRect();
  const move = event.clientX - bounds.left < bounds.width * 0.35;
  if ([...touches.values()].some((t) => t.move === move)) return;
  touches.set(event.pointerId, {
    x: event.clientX,
    y: event.clientY,
    lastX: event.clientX,
    lastY: event.clientY,
    move,
  });
});
canvas.addEventListener("pointermove", (event) => {
  if (event.pointerType !== "touch") {
    if (battleOrbitDrag?.id === event.pointerId && document.pointerLockElement !== canvas) {
      worker.postMessage({type: "look", yaw: (event.clientX - battleOrbitDrag.x) * .004, pitch: -(event.clientY - battleOrbitDrag.y) * .004});
      battleOrbitDrag.x = event.clientX; battleOrbitDrag.y = event.clientY;
      sendAim(); return;
    }
    sendAim(event.clientX, event.clientY);
    return;
  }
  const touch = touches.get(event.pointerId);
  if (!touch) return;
  event.preventDefault();
  lastTouch = performance.now();
  if (touch.move) {
    touchMove.forward = Math.max(
      -1,
      Math.min(1, (touch.y - event.clientY) / 60),
    );
    touchMove.right = Math.max(-1, Math.min(1, (event.clientX - touch.x) / 60));
    input();
  } else
    worker.postMessage({
      type: "look",
      yaw: (event.clientX - touch.lastX) * 0.004,
      pitch: -(event.clientY - touch.lastY) * 0.004,
    });
  touch.lastX = event.clientX;
  touch.lastY = event.clientY;
});
const releaseTouch = (event: PointerEvent) => {
  const touch = touches.get(event.pointerId);
  if (!touch) return;
  touches.delete(event.pointerId);
  lastTouch = performance.now();
  if (touch.move) {
    touchMove.forward = 0;
    touchMove.right = 0;
    input();
  } else if (
    event.type === "pointerup" &&
    selectedAbility &&
    !castleRunning &&
    Math.hypot(event.clientX - touch.x, event.clientY - touch.y) < 12
  ) {
    sendAim(event.clientX, event.clientY);
    navigation({ type: "castAbility" });
  } else if (
    event.type === "pointerup" &&
    !selectedAbility &&
    Math.hypot(event.clientX - touch.x, event.clientY - touch.y) < 12
  ) {
    worker.postMessage({ type: "pick" });
  }
};
canvas.addEventListener("pointerup", releaseTouch);
canvas.addEventListener("pointercancel", releaseTouch);
canvas.addEventListener("lostpointercapture", (event) => {
  if (battleOrbitDrag?.id === event.pointerId) {
    battleOrbitDrag = undefined;
    pointerPosition = undefined; sendAim();
  }
});
window.addEventListener("mousemove", (event) => {
  if (document.pointerLockElement === canvas)
    worker.postMessage({
      type: "look",
      yaw: event.movementX * 0.002,
      pitch: -event.movementY * 0.002,
    });
});
window.addEventListener("keydown", (event) => {
  if (battleParticipation && session && !event.repeat && !event.altKey && !event.ctrlKey && !event.metaKey && ["KeyV", "Digit1", "Digit2", "Digit3"].includes(event.code)) {
    event.preventDefault();
    worker.postMessage({type: "battle", protocol: PREVIEW_PROTOCOL, sessionId: session, requestId: crypto.randomUUID(),
      action: event.code === "KeyV" ? {op: "camera", view: battleView === "first" ? "third" : "first"} : {op: "weapon", attackKind: event.code === "Digit1" ? "melee" : event.code === "Digit2" ? "ranged" : "meteor_shower"}});
    return;
  }
  if (mmorpgEditing) {
    if (event.code === "Escape" && !event.repeat && session) {
      event.preventDefault();
      clear();
      worker.postMessage({
        type: "mmorpgEditing",
        editing: false,
        protocol: PREVIEW_PROTOCOL,
        sessionId: session,
        requestId: crypto.randomUUID(),
      });
    } else if (
      [
        "KeyW",
        "KeyA",
        "KeyS",
        "KeyD",
        "ArrowUp",
        "ArrowDown",
        "ArrowLeft",
        "ArrowRight",
        "Space",
        "Backspace",
      ].includes(event.code)
    )
      event.preventDefault();
    return;
  }
  if (
    multiplayerMmorpg && !battleParticipation &&
    event.code === "Enter" &&
    !event.repeat &&
    !event.altKey &&
    !event.ctrlKey &&
    !event.metaKey &&
    session
  ) {
    event.preventDefault();
    clear();
    if (document.pointerLockElement === canvas) document.exitPointerLock();
    parent.postMessage(
      {
        protocol: PREVIEW_PROTOCOL,
        sessionId: session,
        type: "mmorpgFocusChat",
      },
      editorOrigin,
    );
    return;
  }
  if (
    npcPresent &&
    event.code === "KeyH" &&
    !event.repeat &&
    !event.altKey &&
    !event.ctrlKey &&
    !event.metaKey &&
    session
  ) {
    event.preventDefault();
    askNpc("hint");
    return;
  }
  if (
    WORLD_PROFILE.id !== "open-ground" &&
    event.code === "KeyE" &&
    !event.repeat &&
    session
  ) {
    event.preventDefault();
    worker.postMessage({
      type: "game",
      action: "interact",
      protocol: PREVIEW_PROTOCOL,
      sessionId: session,
      requestId: crypto.randomUUID(),
    });
    return;
  }
  if (event.code === "Escape") {
    if (castleRunning) navigation({ type: "cancelAbility" });
    if (selectedAbility) navigation({ type: "selectAbility", ability: null });
    document.exitPointerLock();
    clear();
    return;
  }
  if (
    gameplayEnabled &&
    !event.repeat &&
    !event.altKey &&
    !event.ctrlKey &&
    !event.metaKey
  ) {
    const ability = DRUID_ABILITIES.find(
      (entry) =>
        event.code === `Digit${entry.key}` ||
        event.code === `Numpad${entry.key}`,
    );
    if (ability) {
      if (ability.id === "destroy_castle" && !castleAvailable) return;
      event.preventDefault();
      navigation({
        type: "selectAbility",
        ability: selectedAbility === ability.id ? null : ability.id,
      });
      return;
    }
    if (event.code === "Enter" && selectedAbility && !castleRunning) {
      event.preventDefault();
      navigation({ type: "castAbility" });
      return;
    }
  }
  if (
    !quickMmorpg &&
    event.code === "KeyV" &&
    !event.repeat &&
    !event.altKey &&
    !event.ctrlKey &&
    !event.metaKey
  ) {
    event.preventDefault();
    navigation({
      type: "perspective",
      perspective: perspective === "third" ? "first" : "third",
    });
    return;
  }
  if (
    [
      "KeyW",
      "KeyA",
      "KeyS",
      "KeyD",
      "ArrowUp",
      "ArrowDown",
      "ArrowLeft",
      "ArrowRight",
      "Space",
      ...(quickMmorpg ? ["Backspace"] : []),
      "KeyC",
      "ShiftLeft",
      "ShiftRight",
    ].includes(event.code)
  ) {
    event.preventDefault();
    keys.add(event.code);
    input();
  }
});
canvas.addEventListener(
  "wheel",
  (event) => {
    if (
      mode !== "walk" ||
      (quickMmorpg && !(battleParticipation && battleView !== "first")) ||
      WORLD_PROFILE.readOnly ||
      event.ctrlKey ||
      !Number.isFinite(event.deltaY)
    )
      return;
    event.preventDefault();
    const [min, max] = battleParticipation ? BATTLE_ORBIT_RANGE : CHARACTER_ORBIT_RANGE;
    orbitDistance = Math.max(
      min,
      Math.min(
        max,
        orbitDistance *
          Math.exp(Math.max(-1, Math.min(1, event.deltaY * 0.0015))),
      ),
    );
    navigation({ type: "orbitDistance", distance: orbitDistance });
  },
  { passive: false },
);
window.addEventListener("keyup", (event) => {
  keys.delete(event.code);
  input();
});
window.addEventListener("pointerup", (event) => {
  if (battleOrbitDrag?.id === event.pointerId) {
    battleOrbitDrag = undefined;
    if (canvas.hasPointerCapture(event.pointerId)) canvas.releasePointerCapture(event.pointerId);
    pointerPosition = undefined; sendAim();
  }
  if (event.pointerType !== "touch" && mouseAttack) { mouseAttack = false; input(); }
});
const clear = () => {
  if (battleOrbitDrag && canvas.hasPointerCapture(battleOrbitDrag.id)) canvas.releasePointerCapture(battleOrbitDrag.id);
  battleOrbitDrag = undefined;
  if (battleParticipation) worker.postMessage({type: "clearBattleInput"});
  mouseAttack = false;
  touches.clear();
  touchMove.forward = 0;
  touchMove.right = 0;
  keys.clear();
  input();
};
window.addEventListener("blur", clear);
document.addEventListener("visibilitychange", () => {
  if (document.hidden) clear();
});
document.addEventListener("pointerlockchange", () => {
  pointerPosition = undefined;
  sendAim();
  if (document.pointerLockElement !== canvas) clear();
});
window.addEventListener("pagehide", () => {
  clear();
  worker.postMessage({ type: "dispose" });
  // A cached page can freeze the worker before its queued dispose runs.
  worker.terminate();
});
