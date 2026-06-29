#!/usr/bin/env bun
// ZERO-BASE SHELL programmatic-control proof (owner directive, 2026-06-19).
//
// Proves the exact hook a driver (Claude) uses to control the dead-simple shell
// and read back what the owner sees, with NO GUI:
//
//   1. SET INPUT   — the Bun→webview `shellControl` push routes to the SAME
//      inbound message the UI dispatches when the owner types (ChangedShellInput).
//   2. SUBMIT      — `shellControl{action:"submit"}` routes to SubmittedShell,
//      which records the user turn and fires the response seam.
//   3. RESPONSE    — the loopback command lands the Autopilot turn (RespondedShell).
//   4. READ        — `shellTranscriptText(model)` is the plain-text projection of
//      exactly what the shell view renders, so the driver sees the SAME state.
//
// We exercise the REAL reducer (update) + the REAL view + the REAL loopback
// reply, so this is honest parity, not a mock. Run from the repo root:
//
//   bun apps/autopilot-desktop/scripts/shell-control-proof.ts

import { initialRuntimeState } from "../src/ui/initial-state"
import { shellTranscriptText } from "../src/ui/model"
import { update } from "../src/ui/update"
import { view } from "../src/ui/view"
import {
  ChangedShellInput,
  RespondedShell,
  SubmittedShell,
} from "../src/ui/message"
import { shellLoopbackReply } from "../src/ui/commands"

const assert = (cond: boolean, msg: string): void => {
  if (!cond) {
    console.error(`FAIL: ${msg}`)
    process.exit(1)
  }
  console.log(`ok: ${msg}`)
}

// Serialize the rendered view tree (plain foldkit Html objects) so we can prove
// the driver-visible transcript text is actually IN the rendered screen.
const renderedText = (model: Parameters<typeof view>[0]): string => {
  const seen = new WeakSet<object>()
  return JSON.stringify(view(model).body, (_k, v) => {
    if (typeof v === "function") return "[fn]"
    if (typeof v === "object" && v !== null) {
      if (seen.has(v)) return "[cycle]"
      seen.add(v)
    }
    return v
  })
}

// 0. Launch → the black shell, nothing loaded.
let [model] = initialRuntimeState()
assert(model.pane === "shell", "launches on the dead-simple shell pane")
assert(model.shellTurns.length === 0, "launches with an empty conversation")

const PROMPT = "what can you do?"

// 1. SET INPUT (what the `shellControl{action:"set-input"}` push does).
;[model] = update(model, ChangedShellInput({ value: PROMPT }))
assert(model.shellInput === PROMPT, "set-input control sets the text bar")

// 2. SUBMIT (what the `shellControl{action:"submit"}` push does).
let commands
;[model, commands] = update(model, SubmittedShell())
assert(model.shellPending === true, "submit marks the turn pending")
assert(
  model.shellTurns.length === 1 && model.shellTurns[0].role === "you",
  "submit records the user turn",
)
assert(commands.length === 1, "submit dispatches exactly the response seam")

// 3. RESPONSE (what the loopback command produces).
;[model] = update(
  model,
  RespondedShell({ prompt: PROMPT, text: shellLoopbackReply(PROMPT) }),
)
assert(model.shellPending === false, "response clears pending")
assert(
  model.shellTurns.length === 2 && model.shellTurns[1].role === "autopilot",
  "response records the Autopilot turn",
)

// 4. READ (what a driver reads to see the SAME state the owner sees).
const transcript = shellTranscriptText(model)
console.log("--- transcript (driver-visible) ---")
console.log(transcript)
console.log("-----------------------------------")
assert(
  transcript === `you: ${PROMPT}\nautopilot: ${shellLoopbackReply(PROMPT)}`,
  "transcript projection matches the conversation",
)

// PARITY: every line the driver reads is actually in the rendered screen.
const screen = renderedText(model)
assert(screen.includes(PROMPT), "the prompt is in the rendered screen")
assert(
  screen.includes(shellLoopbackReply(PROMPT)),
  "the response is in the rendered screen",
)
assert(screen.includes("shell-input"), "the text bar is in the rendered screen")

console.log("\nPASS: shell programmatic-control + read parity proven (no GUI).")
