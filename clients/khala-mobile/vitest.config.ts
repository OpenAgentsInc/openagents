import { resolve } from "node:path"
import { defineConfig } from "vite-plus/test/config"

export default defineConfig({
  assetsInclude: ["**/*.otf"],
  plugins: [{
    name: "khala-mobile-font-fixture",
    enforce: "pre",
    transform(code, id) {
      if (!id.endsWith("/src/theme/typography.ts")) return
      return code.replace('require("../../assets/fonts/Protomolecule.otf")', "8")
    },
  }],
  resolve: {
    alias: {
      "@expo-google-fonts/jetbrains-mono": resolve(import.meta.dirname, "tests/support/expo-google-fonts-vitest.ts"),
      "@expo-google-fonts/space-grotesk": resolve(import.meta.dirname, "tests/support/expo-google-fonts-vitest.ts"),
      "@react-navigation/native": resolve(import.meta.dirname, "tests/support/react-navigation-native-vitest.ts"),
      "@shopify/react-native-skia": resolve(import.meta.dirname, "tests/support/react-native-skia-vitest.tsx"),
      "react-native": resolve(import.meta.dirname, "tests/support/react-native-vitest.tsx"),
      "react-native-edge-to-edge": resolve(import.meta.dirname, "tests/support/react-native-edge-to-edge-vitest.tsx"),
      "react-native-keyboard-controller": resolve(import.meta.dirname, "tests/support/react-native-keyboard-controller-vitest.tsx"),
      "react-native-reanimated": resolve(import.meta.dirname, "tests/support/react-native-reanimated-vitest.tsx"),
      "react-native-safe-area-context": resolve(import.meta.dirname, "tests/support/react-native-safe-area-context-vitest.tsx"),
    },
  },
  test: {
    environment: "node",
    include: ["tests/**/*.{test,spec}.{ts,tsx}"],
    setupFiles: [
      resolve(import.meta.dirname, "tests/support/rn-vitest-setup.ts"),
      resolve(import.meta.dirname, "../../scripts/vp3-vitest-setup.ts"),
    ],
  },
})
