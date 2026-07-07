import type { Meta, StoryObj } from "@storybook/react-native"
import { useEffect, useMemo, useState } from "react"
import type { ReactNode } from "react"
import { Pressable, StyleSheet, View } from "react-native"
import Animated, {
  FadeIn,
  interpolate,
  interpolateColor,
  useAnimatedStyle,
  useDerivedValue,
  useSharedValue,
  withRepeat,
  withSpring,
  withTiming
} from "react-native-reanimated"

import { MOTION_AMBIENT, MOTION_FAST, MOTION_MEDIUM, MOTION_STAGGER_MS } from "../theme/motion"
import { khalaMobileTheme } from "../theme/tokens"
import { ActivityIndicator as KhalaActivityIndicator } from "./activity-indicator"
import { ArwesButton } from "./arwes-button"
import { BackgroundGradient } from "./background-gradient"
import { BlurredPopupProvider, TouchablePopupHandler } from "./blurred-popup"
import { Frame, usePowerOnVisible } from "./frame"
import { KhalaListItem } from "./khala-list-item"
import { KhalaText } from "./khala-text"
import { ReText } from "./re-text"
import { SwipeableItem } from "./swipeable-item"
import { SwipeQuoteDonut } from "./swipeable-item/swipe-quote-donut"
import { Toggle } from "./toggle"
import { TouchableFeedback } from "./touchable-feedback"

const meta = {
  title: "Khala/Animations/Arcade Ports",
  component: View,
  parameters: { layout: "fullscreen" },
  decorators: [
    (Story) => (
      <View style={styles.screen}>
        <Story />
      </View>
    )
  ]
} satisfies Meta<typeof View>

export default meta

type Story = StoryObj<typeof meta>

const StoryShell = ({ children }: { children: ReactNode }) => (
  <View style={styles.storyShell}>{children}</View>
)

const PowerOnFrame = ({ index, title }: { index: number; title: string }) => {
  const visible = usePowerOnVisible(index * MOTION_STAGGER_MS)

  return (
    <Frame style={styles.powerFrame} visible={visible}>
      <KhalaText className="text-accent" variant="body">
        {title}
      </KhalaText>
      <KhalaText className="mt-1" variant="muted">
        corner unfold + border grow
      </KhalaText>
    </Frame>
  )
}

const ArwesButtonExample = () => (
  <ArwesButton style={styles.arwesButton} visible alwaysShowBackground>
    <KhalaText className="text-center text-accent" variant="body">
      press glow
    </KhalaText>
  </ArwesButton>
)

const DrawerMorphButton = () => {
  const [open, setOpen] = useState(false)
  const progress = useSharedValue(0)

  useEffect(() => {
    progress.value = withSpring(open ? 1 : 0, { damping: 14, stiffness: 180 })
  }, [open, progress])

  useEffect(() => {
    const intervalId = setInterval(() => setOpen(value => !value), 1800)
    return () => clearInterval(intervalId)
  }, [])

  const topBar = useAnimatedStyle(() => ({
    backgroundColor: interpolateColor(progress.value, [0, 1], [khalaMobileTheme.text, khalaMobileTheme.accent]),
    marginBottom: interpolate(progress.value, [0, 1], [0, -2]),
    marginStart: interpolate(progress.value, [0, 1], [0, -11.5]),
    transform: [{ rotate: `${interpolate(progress.value, [0, 1], [0, -45])}deg` }],
    width: interpolate(progress.value, [0, 1], [24, 14])
  }))

  const middleBar = useAnimatedStyle(() => ({
    backgroundColor: interpolateColor(progress.value, [0, 1], [khalaMobileTheme.text, khalaMobileTheme.accent]),
    width: interpolate(progress.value, [0, 1], [24, 18])
  }))

  const bottomBar = useAnimatedStyle(() => ({
    backgroundColor: interpolateColor(progress.value, [0, 1], [khalaMobileTheme.text, khalaMobileTheme.accent]),
    marginStart: interpolate(progress.value, [0, 1], [0, -11.5]),
    marginTop: interpolate(progress.value, [0, 1], [5, 2]),
    transform: [{ rotate: `${interpolate(progress.value, [0, 1], [0, 45])}deg` }],
    width: interpolate(progress.value, [0, 1], [24, 14])
  }))

  return (
    <Pressable accessibilityRole="button" onPress={() => setOpen(value => !value)} style={styles.drawerButton}>
      <Animated.View style={[styles.drawerBar, topBar]} />
      <Animated.View style={[styles.drawerBar, middleBar]} />
      <Animated.View style={[styles.drawerBar, bottomBar]} />
    </Pressable>
  )
}

const ReplyPreviewReveal = () => {
  const [visible, setVisible] = useState(true)
  const progress = useDerivedValue(() => withTiming(visible ? 1 : 0, { duration: MOTION_MEDIUM }))

  useEffect(() => {
    const intervalId = setInterval(() => setVisible(value => !value), 1800)
    return () => clearInterval(intervalId)
  }, [])

  const containerStyle = useAnimatedStyle(() => ({
    height: progress.value * 72,
    opacity: progress.value
  }))

  return (
    <Animated.View style={[styles.replyPreview, containerStyle]}>
      <View style={styles.replyRail} />
      <View style={styles.replyText}>
        <KhalaText className="text-accent" variant="body">
          Agent reply target
        </KhalaText>
        <KhalaText numberOfLines={1} variant="muted">
          height + opacity reveal copied from Arcade DirectMessageReply
        </KhalaText>
      </View>
    </Animated.View>
  )
}

const TypingText = () => {
  const lines = useMemo(
    () => ["Booting Khala animation catalog...", "Arcade motion primitives online."],
    []
  )
  const [text, setText] = useState("")
  const [cursorVisible, setCursorVisible] = useState(true)

  useEffect(() => {
    let lineIndex = 0
    let charIndex = 0
    let cancelled = false

    const tick = () => {
      if (cancelled) return

      const line = lines[lineIndex] ?? ""
      if (charIndex < line.length) {
        setText(value => value + line.charAt(charIndex))
        charIndex += 1
        setTimeout(tick, 24)
        return
      }

      if (lineIndex + 1 < lines.length) {
        lineIndex += 1
        charIndex = 0
        setText(value => `${value}\n`)
        setTimeout(tick, 260)
      }
    }

    const typingTimeoutId = setTimeout(tick, 250)
    const cursorIntervalId = setInterval(() => setCursorVisible(value => !value), 260)

    return () => {
      cancelled = true
      clearTimeout(typingTimeoutId)
      clearInterval(cursorIntervalId)
    }
  }, [lines])

  return (
    <KhalaText className="font-mono text-text" variant="body">
      {text}
      <KhalaText className={cursorVisible ? "text-accent" : "text-transparent"} variant="body">
        |
      </KhalaText>
    </KhalaText>
  )
}

const ReTextTicker = () => {
  const label = useSharedValue("UI-thread text: standby")

  useEffect(() => {
    let index = 0
    const states = ["UI-thread text: standby", "UI-thread text: measuring", "UI-thread text: synced"] as const
    const intervalId = setInterval(() => {
      index = (index + 1) % states.length
      label.value = states[index] ?? states[0]
    }, 900)
    return () => clearInterval(intervalId)
  }, [label])

  return <ReText style={styles.reText} text={label} />
}

const StaggeredRows = () => {
  const rows = ["Frame", "Swipe", "Popup", "Gradient"]

  return (
    <View style={styles.staggerRows}>
      {rows.map((row, index) => (
        <Animated.View entering={FadeIn.delay(320 + MOTION_STAGGER_MS * index).duration(MOTION_MEDIUM)} key={row}>
          <KhalaListItem detail="FadeIn.delay(index * MOTION_STAGGER_MS)" meta={`0${index + 1}`} title={row} variant="surface" />
        </Animated.View>
      ))}
    </View>
  )
}

const TabFadeInExample = () => (
  <Frame style={styles.tabFrame} visible>
    <View style={styles.tabRow}>
      {["Threads", "Repos", "Runs"].map((tab, index) => (
        <Animated.View entering={FadeIn.delay(500 + 50 * index).duration(300)} key={tab} style={styles.tabPill}>
          <KhalaText className={index === 0 ? "text-accent" : "text-textSoft"} variant="caption">
            {tab}
          </KhalaText>
        </Animated.View>
      ))}
    </View>
  </Frame>
)

const AmbientPulseCard = () => (
  <BackgroundGradient cornerRadius={8} maxBlur={14} style={styles.gradientCard}>
    <KhalaText className="text-accent" variant="body">
      ambient sweep gradient
    </KhalaText>
    <KhalaText className="mt-1" variant="muted">
      withRepeat(withTiming(...), -1, true)
    </KhalaText>
  </BackgroundGradient>
)

const DonutProgressLoop = () => {
  const progress = useSharedValue(0)

  useEffect(() => {
    progress.value = withRepeat(withTiming(1, { duration: MOTION_AMBIENT }), -1, true)
  }, [progress])

  return <SwipeQuoteDonut progress={progress} size={72} strokeWidth={5} />
}

const ToggleExample = () => {
  const [checked, setChecked] = useState(true)
  const [switched, setSwitched] = useState(false)

  return (
    <View style={styles.stack}>
      <Toggle label="Timing checkbox opacity" value={checked} onValueChange={setChecked} />
      <Toggle label="Timing switch knob" value={switched} onValueChange={setSwitched} variant="switch" />
    </View>
  )
}

export const SkiaSpinnerAndFrame: Story = {
  render: () => (
    <StoryShell>
      <KhalaText variant="heading">Skia spinner + frame unfold</KhalaText>
      <View style={styles.row}>
        <KhalaActivityIndicator size={72} strokeWidth={5} type="large" />
        <PowerOnFrame index={0} title="Power-on frame" />
      </View>
      <ArwesButtonExample />
    </StoryShell>
  )
}

export const AmbientGradientAndDonut: Story = {
  render: () => (
    <StoryShell>
      <KhalaText variant="heading">Ambient loops</KhalaText>
      <AmbientPulseCard />
      <View style={styles.centerRow}>
        <DonutProgressLoop />
        <KhalaText className="max-w-52" variant="muted">
          The donut uses Arcade's eased progress fill, driven directly by a Reanimated shared value.
        </KhalaText>
      </View>
    </StoryShell>
  )
}

export const GestureFeedback: Story = {
  render: () => (
    <StoryShell>
      <KhalaText variant="heading">Gesture feedback</KhalaText>
      <TouchableFeedback className="rounded-lg border border-border px-4 py-4" onPress={() => undefined}>
        <KhalaText className="text-accent" variant="body">
          tap for UI-thread highlight
        </KhalaText>
      </TouchableFeedback>
      <ToggleExample />
      <DrawerMorphButton />
    </StoryShell>
  )
}

export const SwipeAndPopup: Story = {
  render: () => (
    <BlurredPopupProvider>
      <StoryShell>
        <KhalaText variant="heading">Swipe + blurred popup</KhalaText>
        <SwipeableItem onSwipeComplete={() => undefined}>
          <KhalaListItem
            detail="Drag right to fill the donut and reveal the quote badge."
            meta="swipe"
            title="Swipeable transcript row"
            variant="surface"
          />
        </SwipeableItem>
        <TouchablePopupHandler
          options={[
            { label: "Quote", onPress: () => undefined },
            { label: "Copy", onPress: () => undefined },
            { label: "Dismiss", onPress: () => undefined }
          ]}
        >
          <KhalaListItem
            detail="Long-press to freeze and blur the Storybook canvas behind this row."
            meta="hold"
            title="Blurred popup target"
            variant="surface"
          />
        </TouchablePopupHandler>
      </StoryShell>
    </BlurredPopupProvider>
  )
}

export const TextAndReplyMotion: Story = {
  render: () => (
    <StoryShell>
      <KhalaText variant="heading">Text and reply motion</KhalaText>
      <ReplyPreviewReveal />
      <View style={styles.terminal}>
        <TypingText />
      </View>
      <ReTextTicker />
    </StoryShell>
  )
}

export const EntranceStaggers: Story = {
  render: () => (
    <StoryShell>
      <KhalaText variant="heading">Entrance staggers</KhalaText>
      <StaggeredRows />
      <TabFadeInExample />
    </StoryShell>
  )
}

const styles = StyleSheet.create({
  arwesButton: {
    alignItems: "center",
    height: 64,
    justifyContent: "center",
    paddingHorizontal: 16,
    width: "100%"
  },
  centerRow: { alignItems: "center", flexDirection: "row", gap: 18 },
  drawerBar: { borderRadius: 999, height: 2 },
  drawerButton: {
    alignItems: "center",
    borderColor: khalaMobileTheme.border,
    borderRadius: 8,
    borderWidth: 1,
    height: 64,
    justifyContent: "center",
    width: 64
  },
  gradientCard: {
    borderRadius: 8,
    overflow: "hidden",
    padding: 20,
    width: "100%"
  },
  powerFrame: {
    flex: 1,
    minHeight: 96,
    padding: 16
  },
  reText: {
    color: khalaMobileTheme.accent,
    fontFamily: "JetBrainsMono_500Medium",
    fontSize: 14,
    paddingVertical: 0
  },
  replyPreview: {
    alignItems: "center",
    backgroundColor: "rgba(79, 208, 255, 0.08)",
    borderTopColor: khalaMobileTheme.border,
    borderTopWidth: 1,
    flexDirection: "row",
    overflow: "hidden",
    width: "100%"
  },
  replyRail: {
    backgroundColor: khalaMobileTheme.accent,
    borderRadius: 999,
    height: 36,
    marginHorizontal: 14,
    width: 3
  },
  replyText: { flex: 1, justifyContent: "center", paddingRight: 12 },
  row: { alignItems: "center", flexDirection: "row", gap: 18 },
  screen: { backgroundColor: khalaMobileTheme.background, flex: 1 },
  stack: { gap: 14 },
  staggerRows: { gap: 10 },
  storyShell: {
    flex: 1,
    gap: 18,
    padding: 20
  },
  tabFrame: {
    height: 68,
    justifyContent: "center",
    paddingHorizontal: 10,
    width: "100%"
  },
  tabPill: {
    alignItems: "center",
    flex: 1,
    justifyContent: "center"
  },
  tabRow: { flexDirection: "row" },
  terminal: {
    backgroundColor: khalaMobileTheme.surfaceRaised,
    borderColor: khalaMobileTheme.border,
    borderRadius: 8,
    borderWidth: 1,
    minHeight: 116,
    padding: 16
  }
})
