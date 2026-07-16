/**
 * `@effect-native/render-canvas`
 *
 * A typed scene-descriptor renderer for 3D / data-viz surfaces, with frame
 * scheduling and resource lifetimes on Effect `Scope`/`Stream`. It sits under
 * the same component contract as the DOM/RN renderers:
 *
 *  - a closed, typed scene-descriptor catalog (Effect Schema), local to this
 *    package (`scene.ts`);
 *  - a pure, inspectable reconciler that diffs typed scene trees into a minimal
 *    op list (`reconciler.ts`);
 *  - a backend scene-graph interface + op application (`backend.ts`);
 *  - a `Stream`-driven frame loop with `Scope`-owned disposal (`frame.ts`);
 *  - a headless recording backend for GPU-free snapshot tests (`headless.ts`);
 *  - a live Three.js backend + injectable graph port (`three-backend.ts`).
 */

export const packageName = "@effect-native/render-canvas" as const

export * from "./scene"
export * from "./reconciler"
export * from "./backend"
export * from "./frame"
export * from "./headless"
export * from "./three-backend"
export * from "./graph-figure"
export * from "./khala-background"
export { createSceneNodeReconciler, createSceneResourceScope } from "./scene-node-reconciler"
