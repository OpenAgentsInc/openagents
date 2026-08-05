import { createServerFn } from '@tanstack/react-start'

import {
  SWAP_UI_GATE_ENV_VAR,
  swapSurfaceEnabled,
  type SwapSurfaceStatus,
} from './gate'

/**
 * The gate is read server-side for every document request so a production
 * deployment without the flag can never serve the swap surface, and so the
 * owner can disarm it on a live revision without a new container image.
 */
export const readSwapSurfaceStatus = createServerFn({ method: 'GET' }).handler(
  async (): Promise<SwapSurfaceStatus> => ({
    enabled: swapSurfaceEnabled(process.env[SWAP_UI_GATE_ENV_VAR]),
  }),
)
