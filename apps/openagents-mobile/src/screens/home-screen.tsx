import { useEffect, useMemo, useState } from "react"
import { Platform, Pressable, Text as RNText, TextInput, View as RNView } from "react-native"
import { SafeAreaView, useSafeAreaInsets } from "react-native-safe-area-context"

import { Effect, Stream } from "@effect-native/core/effect"
import { khalaTheme } from "@effect-native/tokens"

import { loadGlassComposer, loadGlassIconButton, loadGlassPill } from "openagents-liquid-glass"
import { EffectNativeHost } from "../effect-native/effect-native-host"
import { sendKhalaTurn } from "../khala/khala-client"
import {
  buildHomeProgram,
  chromeProps,
  initialHomeState,
  renderContentView,
  renderDrawerView,
  surfaceModeOptions,
  type MobileSyncPhase,
} from "./home-core"

/**
 * The React Native shell only positions the native SwiftUI islands and mounts
 * the Effect Native program. The active product path is persona-neutral Khala:
 * no named-persona relationship/session, demo media, local transcript catalog, or
 * presentation-only purchase state remains in this client.
 */
const enPlatform = Platform.OS === "android" ? ("android" as const) : ("ios" as const)
const GlassIconButton = Platform.OS === "ios" ? loadGlassIconButton() : undefined
const GlassPill = Platform.OS === "ios" ? loadGlassPill() : undefined
const GlassComposer = Platform.OS === "ios" ? loadGlassComposer() : undefined

const fallbackChromeStyle = {
  backgroundColor: "rgba(11, 18, 32, 0.9)",
  borderColor: khalaTheme.color.border,
  borderWidth: 1,
} as const

export const HomeScreen = ({ syncPhase }: { readonly syncPhase: MobileSyncPhase }) => {
  const program = useMemo(() => buildHomeProgram({ khalaTurn: { sendTurn: sendKhalaTurn } }), [])
  const [homeState, setHomeState] = useState(initialHomeState)
  const insets = useSafeAreaInsets()

  useEffect(() => {
    const controller = new AbortController()
    Effect.runPromise(
      Stream.runForEach(program.stateChanges, (next) => Effect.sync(() => setHomeState(next))),
      { signal: controller.signal },
    ).catch(() => {
      // Interrupting the state subscription on unmount is expected.
    })
    return () => controller.abort()
  }, [program])

  useEffect(() => {
    program.sync.setPhase(syncPhase)
  }, [program, syncPhase])

  const chrome = chromeProps(homeState)

  return (
    <RNView style={{ flex: 1, backgroundColor: khalaTheme.color.background }}>
      <SafeAreaView edges={["top"]} style={{ flex: 1 }} pointerEvents="box-none">
        <RNView style={{ flex: 1 }}>
          <EffectNativeHost
            viewStream={program.contentViewStream}
            report={program.report}
            theme={khalaTheme}
            platform={enPlatform}
            initialView={renderContentView(initialHomeState)}
          />
        </RNView>

        {chrome.chromeVisible ? (
          <>
            <RNView
              pointerEvents="box-none"
              style={{
                position: "absolute",
                top: insets.top + 8,
                left: 16,
                right: 16,
                flexDirection: "row",
                alignItems: "center",
                gap: 10,
              }}
            >
              {GlassIconButton === undefined ? (
                <Pressable accessibilityRole="button" accessibilityLabel="Open navigation" onPress={program.chrome.toggleDrawer} style={{ width: 44, height: 44, borderRadius: 22, alignItems: "center", justifyContent: "center", ...fallbackChromeStyle }}>
                  <RNText style={{ color: khalaTheme.color.textPrimary, fontSize: 18 }}>≡</RNText>
                </Pressable>
              ) : <GlassIconButton symbol="line.3.horizontal" accessibilityLabelText="Open navigation" onTap={program.chrome.toggleDrawer} style={{ width: 44, height: 44 }} />}
              {GlassPill === undefined ? (
                <Pressable accessibilityRole="button" accessibilityLabel={chrome.pillLabel} onPress={() => program.chrome.selectSurfaceMode(chrome.surfaceMode === "khala" ? "openagents" : "khala")} style={{ height: 44, borderRadius: 22, paddingHorizontal: 16, justifyContent: "center", ...fallbackChromeStyle }}>
                  <RNText style={{ color: khalaTheme.color.textPrimary, fontWeight: "600" }}>{chrome.pillLabel}</RNText>
                </Pressable>
              ) : <GlassPill label={chrome.pillLabel} symbol="sparkles" options={surfaceModeOptions} selectedId={chrome.surfaceMode} onSelect={(event) => program.chrome.selectSurfaceMode(event.nativeEvent.id === "openagents" ? "openagents" : "khala")} style={{ width: 76 + chrome.pillLabel.length * 10, height: 44 }} />}
              <RNView style={{ flex: 1 }} />
              {GlassIconButton === undefined ? null : <GlassIconButton symbol="square.and.pencil" accessibilityLabelText="New chat" onTap={program.chrome.pressNewChat} style={{ width: 44, height: 44 }} />}
            </RNView>

            {chrome.glassComposerVisible ? (
              <RNView
                pointerEvents="box-none"
                style={{ position: "absolute", bottom: insets.bottom + 16, left: 16, right: 16 }}
              >
                {GlassComposer === undefined ? (
                  <RNView style={{ height: 54, flexDirection: "row", alignItems: "center", gap: 10, paddingHorizontal: 14, borderRadius: 27, ...fallbackChromeStyle }}>
                    <Pressable accessibilityRole="button" accessibilityLabel="New chat" onPress={program.chrome.pressNewChat}>
                      <RNText style={{ color: khalaTheme.color.textPrimary, fontSize: 24 }}>＋</RNText>
                    </Pressable>
                    <TextInput
                      accessibilityLabel="Message Khala"
                      value={chrome.draft}
                      editable={!chrome.sending}
                      placeholder={chrome.composerPlaceholder}
                      placeholderTextColor={khalaTheme.color.textMuted}
                      onChangeText={program.khala.draftChanged}
                      onSubmitEditing={() => program.khala.submitTurn(chrome.draft)}
                      style={{ flex: 1, color: khalaTheme.color.textPrimary, fontSize: 16 }}
                    />
                    <Pressable accessibilityRole="button" accessibilityLabel="Send message" onPress={() => program.khala.submitTurn(chrome.draft)}>
                      <RNText style={{ color: khalaTheme.color.accent, fontSize: 20 }}>{chrome.sending ? "…" : "↑"}</RNText>
                    </Pressable>
                  </RNView>
                ) : <GlassComposer placeholder={chrome.composerPlaceholder} text={chrome.draft} isSending={chrome.sending} onTextChange={(event) => program.khala.draftChanged(event.nativeEvent.text)} onSubmit={(event) => program.khala.submitTurn(event.nativeEvent.text)} onTapPlus={program.chrome.pressNewChat} style={{ height: 54 }} />}
              </RNView>
            ) : null}
          </>
        ) : null}

        {homeState.drawerOpen ? (
          <RNView style={{ position: "absolute", top: 0, bottom: 0, left: 0, right: 0, flexDirection: "row" }}>
            <RNView style={{ width: "82%", height: "100%", borderRightWidth: 1, borderRightColor: khalaTheme.color.border }}>
              <EffectNativeHost
                viewStream={program.drawerViewStream}
                report={program.report}
                theme={khalaTheme}
                platform={enPlatform}
                initialView={renderDrawerView(homeState)}
              />
            </RNView>
            <Pressable accessibilityRole="button" accessibilityLabel="Close navigation" onPress={program.chrome.toggleDrawer} style={{ flex: 1, backgroundColor: "rgba(0, 0, 0, 0.55)" }} />
          </RNView>
        ) : null}
      </SafeAreaView>
    </RNView>
  )
}
