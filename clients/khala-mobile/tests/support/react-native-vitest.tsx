import React from "react"

const host = (name: string) => React.forwardRef<unknown, Record<string, unknown>>(
  ({ children, ...props }, ref) =>
    React.createElement(name, { ...props, ref }, children as React.ReactNode),
)

export const ActivityIndicator = host("ActivityIndicator")
export const Image = Object.assign(host("Image"), { resolveAssetSource: (source: unknown) => source })
export const ImageBackground = host("ImageBackground")
export const KeyboardAvoidingView = host("KeyboardAvoidingView")
export const Modal = host("Modal")
export const Pressable = React.forwardRef<unknown, Record<string, any>>(
  ({ children, ...props }, ref) => React.createElement(
    "Pressable",
    { ...props, ref },
    typeof children === "function" ? children({ pressed: false }) : children,
  ),
)
export const SafeAreaView = host("SafeAreaView")
export const ScrollView = host("ScrollView")
export const StatusBar = host("StatusBar")
export const Text = host("Text")
export const TextInput = Object.assign(host("TextInput"), {
  State: { currentlyFocusedInput: () => null },
})
export const TouchableOpacity = host("TouchableOpacity")
export const View = host("View")

export const FlatList = React.forwardRef<unknown, Record<string, any>>(
  ({ data = [], renderItem, keyExtractor, ...props }, ref) => React.createElement(
    "FlatList",
    { ...props, ref },
    data.map((item: unknown, index: number) => React.createElement(
      React.Fragment,
      { key: keyExtractor?.(item, index) ?? index },
      renderItem?.({ item, index, separators: {} }),
    )),
  ),
)

export const StyleSheet = {
  absoluteFill: { bottom: 0, left: 0, position: "absolute", right: 0, top: 0 },
  absoluteFillObject: { bottom: 0, left: 0, position: "absolute", right: 0, top: 0 },
  compose: (a: unknown, b: unknown) => [a, b],
  create: <T extends Record<string, unknown>>(styles: T): T => styles,
  flatten: (style: unknown) => Array.isArray(style) ? Object.assign({}, ...style.filter(Boolean)) : style,
  hairlineWidth: 1,
}
export const Platform = {
  OS: "ios",
  Version: "18.0",
  isPad: false,
  isTV: false,
  select: <T,>(options: Record<string, T>): T | undefined => options.ios ?? options.native ?? options.default,
}
export const Linking = {
  addEventListener: () => ({ remove() {} }),
  canOpenURL: async () => true,
  getInitialURL: async () => null,
  openSettings: async () => undefined,
  openURL: async () => undefined,
}
export const AppState = { addEventListener: () => ({ remove() {} }), currentState: "active" }
export const Appearance = {
  addChangeListener: () => ({ remove() {} }),
  getColorScheme: () => "dark",
  setColorScheme() {},
}
export const BackHandler = { addEventListener: () => ({ remove() {} }), exitApp() {} }
export const Keyboard = { addListener: () => ({ remove() {} }), dismiss() {} }
export const LogBox = { ignoreAllLogs() {}, ignoreLogs() {} }
export const TurboModuleRegistry = {
  get: () => null,
  getEnforcing: () => ({}),
}
export const useColorScheme = () => "dark"
export const useWindowDimensions = () => ({ fontScale: 1, height: 844, scale: 3, width: 390 })
