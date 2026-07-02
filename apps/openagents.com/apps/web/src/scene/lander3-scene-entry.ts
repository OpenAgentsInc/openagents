import {
  type LandingSquaresHandle,
  mountLandingSquares,
} from './landingSquares'

// Standalone entry for the `/lander3` async hero bundle
// (vite.lander3.config.ts builds it as a self-contained ES module at
// `/assets/lander3-scene.js`, Three.js included). The server-rendered page
// paints instantly with the CSS grid backdrop; this module is dynamically
// imported after `load` + idle, mounts the SAME landing-squares scene the SPA
// landing page uses, and resolves once the first scene frames have actually
// rendered so the page can fade the canvas in over the grid.
export const mountLander3Scene = (
  element: HTMLElement,
): Promise<LandingSquaresHandle> =>
  new Promise(resolve => {
    const handle = mountLandingSquares(element)
    // Two animation frames: one for the renderer's first paint to be
    // scheduled, one for it to have committed — so the fade-in never reveals
    // an empty black canvas.
    requestAnimationFrame(() => {
      requestAnimationFrame(() => resolve(handle))
    })
  })
