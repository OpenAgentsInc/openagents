import { defineConfig } from 'vite-plus'

// Match T3 Code's production pack pattern: owned workspace packages are part
// of the deployable artifact, never runtime dependencies of the slim image.
export const shouldBundleCloudRunDependency = (id: string): boolean =>
  id.startsWith('@openagentsinc/')

export default defineConfig({
  pack: {
    deps: {
      alwaysBundle: shouldBundleCloudRunDependency,
      neverBundle: ['cloudflare:workers', '@cloudflare/playwright'],
      onlyBundle: false,
    },
  },
})
