import type { NpcLabel, TalkIntent } from "../../packages/runtime/npc.ts";

/**
 * Name labels and speech bubbles over the world's characters. The worker
 * projects each character's head into canvas fractions; this only places
 * them. Labels hide when a wall stands between the eye and the character.
 */
export class NpcOverlay {
  private readonly root: HTMLDivElement;
  private readonly items = new Map<string, HTMLDivElement>();
  constructor(
    host: HTMLElement,
    private readonly ask: (intent: TalkIntent, npc: string) => void,
  ) {
    this.root = document.createElement("div");
    this.root.className = "npc-overlay";
    this.root.setAttribute("aria-live", "polite");
    host.append(this.root);
  }
  update(labels: readonly NpcLabel[]) {
    const seen = new Set<string>();
    for (const label of labels) {
      seen.add(label.id);
      let item = this.items.get(label.id);
      if (!item) {
        item = this.create(label);
        this.items.set(label.id, item);
        this.root.append(item);
      }
      item.style.left = `${(label.x * 100).toFixed(2)}%`;
      item.style.top = `${(label.y * 100).toFixed(2)}%`;
      item.toggleAttribute("data-nearby", label.nearby);
      const bubble = item.querySelector<HTMLElement>(".npc-line")!;
      const text = label.line?.text ?? "";
      if (bubble.textContent !== text) bubble.textContent = text;
      bubble.hidden = !text;
      bubble.dataset.kind = label.line?.kind ?? "";
      item.querySelector<HTMLElement>(".npc-actions")!.hidden = !label.nearby;
    }
    for (const [id, item] of this.items)
      if (!seen.has(id)) {
        item.remove();
        this.items.delete(id);
      }
  }
  clear() {
    this.update([]);
  }
  private create(label: NpcLabel) {
    const item = document.createElement("div");
    item.className = "npc-label";
    const line = document.createElement("p");
    line.className = "npc-line";
    line.hidden = true;
    const name = document.createElement("span");
    name.className = "npc-name";
    name.textContent = label.name;
    const actions = document.createElement("div");
    actions.className = "npc-actions";
    actions.hidden = true;
    for (const [intent, text] of [
      ["reply", "Talk (E)"],
      ["hint", "Hint (H)"],
      ["emote", "Wave"],
    ] as const) {
      const button = document.createElement("button");
      button.type = "button";
      button.textContent = text;
      button.addEventListener("click", (event) => {
        event.stopPropagation();
        this.ask(intent, label.id);
      });
      button.addEventListener("pointerdown", (event) =>
        event.stopPropagation(),
      );
      actions.append(button);
    }
    item.append(line, name, actions);
    return item;
  }
}
