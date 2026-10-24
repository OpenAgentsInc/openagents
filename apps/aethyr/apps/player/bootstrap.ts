import {
  verifyPlayerArtifact,
  type PlayerProfile,
} from "../../packages/runtime/player-artifact.ts";

export async function playerArtifact(profile: PlayerProfile) {
  const started = performance.now();
  const response = await fetch("/player-artifact.json", { cache: "no-store" });
  if (!response.ok)
    throw new Error("This world could not open. Reload to try again.");
  const text = await response.text();
  if (new TextEncoder().encode(text).length > 16 * 1024 * 1024)
    throw new Error("This world is too large to open.");
  const artifact = await verifyPlayerArtifact(JSON.parse(text), { profile });
  const runtime = await fetch(artifact.runtime.url);
  const bytes = await runtime.arrayBuffer();
  const digest = [
    ...new Uint8Array(await crypto.subtle.digest("SHA-256", bytes)),
  ]
    .map((n) => n.toString(16).padStart(2, "0"))
    .join("");
  if (
    !runtime.ok ||
    bytes.byteLength !== artifact.runtime.bytes ||
    digest !== artifact.runtime.sha256
  )
    throw new Error("This world needs its matching player build.");
  return { artifact, descriptorAndRuntimeCheckMs: performance.now() - started };
}

export function playerChrome() {
  const canvas = document.querySelector<HTMLCanvasElement>("#world")!;
  const status = document.querySelector<HTMLOutputElement>("#status")!;
  const controls = document.querySelector<HTMLElement>("#controls")!;
  const message = (text: string) => {
    status.textContent = text;
  };
  const button = (
    text: string,
    title: string,
    action: () => void | Promise<void>,
  ) => {
    const item = document.createElement("button");
    item.textContent = text;
    item.title = title;
    item.addEventListener("click", () =>
      Promise.resolve()
        .then(action)
        .catch((e) => message(e instanceof Error ? e.message : String(e))),
    );
    controls.append(item);
    return item;
  };
  return { canvas, controls, message, button };
}
