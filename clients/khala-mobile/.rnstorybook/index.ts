import { view } from "./storybook.requires"
import { AppRegistry } from "react-native"

const storybookStorage = new Map<string, string>()

const StorybookUIRoot = view.getStorybookUI({
  storage: {
    getItem: async (key) => storybookStorage.get(key) ?? null,
    setItem: async (key, value) => {
      storybookStorage.set(key, value)
    },
  },
})

AppRegistry.registerComponent("main", () => StorybookUIRoot)

export default StorybookUIRoot
