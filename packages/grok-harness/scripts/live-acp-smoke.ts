#!/usr/bin/env bun
/**
 * Live ACP smoke (env-armed). Requires `grok login` or XAI_API_KEY.
 *
 *   GROK_ACP_LIVE=1 bun packages/grok-harness/scripts/live-acp-smoke.ts
 */

import {
  createGrokAcpChatRuntime,
} from "../src/chat-runtime.ts"

if (process.env.GROK_ACP_LIVE !== "1") {
  console.error("Set GROK_ACP_LIVE=1 to run the live ACP smoke.")
  process.exit(2)
}

const runtime = await createGrokAcpChatRuntime()
try {
  const thread = await runtime.startThread({ cwd: process.cwd() })
  const events: string[] = []
  const turn = await runtime.startTurn({
    threadId: thread.threadId,
    desktopSessionId: thread.desktopSessionId,
    grokSessionId: thread.grokSessionId,
    prompt: "Reply with only the single word: pong",
    onEvent: (e) => events.push(e.type),
  })
  console.log(
    JSON.stringify(
      {
        ok: turn.text.toLowerCase().includes("pong") || turn.text.trim().length > 0,
        stopReason: turn.stopReason,
        text: turn.text.slice(0, 200),
        eventTypes: [...new Set(events)],
        threadId: thread.threadId,
      },
      null,
      2,
    ),
  )
  if (turn.text.trim().length === 0) process.exit(1)
} finally {
  runtime.dispose()
}
