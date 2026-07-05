import { Blur, Canvas, Image, makeImageFromView, rect } from "@shopify/react-native-skia"
import type { SkImage, SkSize } from "@shopify/react-native-skia"
import type { ReactNode } from "react"
import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import type { ViewProps, ViewStyle } from "react-native"
import { StyleSheet, Text, TouchableOpacity, View } from "react-native"
import Animated, {
  runOnJS,
  useAnimatedProps,
  useAnimatedReaction,
  useAnimatedStyle,
  useDerivedValue,
  useSharedValue,
  withTiming,
  type MeasuredDimensions
} from "react-native-reanimated"

import { BlurredPopupContext, type PopupAlignment, type PopupOptionType } from "./blurred-context"

/** Ported from Arcade's `BlurredPopupProvider`
 * (`app/components/BlurredPopup/BlurredPopupProvider.tsx`, see
 * `docs/design/2026-07-05-arcade-ui-harvest-audit.md` §2.5 and issue #8395).
 *
 * On `showPopup`, freezes a full-screen Skia screenshot of `mainView`
 * (`makeImageFromView`), renders it under TWO layered `Blur` filters at
 * different intensities — a subtle 1/3-strength base blur plus the full
 * blur, which reads noticeably better than a single pass — then re-renders
 * the long-pressed node at its exact original screen position on top
 * (non-blurred, so it "pops forward"), with a context menu that
 * auto-positions itself from the available screen space around that node.
 *
 * Dismissal is choreographed: the menu fades first (`menuVisible.value =
 * false`), then 200ms later the blur animates back to 0
 * (`dismissBlurredPopup`), and only once `useAnimatedReaction` observes the
 * blur hitting exactly 0 does it `runOnJS` unmount the whole overlay
 * (`resetParams`) — animate-out-then-unmount, not unmount-then-hope.
 *
 * Per #8390/#8392/#8399/#8402's finding, the Skia version pinned in this
 * repo (2.6.2) reads Reanimated `SharedValue`/`DerivedValue` props directly
 * (including `Canvas`'s own `onSize`, which is now a plain
 * `SharedValue<SkSize>` instead of Arcade's Skia-native `useValue`), so
 * there's no `useSharedValueEffect`-style bridge anywhere in this port.
 *
 * Deviation from Arcade: menu chrome colors are fixed NativeWind tokens
 * (`surfaceRaised`/`border`/`text`/`accent`, `../../theme/tokens` via
 * Tailwind classes) instead of a caller-overridable
 * `backgroundColor`/`titleColor` pair — every real call site in this app
 * wants the one recolored StarCraft-chrome look the harvest issue asks for,
 * not an arbitrary per-caller palette. `listItemHeight` stays configurable
 * since it's a layout number, not a color. Each menu item also grows a
 * small `accent`-colored leading marker (matching the Frame corner-square
 * look already established by `../frame`) when no explicit `leading` node is
 * supplied. */

type MenuLayout = Readonly<{ listItemHeight?: number }>

export type BlurredPopupProviderProps = Readonly<{
  children?: ReactNode
  menuLayout?: MenuLayout
  maxBlur?: number
}>

const DEFAULT_LIST_ITEM_HEIGHT = 50

type PopupParams = Readonly<{
  image: SkImage
  node: ReactNode
  layout: MeasuredDimensions
  options: ReadonlyArray<PopupOptionType>
}>

export const BlurredPopupProvider = ({ children, maxBlur = 10, menuLayout }: BlurredPopupProviderProps) => {
  const [params, setParams] = useState<PopupParams | undefined>(undefined)

  const menuVisible = useSharedValue(false)
  const menuOpacity = useDerivedValue(() => withTiming(menuVisible.value ? 1 : 0))

  const image = params?.image
  const options = useMemo(() => params?.options ?? [], [params])

  // Screenshot target: everything rendered behind the popup, including the
  // rest of the app (`children`). Plain `useRef`, not `useAnimatedRef` —
  // `makeImageFromView` takes a normal React ref.
  const mainView = useRef<View>(null)

  const showPopup = useCallback(
    async ({
      layout,
      node,
      options: popupOptions
    }: {
      layout: MeasuredDimensions
      node: ReactNode
      options: ReadonlyArray<PopupOptionType>
    }) => {
      const skImage = await makeImageFromView(mainView)
      if (skImage === null) return
      setParams({ image: skImage, layout, node, options: popupOptions })
      menuVisible.value = true
    },
    [menuVisible]
  )

  // `SkSize` updated by `Canvas`'s own `onSize` prop below — used to
  // auto-position the popup menu against available screen space.
  const canvasSize = useSharedValue<SkSize>({ height: 0, width: 0 })

  const rBlur = useSharedValue(0)
  const sLightBlurValue = useDerivedValue(() => rBlur.value / 3)

  const dismissBlurredPopup = useCallback(() => {
    rBlur.value = withTiming(0, { duration: 200 })
  }, [rBlur])

  const resetParams = useCallback(() => setParams(undefined), [])

  // Once the blur finishes animating back to 0, unmount the overlay —
  // animate-out-then-unmount, never the reverse.
  useAnimatedReaction(
    () => rBlur.value,
    (value, previousValue) => {
      if (value === 0 && previousValue !== null && previousValue > value) runOnJS(resetParams)()
    }
  )

  const close = useCallback(() => {
    menuVisible.value = false
    setTimeout(() => dismissBlurredPopup(), 200)
  }, [dismissBlurredPopup, menuVisible])

  useEffect(() => {
    if (image === undefined) return
    rBlur.value = withTiming(maxBlur, { duration: 200 })
  }, [image, maxBlur, rBlur])

  const imageRect = useDerivedValue(() => rect(0, 0, canvasSize.value.width, canvasSize.value.height))

  const nodeStyle = useMemo((): ViewStyle => {
    if (params === undefined) return { opacity: 0 }
    const { height, pageX, pageY, width } = params.layout
    return { height, left: pageX, opacity: 1, position: "absolute", top: pageY, width }
  }, [params])

  const hasParams = params !== undefined
  const menuAnimatedProps = useAnimatedProps(
    () => ({ pointerEvents: hasParams ? "auto" : "none" }) as Partial<ViewProps>,
    [hasParams]
  )

  const canvasStyle = useMemo(
    (): ViewStyle => ({
      ...StyleSheet.absoluteFill,
      backgroundColor: "transparent",
      zIndex: image === undefined ? -10 : 100
    }),
    [image]
  )

  const listItemHeight = menuLayout?.listItemHeight ?? DEFAULT_LIST_ITEM_HEIGHT
  const popupHeight = listItemHeight * options.length

  const popupStyle = useMemo((): ViewStyle => {
    if (params === undefined) return {}
    const { height, pageX, pageY, width } = params.layout
    // `canvasSize` is a Reanimated SharedValue — this is an intentional
    // one-shot `.value` read at the moment the menu position is computed
    // (mirroring Arcade's original `canvasSize.current` snapshot), not a
    // reactive dependency, since screen size doesn't change mid-popup.
    const canvasHeight = canvasSize.value.height
    const canvasWidth = canvasSize.value.width

    // Grow the menu on whichever side of the pressed node has room.
    const yAlignment = canvasHeight - pageY - popupHeight < 100 ? "top" : "bottom"
    const xAlignment = canvasWidth - pageX > 200 ? "left" : "right"
    const alignment: PopupAlignment = `${yAlignment}-${xAlignment}`

    const x = alignment.includes("right") ? width : pageX
    const y = alignment.includes("bottom") ? pageY + height : pageY - popupHeight
    const additionalYSpace = 5 * (yAlignment === "top" ? -1 : 1)

    return {
      height: popupHeight,
      position: "absolute",
      top: y + additionalYSpace,
      [xAlignment]: x
    } as ViewStyle
    // eslint-disable-next-line react-hooks/exhaustive-deps -- see the `canvasSize` comment above
  }, [params, popupHeight])

  const rMenuPopupStyle = useAnimatedStyle(() => ({ opacity: menuOpacity.value }))

  const contextValue = useMemo(() => ({ showPopup }), [showPopup])

  return (
    <BlurredPopupContext.Provider value={contextValue}>
      <Animated.View animatedProps={menuAnimatedProps} style={styles.mainPopupContainerView}>
        <Animated.View
          className="overflow-hidden rounded-lg border border-border bg-surfaceRaised"
          style={[popupStyle, styles.popup, rMenuPopupStyle]}
        >
          {image === undefined || options.length === 0
            ? null
            : options.map((option, index) => (
                <TouchableOpacity
                  activeOpacity={0.85}
                  className={`flex-row items-center px-3 ${
                    index === 0 ? "" : "border-t border-border"
                  }`}
                  key={`${option.label}-${index}`}
                  onPress={() => {
                    close()
                    option.onPress?.()
                  }}
                  style={{ height: listItemHeight }}
                >
                  {option.leading ?? <View className="mr-2 h-2 w-2 rounded-sm bg-accent" />}
                  <Text className="mr-3 font-mono text-sm text-text">{option.label}</Text>
                  <View className="flex-1" />
                  {option.trailing}
                </TouchableOpacity>
              ))}
        </Animated.View>
        <View onTouchEnd={close} style={styles.popupBackground} />
        <View style={[nodeStyle, styles.nodeZ]}>{Boolean(image) && params?.node}</View>
      </Animated.View>
      <Canvas onSize={canvasSize} onTouchEnd={close} style={canvasStyle}>
        {image === undefined ? null : (
          <>
            <Image image={image} rect={imageRect}>
              <Blur blur={sLightBlurValue} />
            </Image>
            <Image image={image} rect={imageRect}>
              <Blur blur={rBlur} />
            </Image>
          </>
        )}
      </Canvas>
      <View className="flex-1" ref={mainView}>
        {children}
      </View>
    </BlurredPopupContext.Provider>
  )
}

const styles = StyleSheet.create({
  mainPopupContainerView: { ...StyleSheet.absoluteFill, zIndex: 500 },
  nodeZ: { zIndex: -30 },
  popup: { zIndex: 20 },
  popupBackground: { ...StyleSheet.absoluteFill, zIndex: -20 }
})
