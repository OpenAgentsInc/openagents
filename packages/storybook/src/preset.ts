/**
 * @since 1.0.0
 */

import type { PresetProperty } from "@storybook/types"
import type { StorybookConfig } from "./types.js"
import { renderToCanvas } from "./renderToCanvas.js"

/**
 * Default addons (none for now)
 * 
 * @since 1.0.0
 * @category Configuration
 */
export const addons: PresetProperty<"addons"> = []

/**
 * Core configuration for the Typed framework
 * 
 * @since 1.0.0
 * @category Configuration
 */
export const core: PresetProperty<"core", StorybookConfig> = {
  builder: "@storybook/builder-vite",
  renderer: "@openagentsinc/storybook"
}

/**
 * Preview annotations (pass-through for now)
 * 
 * @since 1.0.0
 * @category Configuration
 */
export const previewAnnotations: PresetProperty<"previewAnnotations"> = (
  entry: any[] = []
) => {
  return [...entry]
}

/**
 * Vite configuration hook
 * 
 * @since 1.0.0
 * @category Configuration
 */
export const viteFinal: NonNullable<StorybookConfig["viteFinal"]> = async (
  config,
  { configType }
) => {
  // Add any custom Vite configuration here
  return {
    ...config,
    // Example: Add path resolution for your workspace packages
    resolve: {
      ...config.resolve,
      alias: {
        ...config.resolve?.alias,
        // Add aliases for workspace packages if needed
      }
    }
  }
}

// Export the render function for Storybook
export { renderToCanvas }