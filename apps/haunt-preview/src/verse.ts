import history from "./world-history.json" with { type: "json" };
import { readChanges, describeChange, type Change } from "./changes.ts";
import type { Operation } from "./project.ts";

const poseKey = "openagents.verse.house.return";
export function createVerseWorkspace(hooks: {
  stop: () => void;
  state: () => any;
  post: (message: object) => void;
  workspace: <T>(job: () => Promise<T>) => Promise<T>;
}) {
  const el = <T extends HTMLElement>(id: string) =>
    document.getElementById(id) as T;
  const workspace = el<HTMLDialogElement>("edit-workspace");
  const signin = el<HTMLDialogElement>("edit-signin");
  // This Rust mount serves the local controls. The standalone lab's optional
  // decision adapter is not mounted here, so do not offer an unavailable action.
  for (const selector of [
    "label[for=request]",
    "#request",
    "#ask",
    ".agent details",
  ])
    document.querySelector<HTMLElement>(selector)!.hidden = true;
  document.querySelector(".agent h3")!.textContent = "Start with one change";
  const guidance = document.createElement("p");
  guidance.textContent =
    "Choose an object, adjust the lantern light, or open the material editor. Scene edits save as you go.";
  document.querySelector(".agent h3")!.after(guidance);
  const meta = (name: string) =>
    document.querySelector<HTMLMetaElement>(`meta[name="${name}"]`)?.content;
  const member = meta("verse-member") === "true";
  const author = meta("verse-author") || "Signed-in creator";
  const login = el<HTMLAnchorElement>("edit-login");
  login.hidden = meta("verse-login") !== "true";
  el("login-unavailable").hidden = !login.hidden;
  let changes: Change[] = [];
  let readable = false;
  let logError: string | undefined;
  const remember = () => {
    hooks.stop();
    try {
      sessionStorage.setItem(
        poseKey,
        JSON.stringify({
          pose: hooks.state().pose,
          request: el<HTMLTextAreaElement>("request").value,
          shader: el<HTMLTextAreaElement>("shader").value,
          editing: workspace.open,
        }),
      );
    } catch {
      /* Return context is optional when storage is unavailable. */
    }
  };
  const render = () => {
    const rows = el<HTMLTableSectionElement>("change-rows");
    rows.replaceChildren();
    for (const change of [...changes].reverse().concat(history as Change[])) {
      const row = document.createElement("tr");
      const who = document.createElement("td");
      const name = document.createElement("strong");
      name.textContent = change.author;
      const when = document.createElement("time");
      when.dateTime = change.at;
      when.textContent = new Date(change.at).toLocaleString(undefined, {
        month: "short",
        day: "numeric",
        hour: "numeric",
        minute: "2-digit",
      });
      who.append(name, when);
      const what = document.createElement("td");
      const text = change.url
        ? document.createElement("a")
        : document.createElement("span");
      text.textContent = change.summary;
      if (text instanceof HTMLAnchorElement) {
        text.href = change.url!;
        text.target = "_blank";
        text.rel = "noopener noreferrer";
      }
      const scope = document.createElement("small");
      scope.textContent =
        change.scope === "world"
          ? "World update"
          : `Saved edit · version ${change.revision ?? "—"}`;
      what.append(text, scope);
      const earned = document.createElement("td");
      earned.textContent = "Not recorded";
      row.append(who, what, earned);
      rows.append(row);
    }
    el("changes-status").textContent =
      logError ??
      `${history.length} world updates · ${changes.length} saved edits`;
  };
  const close = () => {
    workspace.close();
    el<HTMLCanvasElement>("house").focus();
  };
  el("open-edit").addEventListener("click", () => {
    hooks.stop();
    if (document.pointerLockElement) document.exitPointerLock();
    workspace.showModal();
    render();
  });
  el("close-edit").addEventListener("click", close);
  workspace.addEventListener("cancel", (event) => {
    event.preventDefault();
    close();
  });
  el("keep-exploring").addEventListener("click", () => signin.close());
  login.addEventListener("click", remember);
  addEventListener("pagehide", remember, { once: true });
  render();
  return {
    authorize() {
      if (member) return true;
      hooks.stop();
      if (!signin.open) signin.showModal();
      return false;
    },
    async ready() {
      try {
        if (typeof window.kilnReadChanges !== "function")
          throw new Error("Missing change-log reader");
        changes = readChanges(window.kilnReadChanges());
        readable = true;
        render();
      } catch (error) {
        console.warn("Change log:", error);
        logError =
          "Your saved changes couldn't be opened. Reload to try again.";
        render();
      }
      try {
        const previous = JSON.parse(sessionStorage.getItem(poseKey) || "null");
        if (previous?.pose)
          hooks.post({ type: "restore-pose", pose: previous.pose });
        if (typeof previous?.request === "string")
          el<HTMLTextAreaElement>("request").value = previous.request;
        if (typeof previous?.shader === "string")
          el<HTMLTextAreaElement>("shader").value = previous.shader;
        if (previous?.editing && member) workspace.showModal();
      } catch {
        /* A fresh visit starts at the house entrance. */
      }
    },
    async record(operation: Operation, revision: number) {
      if (!member || !readable) return;
      const change: Change = {
        id: crypto.randomUUID(),
        author,
        at: new Date().toISOString(),
        summary: describeChange(operation),
        revision,
        scope: "draft",
        earnings: null,
      };
      try {
        await hooks.workspace(async () => {
          const next = changes.concat(change).slice(-64);
          const code = await window.kilnWrite(
            "haunt/changes.json",
            JSON.stringify({ schema: 1, changes: next }),
          );
          if (code !== 0) throw new Error("Change log write failed");
          changes = next;
        });
        logError = undefined;
      } catch (error) {
        console.warn("Change log:", error);
        logError =
          "Your scene was saved, but the change log couldn't be saved. Reload to try again.";
      }
      render();
    },
  };
}
