import { plugin } from "bun"
import { createRequire } from "node:module"
import { resolve } from "node:path"

const supportRoot = import.meta.dir
const aliases = new Map([
  ["@expo-google-fonts/jetbrains-mono", "expo-google-fonts-vitest.ts"],
  ["@expo-google-fonts/space-grotesk", "expo-google-fonts-vitest.ts"],
  ["@react-navigation/native", "react-navigation-native-vitest.ts"],
  ["@shopify/react-native-skia", "react-native-skia-vitest.tsx"],
  ["react-native", "react-native-vitest.tsx"],
  ["react-native-edge-to-edge", "react-native-edge-to-edge-vitest.tsx"],
  ["react-native-keyboard-controller", "react-native-keyboard-controller-vitest.tsx"],
  ["react-native-reanimated", "react-native-reanimated-vitest.tsx"],
  ["react-native-safe-area-context", "react-native-safe-area-context-vitest.tsx"],
])
const requireModule = createRequire(import.meta.url)
const escapePattern = (value: string): string =>
  value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")

plugin({
  name: "khala-mobile-vp3-bun-compat",
  setup(build) {
    for (const [specifier, target] of aliases) {
      const entry = requireModule.resolve(specifier)
      build.onLoad({ filter: new RegExp(`^${escapePattern(entry)}$`) }, () => ({
        contents: `export * from ${JSON.stringify(resolve(supportRoot, target))}`,
        loader: "js",
      }))
    }
  },
})

await import("./rn-vitest-setup")
