import { Pressable, type PressableProps, StyleSheet, View } from "react-native"
import { useDrawerProgress } from "react-native-drawer-layout"

import { drawerButtonBars } from "./drawer-interp"

export type DrawerIconButtonProps = PressableProps & {
  textColor?: string
  tintColor?: string
}

const textColorDefault = "#111827"
const tintColorDefault = "#2563eb"

const colorForStop = (colorStop: number, textColor: string, tintColor: string): string => {
  return colorStop >= 0.5 ? tintColor : textColor
}

export function DrawerIconButton({
  textColor = textColorDefault,
  tintColor = tintColorDefault,
  style,
  ...pressableProps
}: DrawerIconButtonProps) {
  const progress = useDrawerProgress()
  const progressValue = typeof progress === "number" ? progress : progress.value
  const bars = drawerButtonBars(progressValue)
  const topBarColor = colorForStop(bars.topBar.colorStop, textColor, tintColor)
  const middleBarColor = colorForStop(bars.middleBar.colorStop, textColor, tintColor)
  const bottomBarColor = colorForStop(bars.bottomBar.colorStop, textColor, tintColor)

  return (
    <Pressable
      {...pressableProps}
      style={[
        styles.container,
        {
          transform: [{ translateX: bars.container.translateX }],
        },
        style,
      ]}
    >
      <View
        style={[
          styles.bar,
          {
            backgroundColor: topBarColor,
            marginBottom: bars.topBar.marginBottom,
            transform: [{ translateX: bars.topBar.translateX }, { rotate: `${bars.topBar.rotateDeg}deg` }],
            width: bars.topBar.width,
          },
        ]}
      />
      <View
        style={[
          styles.bar,
          styles.middleBar,
          {
            backgroundColor: middleBarColor,
            width: bars.middleBar.width,
          },
        ]}
      />
      <View
        style={[
          styles.bar,
          {
            backgroundColor: bottomBarColor,
            marginTop: bars.bottomBar.marginTop,
            transform: [{ translateX: bars.bottomBar.translateX }, { rotate: `${bars.bottomBar.rotateDeg}deg` }],
            width: bars.bottomBar.width,
          },
        ]}
      />
    </Pressable>
  )
}

const styles = StyleSheet.create({
  bar: {
    height: 2,
  },
  container: {
    alignItems: "center",
    height: 56,
    justifyContent: "center",
    width: 56,
  },
  middleBar: {
    marginTop: 4,
  },
})
