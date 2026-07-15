import { defineConfig } from "vite-plus";

// Match T3 Code's production pack pattern: owned workspace packages are part
// of the deployable artifact instead of becoming accidental runtime edges.
export const shouldBundleCloudRunDependency = (id: string): boolean =>
  id.startsWith("@openagentsinc/") || id === "nostr-effect" || id.startsWith("nostr-effect/");

export default defineConfig({
  pack: {
    deps: {
      alwaysBundle: shouldBundleCloudRunDependency,
      onlyBundle: false,
    },
  },
});
