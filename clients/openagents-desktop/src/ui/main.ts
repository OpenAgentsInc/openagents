import { mountLandingSquares } from "./landing-squares"
import "./styles.css"

const scene = document.querySelector<HTMLElement>("#openagents-scene")
if (scene === null) {
  throw new Error("Missing #openagents-scene mount")
}

const prefersReducedMotion = globalThis.matchMedia?.(
  "(prefers-reduced-motion: reduce)",
).matches ?? false

const handle = mountLandingSquares(scene, {
  animate: !prefersReducedMotion,
  pose: "landing",
})

globalThis.addEventListener("pagehide", () => {
  handle.dispose()
})
