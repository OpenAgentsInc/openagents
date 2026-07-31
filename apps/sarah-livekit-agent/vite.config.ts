import { defineConfig } from "vite-plus";

export default defineConfig({
  pack: {
    dts: { eager: true },
    deps: {
      alwaysBundle: [/^@openagentsinc\//],
      onlyBundle: false,
      dts: { alwaysBundle: [/^@openagentsinc\//] },
    },
  },
});
