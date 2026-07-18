import { describe, expect, test } from "vite-plus/test"

import {
  mobileIntentUsesRouteTransition,
  mobileNativeFeedbackKind,
} from "../src/effect-native/mobile-native-feedback-policy"

// Oracle for openagents_mobile.t3_code_full_mobile_parity.v1.
describe("T3M-F2 restrained native motion and feedback", () => {
  test("classifies a closed set of consequential intents without haptics on text entry", () => {
    expect(mobileNativeFeedbackKind("ConversationThreadSelected")).toBe("selection")
    expect(mobileNativeFeedbackKind("KhalaTurnSubmitted")).toBe("action")
    expect(mobileNativeFeedbackKind("RepositoryGitPushRequested")).toBe("warning")
    expect(mobileNativeFeedbackKind("KhalaDraftChanged")).toBe("none")
    expect(mobileNativeFeedbackKind("RepositoryTerminalHostEvent")).toBe("none")
  })

  test("limits layout transitions to route, drawer, sheet, and picker changes", () => {
    expect(mobileIntentUsesRouteTransition("DrawerToggled")).toBe(true)
    expect(mobileIntentUsesRouteTransition("SettingsSectionSelected")).toBe(true)
    expect(mobileIntentUsesRouteTransition("TerminalRouteOpened")).toBe(true)
    expect(mobileIntentUsesRouteTransition("KhalaDraftChanged")).toBe(false)
    expect(mobileIntentUsesRouteTransition("TranscriptPinnedChanged")).toBe(false)
  })
})
