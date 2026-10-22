import { compileDecision } from "./compound.ts";
import { isHouse } from "./town.ts";
import { IS_TOWN, LAB } from "./lab.ts";
import { createLayoutControls } from "./layout-controls.ts";
import { Effect } from "effect";
import {
  PROJECT_ID,
  ASSETS,
  type AssetKind,
  type Behavior,
  SCENE_PATH,
  MAX_SCENE_BYTES,
  readProject,
  cleanHaunt,
  type Decision,
  type Operation,
  type Project,
  type Transaction,
  type Tint,
} from "./project.ts";

type Bridge = {
  post: (message: object) => void;
  state: () => any;
  workspace: <T>(job: () => Promise<T>) => Promise<T>;
};
const el = <T extends HTMLElement>(id: string) =>
  document.getElementById(id) as T;
const clock = () => performance.timeOrigin + performance.now();
export function createEditor(bridge: Bridge) {
  let ready = false,
    busy = false,
    selected: string | null = null,
    decision: AbortController | undefined,
    epoch = 0;
  let rendered = -1;
  let initialized = false;
  const pending = new Map<
    string,
    { resolve: (r: any) => void; reject: (e: Error) => void; started: number }
  >();
  const selfWrites = new Set<string>();
  const receipts: any[] = [];
  const decisions: any[] = [];
  const explain = (error: unknown) =>
    (error instanceof Error ? error.message : String(error)).replace(
      /^(?:Error:\s*)+/,
      "",
    );
  const status = (s: string) => {
    el("scene-status").textContent = s;
  };
  const layoutControls = createLayoutControls({
    state: bridge.state,
    submit,
    cancel,
    error: (error) => status(explain(error)),
  });
  function refresh() {
    layoutControls.refresh(ready, busy);
    const state = bridge.state(),
      p = state.project as Project | undefined;
    for (const id of [
      "asset-kind",
      "add-pumpkin",
      "move-pumpkin",
      "remove-pumpkin",
      "resize-pumpkin",
      "tint",
      "ask",
    ])
      el<HTMLButtonElement>(id).disabled = !ready || busy;
    el<HTMLButtonElement>("undo").disabled = !ready || busy || !state.canUndo;
    el<HTMLButtonElement>("redo").disabled = !ready || busy || !state.canRedo;
    el<HTMLButtonElement>("cancel-decision").hidden = !decision;
    if (IS_TOWN)
      el("kit-status").textContent = state.medieval?.available
        ? "8 kit assets · Enter a house to change its rooms and walls."
        : (state.medieval?.error ??
          "Medieval assets need the local Everglade kit.");
    if (IS_TOWN)
      for (const option of el<HTMLSelectElement>("asset-kind").options) {
        const item = ASSETS[option.value as AssetKind];
        option.disabled =
          !!item && "source" in item && !state.medieval?.available;
      }
    const ghost = p?.entities.find(
      (e) => e.id === selected && e.kind === "ghost",
    );
    for (const id of ["float-ghost", "chase-ghost", "stop-ghost"])
      el<HTMLButtonElement>(id).disabled = !ready || busy || !ghost;
    el<HTMLButtonElement>("float-ghosts").disabled =
      !ready || busy || !p?.entities.some((e) => e.kind === "ghost");
    el("ghost-behavior").textContent = ghost
      ? `Selected ghost · ${ghost.behavior === "float" ? "Floating around" : ghost.behavior === "chase" ? "Chasing you" : "Still"}`
      : "Select a ghost to give it a behavior.";
    if (!p || rendered === p.revision) return;
    rendered = p.revision;
    const select = el<HTMLSelectElement>("selected");
    if (!p.entities.some((e) => e.id === selected))
      selected = p.entities[0]?.id ?? null;
    select.replaceChildren(
      ...p.entities.map((e, i) => {
        const option = document.createElement("option");
        option.value = e.id;
        option.textContent = `${ASSETS[e.kind].label} ${i + 1} · (${e.x}, ${e.z})`;
        return option;
      }),
    );
    if (selected) select.value = selected;
    const entity = p.entities.find((e) => e.id === selected);
    if (entity)
      for (const key of ["x", "z", "scale"] as const)
        el<HTMLInputElement>(key).value = String(entity[key]);
    el<HTMLSelectElement>("tint").value = p.tint;
    el("scene-count").textContent = `${p.entities.length} assets`;
  }
  function cancel(message = "Request canceled. Your scene is unchanged.") {
    if (decision) {
      decision.abort();
      decision = undefined;
      epoch++;
      status(message);
      refresh();
    }
  }
  async function persist(project: Project) {
    const text = JSON.stringify(project);
    selfWrites.add(text);
    try {
      const code = await bridge.workspace(() =>
        window.kilnWrite(SCENE_PATH, text),
      );
      if (code !== 0)
        throw new Error(
          "The scene is live, but Kiln couldn't save it. Try saving again.",
        );
      status(`Saved in Kiln · ${project.entities.length} assets`);
    } finally {
      selfWrites.delete(text);
    }
  }
  async function submit(
    operation: Operation,
    origin = "manual",
    baseRevision = bridge.state().project.revision,
  ) {
    if (!ready || busy)
      throw new Error("Wait for the current scene change to finish.");
    if (origin === "manual") cancel("Applying your change…");
    busy = true;
    refresh();
    const request = crypto.randomUUID();
    const started = clock();
    const transaction: Transaction = {
      id: request,
      project: PROJECT_ID,
      baseRevision,
      operation,
    };
    try {
      const result = await new Promise<any>((resolve, reject) => {
        pending.set(request, { resolve, reject, started });
        bridge.post({
          type: "scene",
          project: PROJECT_ID,
          request,
          transaction,
        });
      });
      if (operation.kind === "add_asset" || operation.kind === "add_pumpkin")
        selected = operation.entity.id;
      if (operation.kind === "batch") {
        const added = operation.operations.findLast(
          (op) => op.kind === "add_asset" || op.kind === "add_pumpkin",
        );
        if (added?.kind === "add_asset" || added?.kind === "add_pumpkin")
          selected = added.entity.id;
      }
      const receipt = {
        origin,
        operation,
        revision: result.state.project.revision,
        submittedMs: result.visibleAt - started,
      };
      receipts.push(receipt);
      if (receipts.length > 64) receipts.shift();
      rendered = -1;
      refresh();
      status(`Changed · ${Math.round(receipt.submittedMs)} ms`);
      await persist(result.state.project);
      return receipt;
    } finally {
      busy = false;
      refresh();
    }
  }
  async function interpret(text: string) {
    if (!ready || busy)
      throw new Error("Wait for the current scene change to finish.");
    if (!text.trim() || new TextEncoder().encode(text).length > 1200)
      throw new Error("Describe your changes in a short request.");
    cancel();
    const controller = new AbortController();
    decision = controller;
    const requestEpoch = ++epoch;
    const base = bridge.state().project.revision;
    const selectedAtStart = selected;
    const wallAtStart = layoutControls.target();
    const wallChoice = layoutControls.selection();
    const poseAtStart = { ...bridge.state().pose };
    const started = clock();
    status(
      "Choosing your scene changes… You can keep walking or use the controls.",
    );
    refresh();
    const timeout = setTimeout(() => controller.abort(), 2700);
    try {
      const d = await Effect.runPromise(
        Effect.tryPromise({
          try: async () => {
            const response = await fetch(LAB.decision, {
              method: "POST",
              headers: { "Content-Type": "application/json" },
              body: JSON.stringify({
                text,
                selected: !!selectedAtStart,
                lab: IS_TOWN ? "medieval" : "haunt",
              }),
              signal: controller.signal,
            });
            const result = await response.json().catch(() => {
              throw new Error(
                "Natural-language edits need the local decision service. Use the scene controls or start the dev server.",
              );
            });
            if (!response.ok)
              throw new Error(
                result.error ||
                  "The request couldn't finish. Use the scene controls or try again.",
              );
            return result as Decision;
          },
          catch: (e) => (e instanceof Error ? e : new Error(String(e))),
        }),
      );
      if (controller.signal.aborted || requestEpoch !== epoch)
        throw new Error("That request was canceled.");
      if (
        bridge.state().project.revision !== base ||
        selected !== selectedAtStart ||
        wallChoice !== layoutControls.selection()
      )
        throw new Error(
          "The scene changed while this request was pending. Try it again.",
        );
      const roundTripMs = clock() - started;
      const operation = compileDecision(
        d,
        text,
        d.action === "compound" ||
          ["add_room", "add_wing", "remove_wall"].includes(d.action)
          ? poseAtStart
          : bridge.state().pose,
        selectedAtStart,
        bridge.state().project,
        wallAtStart
          ? { id: wallAtStart.id, x: wallAtStart.x, z: wallAtStart.z }
          : undefined,
      );
      decision = undefined;
      clearTimeout(timeout);
      refresh();
      const receipt = await submit(operation, "decision", base);
      const result = {
        text,
        decision: d,
        roundTripMs,
        totalMs: clock() - started,
        receipt,
      };
      decisions.push(result);
      if (decisions.length > 32) decisions.shift();
      status(`Changed and saved · ${Math.round(result.totalMs)} ms`);
      el("decision-details").textContent = JSON.stringify(
        {
          model: d.model,
          confidence: d.confidence,
          modelMs: d.model_ms,
          roundTripMs,
          frameMs: receipt.submittedMs,
          operation,
        },
        null,
        2,
      );
      return result;
    } catch (error) {
      if (requestEpoch === epoch)
        status(
          controller.signal.aborted
            ? "That took too long. Use a scene control or try again."
            : explain(error),
        );
      throw error;
    } finally {
      clearTimeout(timeout);
      if (decision === controller) decision = undefined;
      refresh();
    }
  }
  function receive(message: any) {
    if (message.type === "lost" || message.type === "disposed") {
      for (const job of pending.values())
        job.reject(
          new Error("The preview closed. Reopen it to edit the scene."),
        );
      pending.clear();
    }
    if (
      message.type === "scene-accepted" ||
      message.type === "scene-rejected"
    ) {
      const job = pending.get(message.request);
      pending.delete(message.request);
      if (message.type === "scene-accepted") job?.resolve(message);
      else job?.reject(new Error(message.message));
    }
    refresh();
  }
  async function ingest(text: string, restore = false) {
    if (selfWrites.has(text)) return;
    try {
      if (new TextEncoder().encode(text).length > MAX_SCENE_BYTES)
        throw new Error("The scene file is too large.");
      const project = readProject(JSON.parse(text));
      if (JSON.stringify(project) === JSON.stringify(bridge.state().project))
        return;
      if (restore) {
        const request = crypto.randomUUID();
        await new Promise((resolve, reject) => {
          pending.set(request, { resolve, reject, started: clock() });
          bridge.post({
            type: "scene",
            project: PROJECT_ID,
            request,
            restore: project,
          });
        });
        rendered = -1;
        refresh();
        if (
          !IS_TOWN &&
          JSON.stringify(cleanHaunt(JSON.parse(text))) !==
            JSON.stringify(JSON.parse(text))
        )
          await persist(project);
        status("Opened your saved Kiln scene.");
      } else
        await submit(
          { kind: "replace", project },
          "kiln",
          project.revision - 1,
        );
    } catch (e) {
      status(explain(e));
    }
  }
  const attempt = (f: () => Promise<unknown>) => {
    void f().catch((e) => status(explain(e)));
  };
  const position = () => ({
    x: el<HTMLInputElement>("x").valueAsNumber,
    z: el<HTMLInputElement>("z").valueAsNumber,
  });
  const picker = el<HTMLSelectElement>("asset-kind");
  for (const [kind, asset] of Object.entries(ASSETS)) {
    const option = document.createElement("option");
    option.value = kind;
    option.textContent = asset.label;
    picker.append(option);
  }
  el("add-pumpkin").addEventListener("click", () =>
    attempt(() =>
      submit({
        kind: "add_asset",
        entity: {
          id: `${picker.value}-${crypto.randomUUID()}`,
          kind: picker.value as AssetKind,
          ...position(),
          ...(IS_TOWN
            ? { y: isHouse(picker.value) ? 0 : (bridge.state().pose?.y ?? 0) }
            : {}),
          scale: el<HTMLInputElement>("scale").valueAsNumber,
        },
      }),
    ),
  );
  el("move-pumpkin").addEventListener("click", () =>
    attempt(() =>
      submit({
        kind: "move_asset",
        id: selected ?? "",
        ...position(),
        ...(IS_TOWN
          ? {
              y: isHouse(
                bridge
                  .state()
                  .project.entities.find((e: any) => e.id === selected)?.kind,
              )
                ? 0
                : (bridge.state().pose?.y ?? 0),
            }
          : {}),
      }),
    ),
  );
  el("resize-pumpkin").addEventListener("click", () =>
    attempt(() =>
      submit({
        kind: "resize_asset",
        id: selected ?? "",
        scale: el<HTMLInputElement>("scale").valueAsNumber,
      }),
    ),
  );
  el("remove-pumpkin").addEventListener("click", () =>
    attempt(() => submit({ kind: "remove_asset", id: selected ?? "" })),
  );
  const behavior = (mode: Behavior, all = false) =>
    attempt(() =>
      submit({
        kind: "set_behavior",
        behavior: mode,
        ids: all
          ? bridge
              .state()
              .project.entities.filter((e: any) => e.kind === "ghost")
              .map((e: any) => e.id)
          : [selected ?? ""],
      }),
    );
  el("float-ghost").addEventListener("click", () => behavior("float"));
  el("chase-ghost").addEventListener("click", () => behavior("chase"));
  el("stop-ghost").addEventListener("click", () => behavior("still"));
  el("float-ghosts").addEventListener("click", () => behavior("float", true));
  el("undo").addEventListener("click", () =>
    attempt(() => submit({ kind: "undo" })),
  );
  el("redo").addEventListener("click", () =>
    attempt(() => submit({ kind: "redo" })),
  );
  el("tint").addEventListener("change", () =>
    attempt(() =>
      submit({
        kind: "set_tint",
        tint: el<HTMLSelectElement>("tint").value as Tint,
      }),
    ),
  );
  el("selected").addEventListener("change", () => {
    cancel();
    selected = el<HTMLSelectElement>("selected").value;
    const entity = bridge
      .state()
      .project.entities.find((e: any) => e.id === selected);
    if (entity)
      for (const key of ["x", "z", "scale"])
        el<HTMLInputElement>(key).value = String(entity[key]);
    rendered = -1;
    refresh();
  });
  el("ask").addEventListener("click", () => {
    void interpret(el<HTMLTextAreaElement>("request").value).catch(() => {});
  });
  el("cancel-decision").addEventListener("click", () => cancel());
  return {
    receive,
    ingest,
    submit,
    interpret,
    cancel,
    refresh,
    setReady: (value: boolean) => {
      if (value && !initialized) {
        initialized = true;
        status("Ready. Place an asset or describe your changes.");
      }
      ready = value;
      refresh();
    },
    snapshot: () => ({
      receipts: [...receipts],
      decisions: [...decisions],
      selected,
      pending: !!decision,
    }),
  };
}
