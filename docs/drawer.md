# Drawer Implementation Guide (as used in this app)

This document explains exactly how this app implements a side drawer using `react-native-drawer-layout`, including all required packages, setup, platform considerations (iOS/Android/Web), and copy‑pasteable code. A coding agent can reproduce the drawer in a new codebase by following these steps alone.

---

## Overview

- Library: `react-native-drawer-layout` (not React Navigation’s DrawerNavigator). The drawer is composed manually and controlled via local state.
- Gesture handling: `react-native-gesture-handler` (required by drawer gestures on native).
- Animations: `react-native-reanimated` for the animated hamburger/close icon synced to drawer progress.
- Safe areas: `react-native-safe-area-context` to inset the drawer content appropriately.
- RTL: Drawer opens left/right depending on RTL, and the toggle icon mirrors accordingly.
- Web: No `react-native-gesture-handler` import on web; a `setimmediate` polyfill is used to avoid a known issue.

In this app, the drawer appears on the Demo Showroom screen and wraps the screen’s content. The animated “hamburger → close” button uses the drawer’s open/close progress to interpolate transforms and colors.

Key reference implementations in this repo:
- `app/screens/DemoShowroomScreen/DemoShowroomScreen.tsx` (wraps content with `<Drawer/>`, controls open state)
- `app/screens/DemoShowroomScreen/DrawerIconButton.tsx` (animated toggle linked to drawer progress)
- `app/utils/gestureHandler.native.ts` and `app/utils/gestureHandler.ts` (platform‑specific imports)

---

## Dependencies

Install these packages. For Expo:

- `npx expo install react-native-drawer-layout react-native-gesture-handler react-native-reanimated react-native-safe-area-context setimmediate`

For bare React Native:

- `yarn add react-native-drawer-layout react-native-gesture-handler react-native-reanimated react-native-safe-area-context setimmediate`
- iOS: run `cd ios && pod install`

Notes:
- Reanimated typically requires its Babel plugin. Add `"react-native-reanimated/plugin"` last in the plugin list of your `babel.config.js` if your template doesn’t already provide it.
- Hermes/new architecture are supported by default in modern RN/Expo versions.

Example `babel.config.js` snippet (keep your existing config, this shows the plugin placement):

```js
module.exports = function (api) {
  api.cache(true)
  return {
    presets: ["babel-preset-expo"],
    plugins: [
      // other plugins
      "react-native-reanimated/plugin",
    ],
  }
}
```

---

## Platform bootstrap (must-do)

To avoid importing `react-native-gesture-handler` on web and to include a web polyfill, replicate this exact pattern:

Create `utils/gestureHandler.native.ts`:

```ts
// Only import react-native-gesture-handler on native platforms
// https://reactnavigation.org/docs/drawer-navigator/#installation
import "react-native-gesture-handler"
```

Create `utils/gestureHandler.ts`:

```ts
// Don't import react-native-gesture-handler on web
// https://reactnavigation.org/docs/drawer-navigator/#installation

// This however is needed at the moment
// https://github.com/software-mansion/react-native-gesture-handler/issues/2402
import "setimmediate"
```

Then import this once, very early in your app entry (before any navigation or drawer usage). In this repo we do it at the top of `app/app.tsx`:

```ts
import "./utils/gestureHandler"
```

This ensures proper gesture setup on native and avoids issues on web.

---

## Minimal drawer usage (structure and props)

The drawer is created with `<Drawer>` from `react-native-drawer-layout`, controlled by local `open` state, and renders custom drawer content via `renderDrawerContent`.

Core props we use:
- `open`: boolean that controls visibility.
- `onOpen` / `onClose`: synchronize local state with drawer gestures.
- `drawerType="back"`: drawer slides under the content.
- `drawerPosition`: left on LTR, right on RTL.
- `renderDrawerContent`: the view hierarchy shown inside the drawer.

A distilled, copy‑pasteable screen example:

```tsx
import { useCallback, useRef, useState } from "react"
import { View, Text, Image, FlatList, Platform } from "react-native"
import { Drawer } from "react-native-drawer-layout"
import { SafeAreaView, useSafeAreaInsets } from "react-native-safe-area-context"
import { I18nManager } from "react-native"

// Optional: your own icon/button component. See the full example below.
import { DrawerIconButton } from "./DrawerIconButton"

const isRTL = I18nManager.isRTL

export function ExampleDrawerScreen() {
  const [open, setOpen] = useState(false)
  const insets = useSafeAreaInsets()

  const toggleDrawer = useCallback(() => setOpen((v) => !v), [])

  return (
    <Drawer
      open={open}
      onOpen={() => setOpen(true)}
      onClose={() => setOpen(false)}
      drawerType="back"
      drawerPosition={isRTL ? "right" : "left"}
      renderDrawerContent={() => (
        <SafeAreaView style={{ flex: 1, paddingTop: insets.top }}>
          <View style={{ height: 56, justifyContent: "center", paddingHorizontal: 16 }}>
            {/* Replace with your branding */}
            <Text style={{ fontSize: 18, fontWeight: "600" }}>Menu</Text>
          </View>

          <FlatList
            data={[{ name: "Section 1" }, { name: "Section 2" }]}
            keyExtractor={(item) => item.name}
            renderItem={({ item }) => (
              <Text style={{ paddingHorizontal: 16, paddingVertical: 8 }}>{item.name}</Text>
            )}
            contentContainerStyle={{ paddingHorizontal: 16 }}
          />
        </SafeAreaView>
      )}
    >
      {/* Main content area */}
      <SafeAreaView style={{ flex: 1 }}>
        <View style={{ height: 56, justifyContent: "center" }}>
          <DrawerIconButton onPress={toggleDrawer} />
        </View>

        {/* Replace with your screen content */}
        <View style={{ flex: 1, padding: 16 }}>
          <Text style={{ fontSize: 24, fontWeight: "700", marginBottom: 12 }}>
            Hello Drawer
          </Text>
          <Text>Content goes here…</Text>
        </View>
      </SafeAreaView>
    </Drawer>
  )
}
```

This mirrors the structure and props used in the app’s screen (`app/screens/DemoShowroomScreen/DemoShowroomScreen.tsx`).

---

## Animated toggle button (hamburger ↔ close)

We expose drawer progress to an animated icon using `useDrawerProgress()` from `react-native-drawer-layout` and `react-native-reanimated` for transforms and color interpolation. This keeps the icon in sync with the drawer open state, even when opened via swipe gestures.

Copy‑pasteable version (self‑contained, no theme dependencies):

```tsx
import { Pressable, PressableProps, Platform, I18nManager, ViewStyle } from "react-native"
import Animated, { interpolate, interpolateColor, useAnimatedStyle } from "react-native-reanimated"
import { useDrawerProgress } from "react-native-drawer-layout"

const AnimatedPressable = Animated.createAnimatedComponent(Pressable)

export function DrawerIconButton(props: PressableProps) {
  const progress = useDrawerProgress()
  const isRTL = I18nManager.isRTL
  const isWeb = Platform.OS === "web"

  // Tweak these to match your theme
  const colors = { text: "#1A1A1A", tint: "#5C6EF8" }

  const container = useAnimatedStyle(() => ({
    transform: [{ translateX: interpolate(progress.value, [0, 1], [0, isRTL ? 60 : -60]) }],
  }))

  const topBar = useAnimatedStyle(() => {
    const marginStart = interpolate(progress.value, [0, 1], [0, -11.5])
    const rotate = interpolate(progress.value, [0, 1], [0, isRTL ? 45 : -45])
    return {
      backgroundColor: interpolateColor(progress.value, [0, 1], [colors.text, colors.tint]),
      width: interpolate(progress.value, [0, 1], [18, 12]),
      marginBottom: interpolate(progress.value, [0, 1], [0, -2]),
      ...(isWeb && isRTL ? { marginRight: marginStart } : { marginLeft: marginStart }),
      transform: [{ rotate: `${rotate}deg` }],
    }
  })

  const middleBar = useAnimatedStyle(() => ({
    backgroundColor: interpolateColor(progress.value, [0, 1], [colors.text, colors.tint]),
    width: interpolate(progress.value, [0, 1], [18, 16]),
  }))

  const bottomBar = useAnimatedStyle(() => {
    const marginStart = interpolate(progress.value, [0, 1], [0, -11.5])
    const rotate = interpolate(progress.value, [0, 1], [0, isRTL ? -45 : 45])
    return {
      backgroundColor: interpolateColor(progress.value, [0, 1], [colors.text, colors.tint]),
      width: interpolate(progress.value, [0, 1], [18, 12]),
      marginTop: interpolate(progress.value, [0, 1], [4, 2]),
      ...(isWeb && isRTL ? { marginRight: marginStart } : { marginLeft: marginStart }),
      transform: [{ rotate: `${rotate}deg` }],
    }
  })

  return (
    <AnimatedPressable accessibilityRole="button" {...props} style={[container, $container]}> 
      <Animated.View style={[$bar, topBar]} />
      <Animated.View style={[$bar, { marginTop: 4 }, middleBar]} />
      <Animated.View style={[$bar, bottomBar]} />
    </AnimatedPressable>
  )
}

const $container: ViewStyle = { alignItems: "center", justifyContent: "center", width: 56, height: 56 }
const $bar: ViewStyle = { height: 2 }
```

This is adapted directly from `app/screens/DemoShowroomScreen/DrawerIconButton.tsx` and will work without any theme context.

---

## Safe area handling

In the app, drawer content is padded using a helper hook that maps safe‑area insets onto style props. You can simply use `react-native-safe-area-context` directly:

```tsx
import { SafeAreaView, useSafeAreaInsets } from "react-native-safe-area-context"

const insets = useSafeAreaInsets()
// Example: <SafeAreaView style={{ flex: 1, paddingTop: insets.top }}> ...
```

If you want the same helper used by this app, see `app/utils/useSafeAreaInsetsStyle.ts`.

---

## RTL behavior

- Drawer side is computed with `drawerPosition={isRTL ? "right" : "left"}`.
- The toggle icon mirrors rotations and margins when `isRTL` is true.
- In this app `isRTL` comes from i18n utilities; in a blank project, use `I18nManager.isRTL` from `react-native`.

---

## Web specifics

- Do not import `react-native-gesture-handler` on web (causes issues). The platform split files above ensure this.
- Import `setimmediate` once to avoid a known web issue with RNGH. This app imports it in `utils/gestureHandler.ts`.
- The animated button applies slightly different margins on web for better cross‑platform consistency (see `isWeb && isRTL` checks).

---

## Putting it all together (checklist)

1) Install packages
- Expo: `npx expo install react-native-drawer-layout react-native-gesture-handler react-native-reanimated react-native-safe-area-context setimmediate`
- Bare RN: `yarn add ...` then `cd ios && pod install`

2) Configure Babel (if needed)
- Ensure `"react-native-reanimated/plugin"` is the last plugin in `babel.config.js`.

3) Platform bootstrap
- Add `utils/gestureHandler.native.ts` with `import "react-native-gesture-handler"`.
- Add `utils/gestureHandler.ts` with `import "setimmediate"`.
- Import `"./utils/gestureHandler"` at the very top of your app entry (before any navigators or drawer usage).

4) Build the screen
- Wrap your content in `<Drawer>` as shown above.
- Keep drawer state in your component and wire up `open`, `onOpen`, and `onClose`.
- Provide `renderDrawerContent` with your menu UI.
- Place `<DrawerIconButton onPress={toggleDrawer} />` in the header area.

5) Verify on all platforms
- iOS/Android: swipe from the drawer edge; the toggle animates with the drawer.
- Web: click the toggle; no RNGH import errors; drawer renders properly.

---

## Troubleshooting

- "Reanimated plugin not found" or worklets not running:
  - Ensure `"react-native-reanimated/plugin"` is in `babel.config.js`.
  - Clear caches: `expo start -c` or `watchman watch-del-all && rm -rf node_modules && yarn && yarn start -c`.
- Crashes or red box related to `react-native-gesture-handler` on web:
  - Confirm you’re importing `./utils/gestureHandler` (not the native import directly) and that the files are split with the `.native.ts` suffix.
- Drawer won’t swipe on native:
  - Verify `react-native-gesture-handler` is installed and the native bootstrap file is being executed (place a console.log in `utils/gestureHandler.native.ts` during debugging).
- RTL direction looks wrong:
  - Ensure `drawerPosition` and the toggle icon logic both respect `I18nManager.isRTL`.

---

## How this maps to the repo

- Drawer container and props: `app/screens/DemoShowroomScreen/DemoShowroomScreen.tsx` (see `Drawer` usage)
- Animated toggle: `app/screens/DemoShowroomScreen/DrawerIconButton.tsx`
- Gesture/web bootstrap: `app/utils/gestureHandler.native.ts` and `app/utils/gestureHandler.ts`
- Safe area helper (optional): `app/utils/useSafeAreaInsetsStyle.ts`

Following this guide reproduces the exact drawer behavior from this app in a new project.

