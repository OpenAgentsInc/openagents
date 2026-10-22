/** Separate lab identities share the preview runtime, never a saved project. */
const path = typeof location === "undefined" ? "" : location.pathname;
const search = typeof location === "undefined" ? "" : location.search;
export const IS_TOWN =
  (typeof document === "undefined" &&
    typeof self !== "undefined" &&
    (self as DedicatedWorkerGlobalScope).name === "medieval") ||
  path.startsWith("/medieval") ||
  new URLSearchParams(search).get("lab") === "medieval";
export const LAB = IS_TOWN
  ? ({
      id: "medieval-town-v1",
      scene: "medieval/scene.json",
      material: "medieval/material.wgsl",
      shader: "/medieval/material.wgsl",
      decision: "/api/medieval/decision",
      name: "Medieval Town Lab",
    } as const)
  : ({
      id: "haunt-webgpu-v1",
      scene: "haunt/scene.json",
      material: "haunt/material.wgsl",
      shader: "/material.wgsl",
      decision: "/api/haunt/decision",
      name: "The Haunted House",
    } as const);
