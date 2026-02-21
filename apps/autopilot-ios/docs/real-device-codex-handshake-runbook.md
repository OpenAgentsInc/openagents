# Real Device Codex Handshake Runbook

## Preconditions

1. Desktop app running with runtime sync enabled.
2. iOS app signed in to `openagents.com`.
3. Target worker visible in iOS worker list.

## Procedure

1. Open iOS app and confirm signed-in session.
2. Load workers and select active desktop-backed worker.
3. Send handshake from iOS.
4. Observe ack event in iOS stream and desktop log/view.
5. Send a test message and verify mirrored updates on desktop and iOS.

## Required validations

1. No duplicate user message render.
2. Stream status transitions: connecting -> live.
3. Ack latency within expected range (record measured value in report).
4. Reconnect preserves watermark and does not duplicate applied events.

## Evidence

Store run artifact under `docs/reports/` with:

1. worker id
2. handshake id
3. request id(s)
4. pass/fail + notes
