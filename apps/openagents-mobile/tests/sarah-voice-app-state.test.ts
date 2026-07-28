import { describe, expect, test } from "vite-plus/test"

import { sarahVoiceAppStateAction } from "../src/sarah-voice/app-state.ts"

describe("Sarah voice app state", () => {
  test("ends the session only when the app reaches the background", () => {
    expect(sarahVoiceAppStateAction("background")).toBe("end_session")
    expect(sarahVoiceAppStateAction("inactive")).toBe("stop_audio")
    expect(sarahVoiceAppStateAction("unknown")).toBe("stop_audio")
    expect(sarahVoiceAppStateAction("extension")).toBe("stop_audio")
    expect(sarahVoiceAppStateAction("active")).toBe("resume")
  })
})
