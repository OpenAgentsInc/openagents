/**
 * Serving gate for the swap product surface (SWAP-7, #9322).
 *
 * The SWAP-0 (#9315) owner decision records `/swap` behind
 * `OPENAGENTS_SWAP_UI_ENABLED`, using the repository's established exact
 * `configured === 'true'` shape. Activation remains a separate deployment
 * decision after the live Offerings/Quotes relay subscription and wasm engine
 * binding are wired and the mounted-surface evidence is green.
 *
 * Fail closed: only the exact string 'true' enables the surface. Anything
 * else — unset, empty, 'TRUE', '1', 'yes' — leaves it disabled.
 */
export const SWAP_UI_GATE_ENV_VAR = 'OPENAGENTS_SWAP_UI_ENABLED'

export const swapSurfaceEnabled = (configured: string | undefined): boolean =>
  configured === 'true'

export type SwapSurfaceStatus = Readonly<{
  enabled: boolean
}>
