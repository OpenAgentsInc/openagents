import type {
  MmoActor,
  MmoEvent,
  MmoPoint,
} from "../../../packages/runtime/mmorpg-protocol.ts";

export const MMO_ITEM_NAMES = {
  "weapon.sword1": "Sword",
  "weapon.axe1": "Axe",
  "weapon.hammer1": "Hammer",
} as const;
/** Exact tooltip copy from the source client's index.html statistics panel. */
export const MMO_STAT_ROWS = [
  {
    key: "strength",
    label: "Strength",
    tooltip:
      "How strong you are, affects how much damage you do. So blah blah if you're doing stuff then its stronger or whatever, the damage is up. This is text to show the tooltip.",
  },
  {
    key: "wisdomness",
    label: "Wisdomness",
    tooltip:
      "Wisdom is the guage of something to do with wisdom in the game because wisdom is important and wisdom is wise to wisdoming.",
  },
  {
    key: "benchpress",
    label: "Benchpress",
    tooltip: "Gotta have a good bench to be a warrior or something.",
  },
  {
    key: "curl",
    label: "Curl",
    tooltip:
      "The ultimate expression of strength, this affects literally everything in your life.",
  },
  {
    key: "experience",
    label: "XP",
    tooltip:
      "How much xp you've accumulated by killing things for xp. Get enough and you'll gain a level or something.",
  },
] as const;
export const MMO_NAME_HEIGHT = {
  paladin: 11 / 6,
  sorceror: 10 / 6,
  zombie: 8 / 6,
  warrok: 16 / 6,
} as const;
/** Only a successful device save is shown as saved. */
export const mmoDraftLabel = (saved: boolean): string =>
  saved ? "Saved on this device" : "Local draft";
export interface MmoLogLine {
  id: number;
  text: string;
  kind: "chat" | "server" | "action";
}
/** Keep the source chat and local combat feed together without showing remote fights. */
export function mmoLogLines(
  events: readonly MmoEvent[],
  player: MmoActor,
  actors: readonly MmoActor[],
): MmoLogLine[] {
  const names = new Map(actors.map((a) => [a.id, a.name]));
  names.set(player.id, player.name);
  const name = (id?: number) =>
    id === undefined ? "Someone" : (names.get(id) ?? "Someone");
  const lines: MmoLogLine[] = [];
  for (const e of events) {
    if (e.kind === "chat")
      lines.push({
        id: e.id,
        kind: "chat",
        text: `[${e.name ?? name(e.actor)}]: ${e.text ?? ""}`,
      });
    else if (e.kind === "join" || e.kind === "leave")
      lines.push({
        id: e.id,
        kind: "server",
        text: `[${e.name ?? name(e.actor)} has ${e.kind === "join" ? "entered" : "left"} the game]`,
      });
    else if (
      e.kind === "damage" &&
      (e.actor === player.id || e.target === player.id)
    ) {
      const source = e.actor === player.id ? "You" : name(e.actor);
      const target = e.target === player.id ? "you" : name(e.target);
      lines.push({
        id: e.id,
        kind: "action",
        text: `${source} hit ${target} for ${Math.round(e.amount ?? 0)} damage!`,
      });
    } else if (e.kind === "death" && e.actor === player.id)
      lines.push({ id: e.id, kind: "action", text: "You were defeated." });
  }
  return lines.slice(-40);
}
export interface MmoView {
  eye: MmoPoint;
  target: MmoPoint;
}
/** Perspective projection for a presentation overlay; world poses stay in the engine. */
export function projectMmoName(
  point: MmoPoint,
  view: MmoView,
  width: number,
  height: number,
  fov = 60,
): [number, number] | null {
  if (!(width > 0 && height > 0 && fov > 0 && fov < 179)) return null;
  let fx = view.target[0] - view.eye[0],
    fy = view.target[1] - view.eye[1],
    fz = view.target[2] - view.eye[2];
  const length = Math.hypot(fx, fy, fz);
  if (length < 1e-6) return null;
  fx /= length;
  fy /= length;
  fz /= length;
  const horizontal = Math.hypot(fx, fz);
  if (horizontal < 1e-6) return null;
  const rx = -fz / horizontal,
    rz = fx / horizontal;
  const ux = -rz * fy,
    uy = rz * fx - rx * fz,
    uz = rx * fy;
  const dx = point[0] - view.eye[0],
    dy = point[1] - view.eye[1],
    dz = point[2] - view.eye[2];
  const depth = dx * fx + dy * fy + dz * fz;
  if (depth < 0.15) return null;
  const scale = height / (2 * Math.tan((fov * Math.PI) / 360));
  const x = width / 2 + ((dx * rx + dz * rz) * scale) / depth;
  const y = height / 2 - ((dx * ux + dy * uy + dz * uz) * scale) / depth;
  return x < -100 || x > width + 100 || y < -30 || y > height + 30
    ? null
    : [x, y];
}

/** UI artwork is served by the editor origin so COEP never needs a cross-site image. */
export function mmoUiImageURL(path: string, editorOrigin: string): string {
  if (!/^\/private-assets\/[A-Za-z0-9_.-]+\.(?:png|jpg|jpeg)$/.test(path))
    throw Error("Choose a prepared game image.");
  const base = new URL(editorOrigin);
  if (!["http:", "https:"].includes(base.protocol))
    throw Error("Open the game from its web address.");
  return new URL(path, base.origin).href;
}

/** Exact source canvas styling; its 10 by 5 source-unit quad is projected separately. */
export function paintMmoName(context: CanvasRenderingContext2D, name: string) {
  context.clearRect(0, 0, 256, 128);
  context.fillStyle = "#FFF";
  context.font = "18pt Helvetica";
  context.shadowOffsetX = 3;
  context.shadowOffsetY = 3;
  context.shadowColor = "rgba(0,0,0,0.3)";
  context.shadowBlur = 4;
  context.textAlign = "center";
  context.fillText(name, 128, 64);
}

export function mmoNameSpriteSize(
  point: MmoPoint,
  view: MmoView,
  viewportHeight: number,
  fov = 60,
): [number, number] | null {
  const forward = view.target.map((n, i) => n - view.eye[i]);
  const length = Math.hypot(...forward);
  if (length < 1e-6 || viewportHeight <= 0 || fov <= 0 || fov >= 179)
    return null;
  const depth = point.reduce(
    (sum, n, i) => sum + ((n - view.eye[i]) * forward[i]) / length,
    0,
  );
  if (depth < 0.15) return null;
  const scale = viewportHeight / (2 * Math.tan((fov * Math.PI) / 360) * depth);
  return [(10 / 6) * scale, (5 / 6) * scale];
}
