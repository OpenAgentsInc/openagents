/**
 * Serving gate for the swap product surface (SWAP-7, #9322).
 *
 * Route naming and the gate shape are the owner decision carried by SWAP-0
 * (#9315). That decision is not yet recorded, so this gate uses the
 * repository's established fail-closed shape — the same
 * `configured === 'true'` env gate that serves `/demo`
 * (`OPENAGENTS_MARKET_DEMO_ENABLED`) and `/dh` — and stays OFF in
 * production until the owner flips it. If SWAP-0 records a different route
 * name or gate shape, this module is the single place to change.
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
