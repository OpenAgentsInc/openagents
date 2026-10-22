import { IS_TOWN } from "./lab.ts";
import { TownFloor } from "./town.ts";
import { DEFAULT_LAYOUT, HouseFloor } from "./layout.ts";
import { projectFloor, type Operation, type Project } from "./project.ts";

const el = <T extends HTMLElement>(id: string) =>
  document.getElementById(id) as T;
const svg = (name: string, attributes: Record<string, string | number>) => {
  const node = document.createElementNS("http://www.w3.org/2000/svg", name);
  for (const [key, value] of Object.entries(attributes))
    node.setAttribute(key, String(value));
  return node;
};
export function createLayoutControls(bridge: {
  state: () => any;
  submit: (op: Operation) => Promise<unknown>;
  cancel: () => void;
  error: (error: unknown) => void;
}) {
  const selector = el<HTMLSelectElement>("wall-selected");
  let revision = -1,
    floor: HouseFloor | TownFloor = new HouseFloor(),
    ready = false,
    busy = false;
  function target() {
    if (selector.value === "aim") {
      const aimed = floor.pick(bridge.state().pose);
      return aimed ? { ...aimed, x: aimed.hitX, z: aimed.hitZ } : undefined;
    }
    const wall = floor.surfaces
      .filter((w) => w.id === selector.value)
      .sort(
        (a, b) =>
          Number(b.exterior) - Number(a.exterior) ||
          Math.hypot(b.x2 - b.x1, b.z2 - b.z1) -
            Math.hypot(a.x2 - a.x1, a.z2 - a.z1),
      )[0];
    return wall
      ? { ...wall, x: (wall.x1 + wall.x2) / 2, z: (wall.z1 + wall.z2) / 2 }
      : undefined;
  }
  function refresh(isReady = ready, isBusy = busy) {
    ready = isReady;
    busy = isBusy;
    const state = bridge.state(),
      project = state.project as Project | undefined;
    if (!project) return;
    if (revision !== project.revision) {
      revision = project.revision;
      floor = projectFloor(project);
      const previous = selector.value || "aim";
      selector.replaceChildren();
      const aim = document.createElement("option");
      aim.value = "aim";
      aim.textContent = "Wall I’m looking at";
      selector.append(aim);
      for (const [id, wall] of new Map(floor.surfaces.map((w) => [w.id, w]))) {
        const option = document.createElement("option");
        option.value = id;
        option.textContent = wall.name + (wall.removed ? " · open" : "");
        selector.append(option);
      }
      selector.value = Array.from(selector.options).some(
        (o) => o.value === previous,
      )
        ? previous
        : "aim";
      const plan = document.getElementById("floorplan")!;
      const rooms = floor.layout.rooms,
        minX = Math.min(...rooms.map((r) => r.x - r.width / 2)) - 2,
        minZ = Math.min(...rooms.map((r) => r.z - r.depth / 2)) - 2,
        maxX = Math.max(...rooms.map((r) => r.x + r.width / 2)) + 2,
        maxZ = Math.max(...rooms.map((r) => r.z + r.depth / 2)) + 2;
      plan.setAttribute(
        "viewBox",
        `${minX} ${minZ} ${maxX - minX} ${maxZ - minZ}`,
      );
      plan.replaceChildren();
      for (const room of rooms) {
        plan.append(
          svg("rect", {
            x: room.x - room.width / 2,
            y: room.z - room.depth / 2,
            width: room.width,
            height: room.depth,
            fill: "#29342f",
            stroke: "none",
          }),
        );
        const label = svg("text", {
          x: room.x,
          y: room.z,
          "font-size": 0.65,
          "text-anchor": "middle",
          fill: "#b2b7a4",
        });
        label.textContent =
          room.id === "hall"
            ? "Hall"
            : room.id === "passage"
              ? "Passage"
              : room.id === "gallery"
                ? "Gallery"
                : String(rooms.indexOf(room) + 1);
        plan.append(label);
      }
      for (const wall of floor.surfaces) {
        const line = svg("line", {
          x1: wall.x1,
          y1: wall.z1,
          x2: wall.x2,
          y2: wall.z2,
          stroke: wall.removed ? "#6c785e" : "#c3b99c",
          "stroke-width": 0.15,
          "stroke-dasharray": wall.removed ? ".35 .3" : "none",
        });
        line.addEventListener("click", () => {
          selector.value = wall.id;
          bridge.cancel();
          refresh();
        });
        plan.append(line);
      }
      plan.append(
        svg("line", { id: "map-aim", stroke: "#e9c47d", "stroke-width": 0.2 }),
        svg("circle", { id: "map-player", r: 0.3, fill: "#e9c47d" }),
      );
      el("room-count").textContent = `${rooms.length} rooms`;
    }
    const room = floor.layout.rooms.find(
      (r) =>
        Math.abs((state.pose?.x ?? 0) - r.x) <= r.width / 2 &&
        Math.abs((state.pose?.z ?? 0) - r.z) <= r.depth / 2,
    );
    el("current-room").textContent = IS_TOWN
      ? state.stairs
        ? "Stairs"
        : state.room
          ? `${state.room.buildingName} · ${state.room.level ? "Upstairs · " : ""}${state.room.room.name}`
          : "Town square"
      : (room?.name ?? "Haunted House");
    const wall = target();
    el("wall-target").textContent = wall
      ? `${wall.removed ? "Opening" : "Target"}: ${wall.name}`
      : "Look toward a wall or choose one below.";
    el("wall-aim").textContent = wall
      ? wall.name
      : "Look toward a wall to build";
    for (const id of ["wall-selected", "room-size"])
      el<HTMLSelectElement>(id).disabled = !ready || busy;
    for (const id of ["add-room", "add-wing"])
      el<HTMLButtonElement>(id).disabled = !ready || busy || !wall?.exterior;
    el<HTMLButtonElement>("remove-wall").disabled =
      !ready || busy || !wall || wall.removed;
    const dot = document.getElementById("map-player"),
      line = document.getElementById("map-aim");
    dot?.setAttribute("cx", String(state.pose?.x ?? 0));
    dot?.setAttribute("cy", String(state.pose?.z ?? 0));
    for (const [key, value] of Object.entries({
      x1: state.pose?.x ?? 0,
      y1: state.pose?.z ?? 0,
      x2: wall?.x ?? state.pose?.x ?? 0,
      y2: wall?.z ?? state.pose?.z ?? 0,
    }))
      line?.setAttribute(key, String(value));
  }
  selector.addEventListener("change", () => {
    bridge.cancel();
    refresh();
  });
  for (const kind of ["add_room", "add_wing", "remove_wall"] as const)
    el(kind.replaceAll("_", "-")).addEventListener("click", () => {
      const wall = target();
      if (!wall) return;
      const op: Operation =
        kind === "remove_wall"
          ? { kind, wall: wall.id }
          : {
              kind,
              wall: wall.id,
              size: Number(el<HTMLSelectElement>("room-size").value),
              at: { x: wall.x, z: wall.z },
            };
      void bridge.submit(op).catch(bridge.error);
    });
  return { refresh, target, selection: () => selector.value };
}
