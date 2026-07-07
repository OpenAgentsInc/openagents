import { view } from "./storybook.requires"
import { AppRegistry, LogBox } from "react-native"

LogBox.ignoreLogs([
  "[react-native-gesture-handler] None of the callbacks in the gesture are worklets",
])

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
